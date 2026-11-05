//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use crate::styles::style_with_props;
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `stack` pattern.
pub struct StackRenderer;

impl StackRenderer {
    const NAME: &str = "stack";
}

impl DioxusPatternRenderer for StackRenderer {
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
        let direction = get_string_or(props, "direction", "vertical");
        let gap = get_string_or(props, "gap", "md");
        let align = get_string_or(props, "align", "stretch");
        let justify = get_string_or(props, "justify", "start");
        let reverse = get_bool_or(props, "reverse", false);
        let responsive = get_bool_or(props, "responsive", false);
        let direction_class = match (direction.as_str(), reverse, responsive) {
            ("horizontal", true, true) => "flex-col-reverse md:flex-row-reverse",
            ("horizontal", false, true) => "flex-col md:flex-row",
            ("horizontal", true, false) => "flex-row-reverse",
            ("horizontal", false, false) => "flex-row",
            (_, true, _) => "flex-col-reverse",
            _ => "flex-col",
        };
        let gap_class = match gap.as_str() {
            "none" => "gap-0",
            "xs" => "gap-1",
            "sm" => "gap-2",
            "lg" => "gap-6",
            "xl" => "gap-8",
            "2xl" => "gap-12",
            _ => "gap-4",
        };
        let align_class = match align.as_str() {
            "start" => "items-start",
            "center" => "items-center",
            "end" => "items-end",
            "baseline" => "items-baseline",
            _ => "items-stretch",
        };
        let justify_class = match justify.as_str() {
            "center" => "justify-center",
            "end" => "justify-end",
            "between" => "justify-between",
            "around" => "justify-around",
            "evenly" => "justify-evenly",
            _ => "justify-start",
        };
        let wrap_class = if get_bool_or(props, "wrap", false) {
            "flex-wrap"
        } else {
            ""
        };
        let flex_class = if get_bool_or(props, "flex", false) {
            "flex-1"
        } else {
            ""
        };
        let base = format!("flex {direction_class} {gap_class} {align_class} {justify_class} {wrap_class} {flex_class}");
        let class = merge_class_name(&base, props);
        let style = style_with_props("height:auto", props);
        let action = get_string(props, "action");
        let action_payload = props
            .as_object()
            .and_then(|fields| fields.get("actionPayload"))
            .cloned()
            .unwrap_or_else(|| Value::Object(orbital_core::ValueMap::new().into()));
        let sink = view.sink;
        let children = child_patterns(props, view.trait_content);
        rsx! { div { class: "{class}", style: "{style}",
            onclick: move |_| {
                if let (Some(sink), Some(action)) = (sink, &action) {
                    sink.call((action.clone(), action_payload.clone()));
                }
            },
            ..shared_root_attrs(props),
            for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
        } }
    }
}
