//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use crate::styles::style_with_props;
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `typography` pattern.
pub struct TypographyRenderer;

impl TypographyRenderer {
    const NAME: &str = "typography";
}

impl DioxusPatternRenderer for TypographyRenderer {
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
        let level = props
            .as_object()
            .and_then(|fields| fields.get("level"))
            .and_then(Value::as_int);
        let variant = get_string(props, "variant").unwrap_or_else(|| {
            level
                .map(|level| format!("h{level}"))
                .unwrap_or_else(|| "body1".to_string())
        });
        let color = get_string_or(props, "color", "primary");
        let weight = get_string_or(props, "weight", "none");
        let variant_class = match variant.as_str() {
            "h1" => "text-4xl heading-voice",
            "h2" => "text-3xl heading-voice",
            "h3" | "heading" => "text-2xl heading-voice",
            "h4" => "text-xl heading-voice",
            "h5" | "subheading" => "text-lg heading-voice",
            "h6" => "text-base heading-voice",
            "caption" => "text-xs font-normal",
            "body2" | "small" => "text-sm font-normal",
            "large" => "text-lg font-medium",
            "label" => "text-sm font-medium",
            "overline" => "text-xs uppercase tracking-wide font-bold",
            _ => "text-base font-normal",
        };
        let color_class = match color.as_str() {
            "secondary" | "muted" => "text-muted-foreground",
            "error" => "text-error",
            "success" => "text-success",
            "warning" => "text-warning",
            "inherit" => "text-inherit",
            _ => "text-foreground",
        };
        let weight_class = match weight.as_str() {
            "semibold" => "font-semibold",
            "bold" => "font-bold",
            "medium" => "font-medium",
            "light" => "font-light",
            "normal" => "font-normal",
            _ => "",
        };
        let size_class = match get_string(props, "size").as_deref() {
            Some("xs") => "text-xs",
            Some("sm") => "text-sm",
            Some("md") => "text-base",
            Some("lg") => "text-lg",
            Some("xl") => "text-xl",
            Some("2xl") => "text-2xl",
            Some("3xl") => "text-3xl",
            _ => "",
        };
        let align_class = match get_string(props, "align").as_deref() {
            Some("left") => "text-left",
            Some("center") => "text-center",
            Some("right") => "text-right",
            _ => "",
        };
        let overflow_class = match get_string(props, "overflow").as_deref() {
            Some("visible") => "overflow-visible",
            Some("hidden") => "overflow-hidden min-w-0",
            Some("wrap") => "break-words overflow-hidden min-w-0",
            Some("clamp-2") => "overflow-hidden line-clamp-2 min-w-0",
            Some("clamp-3") => "overflow-hidden line-clamp-3 min-w-0",
            _ => "",
        };
        let truncate_class = if get_bool_or(props, "truncate", false) {
            "truncate min-w-0"
        } else {
            ""
        };
        let base = format!("{variant_class} {color_class} {weight_class} {size_class} {align_class} {overflow_class} {truncate_class}");
        let class = merge_class_name(&base, props);
        let style = style_with_props("", props);
        let text = props
            .as_object()
            .and_then(|fields| fields.get("content"))
            .and_then(|value| match value {
                Value::String(text) => Some(text.to_string()),
                Value::Int(number) => Some(number.to_string()),
                Value::Number(number) => Some(number.to_string()),
                _ => None,
            })
            .filter(|text| !text.is_empty());
        let children = child_patterns(props, view.trait_content);
        rsx! { div { class: "{class}", style: "{style}", ..shared_root_attrs(props), if let Some(t) = text { span { "{t}" } } for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} } } }
    }
}
