//! A thin Chrome DevTools Protocol client: launch Chrome with a debugging port, attach to one page, send commands and
//! read their answers. The browser driver of `orb verify` walks a web client through it; nothing else is wrapped.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// The Chrome binary: `ORB_CHROME`, else the installed Google Chrome, else a Playwright Chromium, else one on `PATH`.
pub fn chrome_executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ORB_CHROME")
        .map(PathBuf::from)
        .filter(|p| p.exists())
    {
        return Some(path);
    }
    let installed = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ];
    installed.iter().map(PathBuf::from).find(|p| p.exists())
}

/// A running Chrome and its browser-level DevTools socket.
pub struct Chrome {
    child: Child,
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    next_id: u64,
    session: Option<String>,
    /// Events read while waiting for an answer, in arrival order.
    events: Vec<Value>,
    _profile: TempProfile,
}

struct TempProfile(PathBuf);

impl Drop for TempProfile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl Drop for Chrome {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Chrome {
    /// Launch `executable` headless on a fresh profile and attach to a blank page.
    pub fn launch(executable: &Path) -> anyhow::Result<Self> {
        static LAUNCHED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = LAUNCHED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let profile = std::env::temp_dir().join(format!("orb-cdp-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&profile)?;
        let mut child = Command::new(executable)
            .args([
                "--headless=new",
                "--remote-debugging-port=0",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-gpu",
                &format!("--user-data-dir={}", profile.display()),
                "about:blank",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("Chrome has no stderr"))?;
        let mut url = None;
        for line in BufReader::new(stderr).lines() {
            let line = line?;
            if let Some(found) = line.strip_prefix("DevTools listening on ") {
                url = Some(found.trim().to_string());
                break;
            }
        }
        let url =
            url.ok_or_else(|| anyhow::anyhow!("Chrome exited before it opened its DevTools port"))?;
        // A page's screenshot or a large evaluation result outgrows tungstenite's 16 MiB default.
        let config = tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(None)
            .max_frame_size(None);
        let (socket, _) = tungstenite::client::connect_with_config(url.as_str(), Some(config), 3)?;
        let mut chrome = Self {
            child,
            socket,
            next_id: 0,
            session: None,
            events: Vec::new(),
            _profile: TempProfile(profile),
        };
        let target = chrome.send("Target.createTarget", json!({ "url": "about:blank" }))?;
        let target_id = target["targetId"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("no targetId: {target}"))?
            .to_string();
        let attached = chrome.send(
            "Target.attachToTarget",
            json!({ "targetId": target_id, "flatten": true }),
        )?;
        chrome.session = attached["sessionId"].as_str().map(str::to_string);
        chrome.send("Page.enable", json!({}))?;
        chrome.send("Runtime.enable", json!({}))?;
        Ok(chrome)
    }

    /// Send `method` (to the attached page once there is one) and wait for its answer.
    pub fn send(&mut self, method: &str, params: Value) -> anyhow::Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = &self.session {
            message["sessionId"] = Value::String(session.clone());
        }
        self.socket.send(Message::text(message.to_string()))?;
        loop {
            let Message::Text(text) = self.socket.read()? else {
                continue;
            };
            let reply: Value = serde_json::from_str(text.as_str())?;
            if reply["id"].as_u64() == Some(id) {
                if let Some(error) = reply.get("error") {
                    anyhow::bail!("{method}: {error}");
                }
                return Ok(reply["result"].clone());
            }
            if reply.get("method").is_some() {
                self.events.push(reply);
            }
        }
    }

    /// Navigate the page to `url` and wait for its load event, up to `timeout`.
    pub fn navigate(&mut self, url: &str, timeout: Duration) -> anyhow::Result<()> {
        self.events.retain(|e| e["method"] != "Page.loadEventFired");
        self.send("Page.navigate", json!({ "url": url }))?;
        let deadline = Instant::now() + timeout;
        while !self
            .events
            .iter()
            .any(|e| e["method"] == "Page.loadEventFired")
        {
            if Instant::now() > deadline {
                anyhow::bail!("{url} did not load within {timeout:?}");
            }
            self.send("Runtime.evaluate", json!({ "expression": "0" }))?;
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }

    /// Evaluate `expression` in the page, awaiting a promise, and answer its value as JSON; a thrown error is an error.
    pub fn evaluate(&mut self, expression: &str) -> anyhow::Result<Value> {
        let result = self.send(
            "Runtime.evaluate",
            json!({ "expression": expression, "awaitPromise": true, "returnByValue": true }),
        )?;
        if let Some(exception) = result.get("exceptionDetails") {
            let text = exception["exception"]["description"]
                .as_str()
                .or(exception["text"].as_str())
                .unwrap_or("evaluation failed");
            anyhow::bail!("{text}");
        }
        Ok(result["result"]["value"].clone())
    }

    /// Poll `predicate` (a page expression) until it is truthy, up to `timeout`.
    pub fn wait_for(&mut self, predicate: &str, timeout: Duration) -> anyhow::Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            if self.evaluate(predicate)?.as_bool() == Some(true) {
                return Ok(());
            }
            if Instant::now() > deadline {
                anyhow::bail!("{predicate} stayed false for {timeout:?}");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// A PNG of the page as it is now.
    pub fn screenshot(&mut self) -> anyhow::Result<Vec<u8>> {
        let shot = self.send("Page.captureScreenshot", json!({ "format": "png" }))?;
        let data = shot["data"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("no screenshot data"))?;
        orbital_core::codec::base64_decode(data)
            .map_err(|e| anyhow::anyhow!("decode screenshot: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chrome() -> Option<Chrome> {
        let Some(executable) = chrome_executable() else {
            eprintln!("no Chrome on this machine; set ORB_CHROME");
            return None;
        };
        Some(Chrome::launch(&executable).expect("Chrome launches"))
    }

    #[test]
    fn a_page_is_driven_and_read_through_devtools() {
        let Some(mut chrome) = chrome() else { return };
        chrome
            .navigate("data:text/html,<p id=p>hi</p><script>window.answer = 41 + 1; window.later = new Promise(r => setTimeout(() => r('done'), 50));</script>", Duration::from_secs(10))
            .unwrap();
        assert_eq!(chrome.evaluate("window.answer").unwrap(), json!(42));
        assert_eq!(
            chrome.evaluate("window.later").unwrap(),
            json!("done"),
            "a promise is awaited"
        );
        assert_eq!(
            chrome
                .evaluate("document.getElementById('p').textContent")
                .unwrap(),
            json!("hi")
        );
        chrome
            .wait_for("window.answer === 42", Duration::from_secs(1))
            .unwrap();
        assert!(
            chrome
                .screenshot()
                .unwrap()
                .starts_with(&[0x89, b'P', b'N', b'G']),
            "a PNG"
        );
    }

    #[test]
    fn control_a_thrown_error_and_an_unmet_wait_are_errors() {
        let Some(mut chrome) = chrome() else { return };
        chrome
            .navigate("data:text/html,<p>x</p>", Duration::from_secs(10))
            .unwrap();
        let thrown = chrome
            .evaluate("(() => { throw new Error('boom') })()")
            .unwrap_err()
            .to_string();
        assert!(thrown.contains("boom"), "{thrown}");
        assert!(chrome
            .wait_for("false", Duration::from_millis(200))
            .is_err());
    }
}
