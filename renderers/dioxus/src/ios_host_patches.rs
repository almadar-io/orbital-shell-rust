//! The vendored iOS window and loop patches. The iOS code compiles only for iOS, so these read its source.

fn body_of<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source.find(signature).expect("function present");
    let rest = &source[start..];
    let end = if signature.starts_with("    ") { "\n    }\n" } else { "\n}\n" };
    &rest[..rest.find(end).expect("function end")]
}

const WINDOW: &str = include_str!("../vendor/winit/src/platform_impl/ios/window.rs");
const APP_STATE: &str = include_str!("../vendor/winit/src/platform_impl/ios/app_state.rs");
const BLITZ_WINDOW: &str = include_str!("../vendor/blitz-shell/src/window.rs");

#[test]
fn every_ios_redraw_is_queued_and_never_left_to_set_needs_display() {
    let redraw = body_of(WINDOW, "    pub fn request_redraw(&self) {");
    assert!(redraw.contains("app_state::queue_gl_or_metal_redraw(mtm, self.window.clone());"));
    assert!(!redraw.contains("self.view.setNeedsDisplay()"));
    assert!(!redraw.contains("gl_or_metal_backed"));
}

#[test]
fn a_redraw_asked_between_or_during_redraw_passes_waits_for_the_next_pass() {
    let queue = body_of(APP_STATE, "pub(crate) fn queue_gl_or_metal_redraw(");
    assert!(queue.contains("this.next_pass_gpu_redraws.insert(window)"));
    assert!(queue.contains("this.waker.start()"));
    assert!(!queue.contains("bug!(\"unexpected state"));
    assert!(APP_STATE.contains("this.next_pass_gpu_redraws.extend(queued_gpu_redraws);"));
    assert!(!APP_STATE.contains("\"redraw queued while processing redraws\""));
    let cleared = body_of(APP_STATE, "    fn events_cleared_transition(&mut self) {");
    assert!(cleared.contains("if !self.next_pass_gpu_redraws.is_empty() {"));
    let wakeup = body_of(APP_STATE, "    fn wakeup_transition(&mut self)");
    assert!(wakeup.contains("mem::take(&mut self.next_pass_gpu_redraws)"));
}

#[test]
fn the_ios_keyboard_is_not_raised_when_the_window_opens() {
    let init = body_of(BLITZ_WINDOW, "    pub fn init(");
    assert!(init.contains("#[cfg(not(any(target_os = \"android\", target_os = \"ios\")))]\n        winit_window.set_ime_allowed(true);"));
    let sync = &BLITZ_WINDOW[BLITZ_WINDOW.find("#[cfg(target_os = \"ios\")]\n    fn sync_ime_to_focus").expect("iOS focus sync")..];
    let sync = &sync[..sync.find("\n    }\n").expect("end")];
    assert!(sync.contains("text_input_focused || self.canvas_input().is_some()"));
}
