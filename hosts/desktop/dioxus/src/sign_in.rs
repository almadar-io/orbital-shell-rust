//! The desktop host's sign-in (RFC 8252 loopback; owner 2026-11-23). There is no Firebase SDK for a native desktop
//! app, so the host opens the app's hand-off page (`native-sign-in.html`, the web template's second page) in the
//! system browser with a loopback port and a one-time state. The page signs the viewer in with the web SDK and
//! form-POSTs the ID token and refresh token back here, and the host keeps the refresh token and renews the ID token
//! through the site's own Firebase config. The kernel and core know no provider: the host hands the ID token to the
//! transport's bearer slot (EXEC-IDENTITY-4) and answers `auth/sign-in` with the viewer it names (EXEC-DIST-13).
//! Everything here is std only with the transport passed in, so it is tested with the crate's default features.

use orbital_core::{Value, ValueMap};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

/// The pair the hand-off page returns and a refresh renews.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceTokens {
    pub id_token: String,
    pub refresh_token: String,
}

/// A refresh's answer: the new pair and how long the ID token lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renewed {
    pub tokens: DeviceTokens,
    pub expires_in_secs: u64,
}

/// Who the ID token names, read from its claims for the `auth/sign-in` answer; the server verifies the token itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewer {
    pub uid: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub expires_at_secs: u64,
}

impl Viewer {
    /// The `auth/sign-in` success payload (EXEC-DIST-13): `{ uid, email?, displayName? }`. The token never enters it.
    pub fn answer(&self) -> Value {
        let mut fields = ValueMap::new();
        fields.insert("uid".into(), Value::string(self.uid.as_str()));
        if let Some(email) = &self.email {
            fields.insert("email".into(), Value::string(email.as_str()));
        }
        if let Some(name) = &self.display_name {
            fields.insert("displayName".into(), Value::string(name.as_str()));
        }
        Value::object(fields)
    }
}

/// A one-time state: 32 bytes from the OS CSPRNG, base64url.
pub fn new_state() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("no randomness for the sign-in state: {e}"))?;
    Ok(orbital_core::codec::base64url_encode(&bytes))
}

/// The hand-off page on the app's web site, told where to post back.
pub fn sign_in_url(site: &str, port: u16, state: &str) -> String {
    format!("{}/native-sign-in.html?port={port}&state={state}", site.trim_end_matches('/'))
}

const RETURNED: &str = "<!DOCTYPE html><meta charset=\"utf-8\"><title>Signed in</title><p>Signed in. You can return to the app.</p>";
const REFUSED: &str = "<!DOCTYPE html><meta charset=\"utf-8\"><title>Not signed in</title><p>This sign-in did not come from the app. Start again from the app.</p>";

/// Accept the hand-off page's POST on `listener` and answer the browser. Only a form whose `state` equals the one
/// this sign-in drew, carrying both tokens, hands them over; anything else is refused with a 400 and hands nothing.
/// A viewer who never finishes in the browser ends the sign-in at `within`.
pub fn accept_callback(listener: &TcpListener, state: &str, within: std::time::Duration) -> Result<DeviceTokens, String> {
    let deadline = std::time::Instant::now() + within;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                return Err("sign-in was not finished in the browser in time".into());
            }
            Err(e) => return Err(format!("sign-in callback: {e}")),
        }
    };
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    let outcome = read_form(&mut stream).and_then(|form| tokens_of(&form, state));
    let (status, page) = if outcome.is_ok() { ("200 OK", RETURNED) } else { ("400 Bad Request", REFUSED) };
    let _ = write!(stream, "HTTP/1.1 {status}\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n{page}", page.len());
    outcome
}

fn read_form(stream: &mut std::net::TcpStream) -> Result<Vec<(String, String)>, String> {
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    if !line.starts_with("POST /callback ") {
        return Err("sign-in callback: not the hand-off page's POST".into());
    }
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).map_err(|e| e.to_string())?;
        if header == "\r\n" || header.is_empty() {
            break;
        }
        if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().map_err(|_| "sign-in callback: bad content-length".to_string())?;
        }
    }
    if length > 64 * 1024 {
        return Err("sign-in callback: the form is too large".into());
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).map_err(|e| e.to_string())?;
    Ok(form_urlencoded::parse(&body).into_owned().collect())
}

fn tokens_of(form: &[(String, String)], state: &str) -> Result<DeviceTokens, String> {
    let field = |name: &str| form.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
    if field("state").as_deref() != Some(state) {
        return Err("sign-in callback: the state does not match this sign-in".into());
    }
    match (field("idToken"), field("refreshToken")) {
        (Some(id_token), Some(refresh_token)) => Ok(DeviceTokens { id_token, refresh_token }),
        _ => Err("sign-in callback: the form carries no ID token or no refresh token".into()),
    }
}

/// The site's web API key, from its served `/__/firebase/init.json`.
pub fn site_api_key(init_json: &str) -> Result<String, String> {
    let config: serde_json::Value = serde_json::from_str(init_json).map_err(|e| format!("firebase init.json: {e}"))?;
    config
        .get("apiKey")
        .and_then(serde_json::Value::as_str)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "firebase init.json names no apiKey".to_string())
}

/// The refresh call: the URL and the form body, posted by the caller's HTTPS transport.
pub fn refresh_request(token_origin: &str, api_key: &str, refresh_token: &str) -> (String, String) {
    let body = form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "refresh_token")
        .append_pair("refresh_token", refresh_token)
        .finish();
    (format!("{}/v1/token?key={api_key}", token_origin.trim_end_matches('/')), body)
}

/// Read a refresh answer; a refusal (an `error` record, a missing field) is an error naming why.
pub fn parse_refresh(text: &str) -> Result<Renewed, String> {
    let answer: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("token refresh: {e}"))?;
    if let Some(message) = answer.get("error").and_then(|e| e.get("message")).and_then(serde_json::Value::as_str) {
        return Err(format!("token refresh refused: {message}"));
    }
    let text_of = |name: &str| answer.get(name).and_then(serde_json::Value::as_str).map(str::to_string);
    match (text_of("id_token"), text_of("refresh_token"), text_of("expires_in").and_then(|s| s.parse::<u64>().ok())) {
        (Some(id_token), Some(refresh_token), Some(expires_in_secs)) => {
            Ok(Renewed { tokens: DeviceTokens { id_token, refresh_token }, expires_in_secs })
        }
        _ => Err("token refresh: the answer carries no id_token, refresh_token or expires_in".into()),
    }
}

/// The viewer an ID token names, from its claims (`sub`, `email`, `name`, `exp`).
pub fn viewer_of(id_token: &str) -> Result<Viewer, String> {
    let mut parts = id_token.split('.');
    let (Some(_), Some(body), Some(_), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err("the ID token is not three segments".into());
    };
    let bytes = orbital_core::codec::base64url_decode(body)?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| format!("the ID token's claims: {e}"))?;
    let text = |name: &str| claims.get(name).and_then(serde_json::Value::as_str).map(str::to_string);
    let uid = text("sub").filter(|s| !s.is_empty()).ok_or_else(|| "the ID token names no subject".to_string())?;
    let expires_at_secs = claims.get("exp").and_then(serde_json::Value::as_u64).ok_or_else(|| "the ID token has no exp".to_string())?;
    Ok(Viewer { uid, email: text("email"), display_name: text("name"), expires_at_secs })
}

/// The desktop glue: the system browser, the OS keychain and the HTTPS calls around the std logic above.
#[cfg(all(feature = "native-sign-in", not(any(target_os = "android", target_os = "ios"))))]
pub mod desktop {
    use super::*;
    use std::sync::OnceLock;

    /// How long a viewer has to finish signing in in the browser.
    const SIGN_IN_WITHIN: std::time::Duration = std::time::Duration::from_secs(300);
    /// Renew the ID token this long before it expires.
    const RENEW_AHEAD_SECS: u64 = 300;
    const TOKEN_ORIGIN: &str = "https://securetoken.googleapis.com";
    const KEYCHAIN_SERVICE: &str = "almadar-native-sign-in";

    static SITE: OnceLock<String> = OnceLock::new();

    /// The app's web site whose hand-off page signs the viewer in, as the launch declared it (`--sign-in-site`).
    pub fn set_site(site: String) {
        let _ = SITE.set(site);
    }

    pub fn site() -> Option<&'static str> {
        SITE.get().map(String::as_str)
    }

    static SIGNED_IN: OnceLock<std::sync::Arc<std::sync::atomic::AtomicBool>> = OnceLock::new();

    /// Whether this app holds a signed-in viewer; a sign-out turns it false and stops the renewal loop.
    pub fn signed_in() -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        SIGNED_IN.get_or_init(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false))).clone()
    }

    /// Sign the viewer in through the browser: the viewer the token names and the pair.
    pub fn sign_in(site: &str) -> Result<(Viewer, DeviceTokens), String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("sign-in: no loopback port: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let state = new_state()?;
        webbrowser::open(&sign_in_url(site, port, &state)).map_err(|e| format!("sign-in: the browser did not open: {e}"))?;
        let tokens = accept_callback(&listener, &state, SIGN_IN_WITHIN)?;
        Ok((viewer_of(&tokens.id_token)?, tokens))
    }

    /// Renew the pair with the refresh token, through the site's own Firebase web config.
    pub fn renew(site: &str, refresh_token: &str) -> Result<Renewed, String> {
        let init = ureq::get(&format!("{}/__/firebase/init.json", site.trim_end_matches('/')))
            .call()
            .map_err(|e| format!("sign-in: the site's firebase config: {e}"))?
            .into_string()
            .map_err(|e| e.to_string())?;
        let (url, body) = refresh_request(TOKEN_ORIGIN, &site_api_key(&init)?, refresh_token);
        let answer = match ureq::post(&url).set("content-type", "application/x-www-form-urlencoded").send_string(&body) {
            Ok(reply) => reply.into_string().map_err(|e| e.to_string())?,
            Err(ureq::Error::Status(_, reply)) => reply.into_string().map_err(|e| e.to_string())?,
            Err(e) => return Err(format!("token refresh: {e}")),
        };
        parse_refresh(&answer)
    }

    fn entry(site: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYCHAIN_SERVICE, site).map_err(|e| format!("keychain: {e}"))
    }

    /// Keep the refresh token in the OS keychain under the site.
    pub fn remember(site: &str, refresh_token: &str) -> Result<(), String> {
        entry(site)?.set_password(refresh_token).map_err(|e| format!("keychain: {e}"))
    }

    /// The refresh token the keychain holds for the site, if any.
    pub fn recall(site: &str) -> Option<String> {
        entry(site).ok()?.get_password().ok()
    }

    /// Forget the site's refresh token (sign-out).
    pub fn forget(site: &str) {
        if let Ok(entry) = entry(site) {
            let _ = entry.delete_credential();
        }
    }

    /// Keep `kernel`'s bearer current from `refresh_token` until a renewal fails or the viewer signs out
    /// (`signed_in` turns false): each round sleeps until shortly before the token expires, renews and re-stores.
    pub fn keep_fresh(
        site: String,
        kernel: std::sync::Arc<orbital_core::client::client_kernel::ClientKernel>,
        mut tokens: DeviceTokens,
        signed_in: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) {
        std::thread::spawn(move || loop {
            let Ok(viewer) = viewer_of(&tokens.id_token) else { return };
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let wait = viewer.expires_at_secs.saturating_sub(now).saturating_sub(RENEW_AHEAD_SECS);
            std::thread::sleep(std::time::Duration::from_secs(wait.max(1)));
            if !signed_in.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            match renew(&site, &tokens.refresh_token) {
                Ok(renewed) => {
                    tokens = renewed.tokens;
                    kernel.set_bearer(Some(tokens.id_token.clone()));
                    if let Err(error) = remember(&site, &tokens.refresh_token) {
                        tracing::error!(%error, "sign-in: the renewed refresh token was not kept");
                    }
                }
                Err(error) => {
                    tracing::error!(%error, "sign-in: renewal failed; the viewer is signed out");
                    kernel.set_bearer(None);
                    return;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    fn unsigned_token(claims: serde_json::Value) -> String {
        let header = orbital_core::codec::base64url_encode(br#"{"alg":"RS256","typ":"JWT"}"#);
        let body = orbital_core::codec::base64url_encode(claims.to_string().as_bytes());
        format!("{header}.{body}.c2ln")
    }

    #[test]
    fn a_state_is_long_random_and_url_safe() {
        let a = new_state().unwrap();
        let b = new_state().unwrap();
        assert_ne!(a, b);
        assert!(a.len() >= 43, "32 random bytes, base64url");
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn the_sign_in_url_names_the_hand_off_page_the_port_and_the_state() {
        let url = sign_in_url("https://family.web.app/", 51234, "abc_DEF-123");
        assert_eq!(url, "https://family.web.app/native-sign-in.html?port=51234&state=abc_DEF-123");
    }

    const WAIT: std::time::Duration = std::time::Duration::from_secs(5);

    #[test]
    fn control_no_callback_before_the_deadline_ends_the_sign_in() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let started = std::time::Instant::now();
        assert!(accept_callback(&listener, "s1", std::time::Duration::from_millis(200)).is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
    }

    fn post(port: u16, body: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(stream, "POST /callback HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut reply = String::new();
        let _ = stream.read_to_string(&mut reply);
        reply
    }

    #[test]
    fn the_callback_with_the_right_state_hands_over_both_tokens_and_tells_the_browser() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let browser = std::thread::spawn(move || post(port, "state=s1&idToken=id%2E1&refreshToken=r1"));
        let tokens = accept_callback(&listener, "s1", WAIT).unwrap();
        assert_eq!(tokens, DeviceTokens { id_token: "id.1".into(), refresh_token: "r1".into() });
        assert!(browser.join().unwrap().starts_with("HTTP/1.1 200"));
    }

    #[test]
    fn control_a_callback_with_another_state_is_refused_and_hands_over_nothing() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let browser = std::thread::spawn(move || post(port, "state=forged&idToken=i&refreshToken=r"));
        assert!(accept_callback(&listener, "s1", WAIT).is_err());
        assert!(browser.join().unwrap().starts_with("HTTP/1.1 400"));
    }

    #[test]
    fn control_a_callback_missing_a_token_is_refused() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let browser = std::thread::spawn(move || post(port, "state=s1&idToken=i"));
        assert!(accept_callback(&listener, "s1", WAIT).is_err());
        assert!(browser.join().unwrap().starts_with("HTTP/1.1 400"));
    }

    #[test]
    fn the_api_key_comes_from_the_sites_served_firebase_config() {
        assert_eq!(site_api_key(r#"{"apiKey":"AIza-1","projectId":"p"}"#).unwrap(), "AIza-1");
        assert!(site_api_key(r#"{"projectId":"p"}"#).is_err(), "control: no key, no refresh");
    }

    #[test]
    fn a_refresh_posts_the_refresh_token_and_reads_the_new_pair() {
        let (url, body) = refresh_request("https://securetoken.googleapis.com", "AIza-1", "r1");
        assert_eq!(url, "https://securetoken.googleapis.com/v1/token?key=AIza-1");
        assert_eq!(body, "grant_type=refresh_token&refresh_token=r1");
        let renewed = parse_refresh(r#"{"id_token":"id2","refresh_token":"r2","expires_in":"3600"}"#).unwrap();
        assert_eq!(renewed, Renewed { tokens: DeviceTokens { id_token: "id2".into(), refresh_token: "r2".into() }, expires_in_secs: 3600 });
        assert!(parse_refresh(r#"{"error":{"message":"TOKEN_EXPIRED"}}"#).is_err(), "control: a refused refresh is an error");
    }

    #[test]
    fn the_viewer_answered_to_the_program_is_read_from_the_id_tokens_claims() {
        let token = unsigned_token(serde_json::json!({ "sub": "u1", "email": "m@x.si", "name": "Mama", "exp": 2000 }));
        let viewer = viewer_of(&token).unwrap();
        assert_eq!(viewer.uid, "u1");
        assert_eq!(viewer.email.as_deref(), Some("m@x.si"));
        assert_eq!(viewer.display_name.as_deref(), Some("Mama"));
        assert_eq!(viewer.expires_at_secs, 2000);
        assert!(viewer_of("not-a-token").is_err(), "control: no three segments, no viewer");
        let answer = viewer.answer();
        assert_eq!(answer.as_object().and_then(|o| o.get("uid")), Some(&orbital_core::Value::string("u1")));
        assert!(answer.as_object().is_some_and(|o| o.get("idToken").is_none()), "edge: the token never reaches the program");
    }
}
