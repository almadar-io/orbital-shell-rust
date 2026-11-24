//! The mobile native-gpu hosts' sign-in bridge (owner 2026-11-23). The Android template's `OrbitalSignIn.java`
//! (Credential Manager + Firebase Auth) and the iOS template's `OrbitalSignIn.m` (GoogleSignIn + Firebase Auth) sign
//! in with the platform SDKs, so the token is the Firebase ID token the web host sends. Every outcome and token
//! refresh arrives on the platform's thread; each is queued and the layout woken, so the Dioxus loop drains it on its
//! own thread, where the client kernel lives.

#![cfg(any(target_os = "android", target_os = "ios"))]

use std::collections::VecDeque;
use std::sync::Mutex;

/// What the platform side reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInEvent {
    /// The current ID token (a sign-in or a refresh), or `None` once nobody is signed in.
    Token(Option<String>),
    SignedIn { uid: String, email: Option<String>, display_name: Option<String>, id_token: String },
    Failed { kind: &'static str, message: String },
    SignedOut,
}

static EVENTS: Mutex<VecDeque<SignInEvent>> = Mutex::new(VecDeque::new());

fn push(event: SignInEvent) {
    if let Ok(mut events) = EVENTS.lock() {
        events.push_back(event);
    }
    dioxus_native::wake_layout();
}

/// Everything reported since the last call, in order.
pub fn drain() -> Vec<SignInEvent> {
    EVENTS.lock().map(|mut events| events.drain(..).collect()).unwrap_or_default()
}

fn kind_of(kind: &str) -> &'static str {
    match kind {
        "cancelled" => "cancelled",
        "denied" => "denied",
        "invalid" => "invalid",
        _ => "unavailable",
    }
}

fn signed_in(uid: Option<String>, email: Option<String>, display_name: Option<String>, id_token: Option<String>) {
    match (uid, id_token) {
        (Some(uid), Some(id_token)) => push(SignInEvent::SignedIn { uid, email, display_name, id_token }),
        _ => push(SignInEvent::Failed { kind: "unavailable", message: "the signed-in user has no id or token".into() }),
    }
}

pub use platform::{request_sign_in, request_sign_out};

#[cfg(target_os = "android")]
mod platform {
    use super::{kind_of, push, signed_in, SignInEvent};
    use std::sync::OnceLock;

    static CLASS: OnceLock<(jni::JavaVM, jni::objects::GlobalRef)> = OnceLock::new();

    fn call(method: &str) -> Result<(), String> {
        let (vm, class) = CLASS.get().ok_or("sign-in JNI not installed")?;
        let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
        env.call_static_method(class, method, "()V", &[]).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Start a Google sign-in; its outcome arrives as [`SignInEvent::SignedIn`] or [`SignInEvent::Failed`].
    pub fn request_sign_in() -> Result<(), String> {
        call("signIn")
    }

    /// Sign out; arrives as [`SignInEvent::SignedOut`].
    pub fn request_sign_out() -> Result<(), String> {
        call("signOut")
    }

    fn text(env: &mut jni::JNIEnv<'_>, value: &jni::objects::JString<'_>) -> Option<String> {
        if value.is_null() {
            return None;
        }
        env.get_string(value).ok().map(String::from).filter(|s| !s.is_empty())
    }

    #[no_mangle]
    pub extern "system" fn Java_com_google_androidgamesdk_OrbitalSignIn_nativeInstall(
        mut env: jni::JNIEnv<'_>,
        class: jni::objects::JClass<'_>,
    ) {
        match env.get_java_vm().and_then(|vm| env.new_global_ref(class).map(|reference| (vm, reference))) {
            Ok(value) => {
                let _ = CLASS.set(value);
            }
            Err(error) => tracing::error!(%error, "sign-in JNI installation failed"),
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_google_androidgamesdk_OrbitalSignIn_nativeToken(
        mut env: jni::JNIEnv<'_>,
        _class: jni::objects::JClass<'_>,
        id_token: jni::objects::JString<'_>,
    ) {
        push(SignInEvent::Token(text(&mut env, &id_token)));
    }

    #[no_mangle]
    pub extern "system" fn Java_com_google_androidgamesdk_OrbitalSignIn_nativeSignedIn(
        mut env: jni::JNIEnv<'_>,
        _class: jni::objects::JClass<'_>,
        uid: jni::objects::JString<'_>,
        email: jni::objects::JString<'_>,
        display_name: jni::objects::JString<'_>,
        id_token: jni::objects::JString<'_>,
    ) {
        let (uid, id_token) = (text(&mut env, &uid), text(&mut env, &id_token));
        signed_in(uid, text(&mut env, &email), text(&mut env, &display_name), id_token);
    }

    #[no_mangle]
    pub extern "system" fn Java_com_google_androidgamesdk_OrbitalSignIn_nativeSignInFailed(
        mut env: jni::JNIEnv<'_>,
        _class: jni::objects::JClass<'_>,
        kind: jni::objects::JString<'_>,
        message: jni::objects::JString<'_>,
    ) {
        let kind = kind_of(&text(&mut env, &kind).unwrap_or_default());
        let message = text(&mut env, &message).unwrap_or_else(|| "sign-in did not finish".into());
        push(SignInEvent::Failed { kind, message });
    }

    #[no_mangle]
    pub extern "system" fn Java_com_google_androidgamesdk_OrbitalSignIn_nativeSignedOut(
        _env: jni::JNIEnv<'_>,
        _class: jni::objects::JClass<'_>,
    ) {
        push(SignInEvent::SignedOut);
    }
}

#[cfg(target_os = "ios")]
mod platform {
    use super::{kind_of, push, signed_in, SignInEvent};
    use std::ffi::{c_char, CStr};

    extern "C" {
        fn orbital_sign_in_start();
        fn orbital_sign_in_sign_out();
    }

    /// Start a Google sign-in; its outcome arrives as [`SignInEvent::SignedIn`] or [`SignInEvent::Failed`].
    pub fn request_sign_in() -> Result<(), String> {
        // SAFETY: the template's OrbitalSignIn.m (or its OrbitalSignInOff.m stub) defines it; it takes no arguments.
        unsafe { orbital_sign_in_start() };
        Ok(())
    }

    /// Sign out; arrives as [`SignInEvent::SignedOut`].
    pub fn request_sign_out() -> Result<(), String> {
        // SAFETY: as above.
        unsafe { orbital_sign_in_sign_out() };
        Ok(())
    }

    fn text(value: *const c_char) -> Option<String> {
        if value.is_null() {
            return None;
        }
        // SAFETY: the Objective-C side passes a NUL-terminated UTF-8 string that lives for the call.
        let text = unsafe { CStr::from_ptr(value) }.to_string_lossy().into_owned();
        (!text.is_empty()).then_some(text)
    }

    #[no_mangle]
    pub extern "C" fn orbital_sign_in_token(id_token: *const c_char) {
        push(SignInEvent::Token(text(id_token)));
    }

    #[no_mangle]
    pub extern "C" fn orbital_sign_in_signed_in(uid: *const c_char, email: *const c_char, display_name: *const c_char, id_token: *const c_char) {
        signed_in(text(uid), text(email), text(display_name), text(id_token));
    }

    #[no_mangle]
    pub extern "C" fn orbital_sign_in_failed(kind: *const c_char, message: *const c_char) {
        let kind = kind_of(&text(kind).unwrap_or_default());
        push(SignInEvent::Failed { kind, message: text(message).unwrap_or_else(|| "sign-in did not finish".into()) });
    }

    #[no_mangle]
    pub extern "C" fn orbital_sign_in_signed_out() {
        push(SignInEvent::SignedOut);
    }
}
