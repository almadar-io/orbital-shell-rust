//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `status-dot` pattern.
pub struct StatusDotRenderer;

pub(crate) fn color_class(status: &str) -> &'static str {
    match status {
        "online" => "bg-success",
        "away" | "warning" => "bg-warning",
        "busy" | "critical" => "bg-error",
        _ => "bg-muted-foreground",
    }
}

impl StatusDotRenderer {
    const NAME: &str = "status-dot";
}

impl DioxusPatternRenderer for StatusDotRenderer {
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
        let status = get_string_or(props, "status", "offline");
        let size = get_string_or(props, "size", "md");
        let pulse = get_bool_or(props, "pulse", false);
        let color = color_class(&status);
        let ring = match status.as_str() {
            "online" => "ring-success",
            "away" | "warning" => "ring-warning",
            "busy" | "critical" => "ring-error",
            _ => "ring-muted-foreground",
        };
        let dimensions = match size.as_str() {
            "sm" => "w-2 h-2",
            "lg" => "w-3 h-3",
            _ => "w-2.5 h-2.5",
        };
        let motion = if pulse {
            "animate-pulse ring-2 ring-offset-1 ring-opacity-40"
        } else {
            ""
        };
        let base =
            format!("inline-block rounded-full flex-shrink-0 {color} {ring} {dimensions} {motion}");
        let class = merge_class_name(&base, props);
        let label = get_string_or(props, "label", &status);
        rsx! { span { class: "{class}", role: "status", aria_label: "{label}", ..crate::registry::shared_root_attrs(props) } }
    }
}
