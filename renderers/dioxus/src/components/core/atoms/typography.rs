//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    cn, format_display_value, passthrough_attrs, DioxusPatternRenderer, DioxusView,
};
use crate::styles::style_with_props;
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
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
            "h1" => "text-4xl heading-voice text-foreground",
            "h2" => "text-3xl heading-voice text-foreground",
            "h3" | "heading" => "text-2xl heading-voice text-foreground",
            "h4" => "text-xl heading-voice text-foreground",
            "h5" | "subheading" => "text-lg heading-voice text-foreground",
            "h6" => "text-base heading-voice text-foreground",
            "caption" => "text-xs font-normal text-muted-foreground",
            "body2" | "small" => "text-sm font-normal text-foreground",
            "large" => "text-lg font-medium text-foreground",
            "label" => "text-sm font-medium text-foreground",
            "overline" => "text-xs uppercase tracking-wide font-bold text-muted-foreground",
            _ => "text-base font-normal text-foreground",
        };
        let default_tag = match variant.as_str() {
            "h1" => "h1",
            "h2" | "heading" => "h2",
            "h3" | "subheading" => "h3",
            "h4" => "h4",
            "h5" => "h5",
            "h6" => "h6",
            "caption" | "overline" | "small" | "label" => "span",
            _ => "p",
        };
        let tag = get_string(props, "as")
            .filter(|tag| !tag.is_empty())
            .unwrap_or_else(|| default_tag.to_string());
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
        let truncate = get_bool_or(props, "truncate", false);
        let truncate_class = if truncate { "truncate min-w-0" } else { "" };
        let block_class = if truncate && tag == "span" {
            "block"
        } else {
            ""
        };
        let class = cn(&[
            variant_class,
            color_class,
            weight_class,
            size_class,
            align_class,
            truncate_class,
            block_class,
            overflow_class,
            &get_string_or(props, "className", ""),
        ]);
        let style = style_with_props("", props);
        let format = get_string(props, "format").filter(|format| format != "none");
        let attrs = passthrough_attrs(props, &[]);
        let children = props.get("children").filter(|children| !children.is_null());
        let body = match (children, props.get("content")) {
            (Some(children), _) => view.render_node(registry, children),
            (None, Some(content @ (Value::String(_) | Value::Int(_) | Value::Number(_)))) => {
                let text = format_display_value(content, format.as_deref());
                rsx! { "{text}" }
            }
            (None, Some(content)) if !content.is_null() => view.render_node(registry, content),
            _ => rsx! {},
        };
        macro_rules! element {
            ($tag:ident) => {
                rsx! { $tag { class: "{class}", style: "{style}", ..attrs, {body} } }
            };
        }
        match tag.as_str() {
            "h1" => element!(h1),
            "h2" => element!(h2),
            "h3" => element!(h3),
            "h4" => element!(h4),
            "h5" => element!(h5),
            "h6" => element!(h6),
            "p" => element!(p),
            "span" => element!(span),
            "label" => element!(label),
            "legend" => element!(legend),
            "a" => element!(a),
            "li" => element!(li),
            "ul" => element!(ul),
            "ol" => element!(ol),
            "strong" => element!(strong),
            "b" => element!(b),
            "em" => element!(em),
            "i" => element!(i),
            "u" => element!(u),
            "small" => element!(small),
            "code" => element!(code),
            "pre" => element!(pre),
            "blockquote" => element!(blockquote),
            "figcaption" => element!(figcaption),
            "dt" => element!(dt),
            "dd" => element!(dd),
            "th" => element!(th),
            "td" => element!(td),
            "time" => element!(time),
            "section" => element!(section),
            "article" => element!(article),
            "header" => element!(header),
            "footer" => element!(footer),
            "nav" => element!(nav),
            "main" => element!(main),
            "aside" => element!(aside),
            _ => element!(div),
        }
    }
}
