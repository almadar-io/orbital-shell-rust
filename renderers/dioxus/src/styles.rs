//! The stylesheets this renderer paints with, in cascade order: the
//! `@almadar/ui` theme bundle, the structural base, the Tailwind utilities.
//! Generated assets, refreshed by `@almadar/ui-wasm`'s `sync-dioxus-css`.

const THEME: &str = include_str!("../assets/css/almadar-theme.css");
const BASE: &str = include_str!("../assets/css/almadar.css");
const UTILITIES: &str = include_str!("../assets/css/tailwind.css");

pub fn stylesheets() -> Vec<String> {
    vec![THEME.to_string(), BASE.to_string(), UTILITIES.to_string()]
}
