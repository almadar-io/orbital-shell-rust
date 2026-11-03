//! Dioxus renderer for the `guest-view` pattern: paints the guest program's children only.
//! `guest`, `source` and `deliver` are routing data the kernel reads; the renderer never shows or emits them.

use dioxus::prelude::*;
use orbital_core::Value;
use orbital_core::client::recurse::*;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use crate::registry::DioxusPatternRegistry;

/// Dioxus renderer for the `guest-view` pattern.
pub struct GuestViewRenderer;

impl GuestViewRenderer {
    const NAME: &str = "guest-view";
}

impl DioxusPatternRenderer for GuestViewRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let children = child_patterns(props, view.trait_content);
        rsx! { div { style: "width:100%;height:100%", for (child, _src) in children.iter() { {registry.render_spec(child, None, view)} } } }
    }
}
