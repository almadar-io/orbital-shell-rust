//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `divider` pattern.
pub struct DividerRenderer;

impl DividerRenderer {
    const NAME: &str = "divider";
}

impl DioxusPatternRenderer for DividerRenderer {
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
        let orientation = get_string_or(props, "orientation", "horizontal");
        let variant = get_string_or(props, "variant", "solid");
        let label = get_string_or(props, "label", "");
        let border = match variant.as_str() {
            "dashed" => "border-dashed",
            "dotted" => "border-dotted",
            _ => "border-solid",
        };
        if orientation == "vertical" {
            let class = merge_class_name(
                &format!("w-0 h-full border-l border-border {border}"),
                props,
            );
            return rsx! { div { class: "{class}", role: "separator", aria_orientation: "vertical", ..shared_root_attrs(props) } };
        }
        if !label.is_empty() {
            let class = merge_class_name("flex items-center gap-3 my-4", props);
            let rule_class = format!("flex-1 h-0 border-t border-border {border}");
            return rsx! { div { class: "{class}", role: "separator", aria_label: "{label}", ..shared_root_attrs(props),
                div { class: "{rule_class}" }
                span { class: "text-sm text-foreground font-bold uppercase tracking-wide", "{label}" }
                div { class: "{rule_class}" }
            } };
        }
        let class = merge_class_name(
            &format!("w-full h-0 border-t border-border my-4 {border}"),
            props,
        );
        rsx! { div { class: "{class}", role: "separator", aria_orientation: "horizontal", ..shared_root_attrs(props) } }
    }
}
