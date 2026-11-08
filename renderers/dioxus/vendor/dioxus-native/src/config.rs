use dioxus_core::LaunchConfig;
use winit::window::WindowAttributes;

/// The configuration for the desktop application.
pub struct Config {
    pub(crate) window_attributes: WindowAttributes,
    // VENDORED FIX: the window takes the host's font context, so a native window falls back to the same
    // embedded fonts as the offscreen capture instead of whatever the OS ships.
    pub(crate) font_ctx: Option<blitz_dom::FontContext>,
}

impl LaunchConfig for Config {}

impl Default for Config {
    fn default() -> Self {
        // VENDORED FIX (orbital-shell-rust/renderers/dioxus/vendor): upstream 0.7.9 calls
        // `dioxus_cli_config::app_title()` here without a feature guard, but
        // the dep is optional (`hot-reload`) — with default features off
        // (required for wasm32, where tokio/devtools don't build) the crate
        // fails to compile. `hot-reload` itself is also dead on wasm
        // (`dioxus_devtools::connect` is non-wasm), so the only correct fix
        // is in source. Remove this patch once upstream guards the call.
        #[cfg(feature = "hot-reload")]
        let title = dioxus_cli_config::app_title().unwrap_or_else(|| "Dioxus App".to_string());
        #[cfg(not(feature = "hot-reload"))]
        let title = "Dioxus App".to_string();
        Self {
            window_attributes: WindowAttributes::default().with_title(title),
            font_ctx: None,
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the configuration for the window.
    pub fn with_window_attributes(mut self, attrs: WindowAttributes) -> Self {
        // We need to do a swap because the window builder only takes itself as muy self
        self.window_attributes = attrs;
        self
    }

    /// Lay text out with this font context (its registered fonts and generic families).
    pub fn with_font_context(mut self, font_ctx: blitz_dom::FontContext) -> Self {
        self.font_ctx = Some(font_ctx);
        self
    }
}
