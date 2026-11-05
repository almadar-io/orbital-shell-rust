//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `spacer` pattern.
pub struct SpacerRenderer;

impl SpacerRenderer {
    const NAME: &str = "spacer";
}

impl DioxusPatternRenderer for SpacerRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        _view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let size = get_string_or(props, "size", "auto");
        let axis = get_string_or(props, "axis", "horizontal");
        let size_class = match size.as_str() {
            "xs" => "1",
            "sm" => "2",
            "md" => "4",
            "lg" => "6",
            "xl" => "8",
            "2xl" => "12",
            _ => "",
        };
        let base = if size == "auto" {
            "flex-1".to_string()
        } else if axis == "vertical" {
            format!("flex-shrink-0 h-{size_class}")
        } else {
            format!("flex-shrink-0 w-{size_class}")
        };
        let class = merge_class_name(&base, props);
        rsx! { div { class: "{class}", aria_hidden: "true", ..shared_root_attrs(props) } }
    }
}
