//! Native (Blitz) mount surface for the Dioxus backend — the pivot away from
//! DOM rendering. The SAME schema pipeline, kernel circuit (`KernelBridge` +
//! `ClientCore`), pattern registry, and root `App` component as the web
//! shell; only the paint layer differs: `dioxus-native` (Blitz: Stylo CSS +
//! Taffy layout + Parley text + wgpu/vello paint) instead of browser DOM.
//!
//! CSS arrives through `document::Style` head elements (see `app.rs`
//! `CSS_TEXT`) — Blitz resolves every `class:` attribute (including the
//! `merge_class_name` output) against them, so the Tailwind build from
//! `@almadar/ui-wasm` works unchanged.

#![cfg(all(feature = "native", not(target_arch = "wasm32")))]

use crate::app::{App, CSS_TEXT};
use crate::shell;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "native-http")]
use std::sync::Arc;

static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);
static MOTION_CHANGED: AtomicBool = AtomicBool::new(false);

pub fn reduced_motion() -> bool {
    REDUCED_MOTION.load(Ordering::Relaxed)
}

pub fn set_reduced_motion(reduced: bool) {
    if REDUCED_MOTION.swap(reduced, Ordering::Relaxed) != reduced {
        MOTION_CHANGED.store(true, Ordering::Relaxed);
        dioxus_native::wake_layout();
    }
}

#[cfg(target_os = "macos")]
fn system_reduced_motion() -> bool {
    objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

#[cfg(target_os = "android")]
pub fn set_android_app(app: android_activity::AndroidApp) {
    blitz_shell::set_android_app(app);
}

#[cfg(target_os = "android")]
pub fn android_is_editable_at(x: f32, y: f32) -> bool {
    blitz_shell::android_is_editable_at(x, y)
}

/// Mount a schema and open the native window. Blocks on the winit event loop
/// until the window closes. `css_texts` are stylesheet contents (already read
/// from disk by the caller — no path knowledge here); `title` labels the
/// window.
pub fn start(schema_json: &str, css_texts: Vec<String>, title: &str) -> Result<(), String> {
    let (orbitals, traits) = shell::mount_core(schema_json)?;
    tracing::info!(target: shell::NS, orbitals, traits, "native mount");
    open_window(css_texts, title);
    Ok(())
}

/// Mount the same client circuit with a server bridge supplied by the host.
#[cfg(feature = "native-http")]
pub fn start_server(
    schema_json: &str,
    server_url: &str,
    css_texts: Vec<String>,
    title: &str,
) -> Result<(), String> {
    let schema = orbital_core::schema::parse_schema(schema_json)
        .map_err(|e| format!("parse schema: {e}"))?;
    let bridge = Arc::new(orbital_core::client::SyncServerBridge::new(server_url));
    let (orbitals, traits) = shell::mount_remote(schema, bridge)?;
    tracing::info!(target: shell::NS, orbitals, traits, "native server mount");
    open_window(css_texts, title);
    Ok(())
}

/// Open the window over the already-mounted app ([`shell::mount`]); returns when it closes.
pub fn open_window(css_texts: Vec<String>, title: &str) {
    tracing::info!(target: shell::NS, css_sheets = css_texts.len(), "native window");
    CSS_TEXT.with(|c| *c.borrow_mut() = css_texts);
    #[cfg(target_os = "macos")]
    set_reduced_motion(system_reduced_motion());

    let attrs = dioxus_native::WindowAttributes::default().with_title(title.to_string());
    let cfg = dioxus_native::Config::new().with_window_attributes(attrs);
    dioxus_native::set_layout_hook(Some(crate::video_overlay::sync_layout));
    dioxus_native::set_visibility_hook(Some(crate::visibility::sync_layout));
    // Game tick driver: the frame hook (vendored dioxus-native patch,
    // dioxus_application.rs) runs the kernel's tick clock through ClientCore on the vsync
    // cadence so `set`-effect loops advance without waiting on a user event.
    dioxus_native::set_frame_hook(Some(Box::new(|| {
        #[cfg(target_os = "macos")]
        set_reduced_motion(system_reduced_motion());
        if MOTION_CHANGED.swap(false, Ordering::Relaxed) {
            crate::app::poke_rerender();
        }
        #[cfg(feature = "native-http")]
        if crate::app::drive_remote_frame() {
            return;
        }
        if shell::drive_ticks() {
            crate::app::poke_rerender();
        }
    })));
    dioxus_native::launch_cfg(App, vec![], vec![Box::new(cfg)]);
    dioxus_native::set_frame_hook(None);
    dioxus_native::set_layout_hook(None);
    dioxus_native::set_visibility_hook(None);
}

/// Drive one event from outside a render (used by the `--events` CLI flag and
/// the future headless harness). Mirrors `web::send_event`.
pub fn send_event(event: &str, payload_json: &str) -> Result<(), String> {
    shell::drive_event(event, payload_json)?;
    crate::app::poke_rerender();
    Ok(())
}

/// Snapshot current render state as JSON — same shape as `web::snapshot`.
pub fn snapshot() -> Result<String, String> {
    shell::snapshot_json()
}

/// Current active state-machine state.
pub fn current_state() -> Result<String, String> {
    shell::current_state_string()
}
