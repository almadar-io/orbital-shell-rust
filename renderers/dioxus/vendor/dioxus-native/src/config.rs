use dioxus_core::LaunchConfig;
use winit::window::WindowAttributes;

/// The configuration for the desktop application.
pub struct Config {
    pub(crate) window_attributes: WindowAttributes,
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
}
