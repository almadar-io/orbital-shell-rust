//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

type IconNode = (String, BTreeMap<String, String>);

static LUCIDE_NODES: LazyLock<HashMap<String, Vec<IconNode>>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../../../assets/lucide-nodes.json"))
        .expect("pinned Lucide nodes must be valid JSON")
});
static LUCIDE_ALIASES: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../../../assets/lucide-aliases.json"))
        .expect("generated web Lucide aliases must be valid JSON")
});
static LUCIDE_PASCAL: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    LUCIDE_NODES
        .keys()
        .map(|name| {
            let pascal = name
                .split('-')
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<String>();
            (pascal, name.clone())
        })
        .collect()
});

fn icon_nodes(name: &str) -> &'static [IconNode] {
    let canonical = LUCIDE_ALIASES
        .get(name)
        .or_else(|| LUCIDE_PASCAL.get(name))
        .map(String::as_str)
        .unwrap_or(name);
    LUCIDE_NODES
        .get(canonical)
        .or_else(|| LUCIDE_NODES.get("circle-question-mark"))
        .expect("pinned Lucide nodes must contain the fallback icon")
}

fn icon_node_attrs(values: &BTreeMap<String, String>) -> Vec<Attribute> {
    values
        .iter()
        .map(|(name, value)| {
            let name: &'static str = match name.as_str() {
                "cx" => "cx",
                "cy" => "cy",
                "d" => "d",
                "fill" => "fill",
                "height" => "height",
                "points" => "points",
                "r" => "r",
                "rx" => "rx",
                "ry" => "ry",
                "width" => "width",
                "x" => "x",
                "x1" => "x1",
                "x2" => "x2",
                "y" => "y",
                "y1" => "y1",
                "y2" => "y2",
                _ => unreachable!("Lucide asset has an unsupported SVG attribute"),
            };
            Attribute::new(name, value.clone(), None, false)
        })
        .collect()
}

fn render_icon_node((tag, values): &IconNode) -> Element {
    let attrs = icon_node_attrs(values);
    match tag.as_str() {
        "path" => rsx! { path { ..attrs } },
        "circle" => rsx! { circle { ..attrs } },
        "rect" => rsx! { rect { ..attrs } },
        "polyline" => rsx! { polyline { ..attrs } },
        "line" => rsx! { line { ..attrs } },
        "polygon" => rsx! { polygon { ..attrs } },
        "ellipse" => rsx! { ellipse { ..attrs } },
        _ => unreachable!("Lucide asset has an unsupported SVG element"),
    }
}

/// Dioxus renderer for the `icon` pattern.
pub struct IconRenderer;

impl IconRenderer {
    const NAME: &str = "icon";
}

impl DioxusPatternRenderer for IconRenderer {
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
        let icon = get_string_or(props, "icon", "");
        let name = if icon.is_empty() {
            get_string_or(props, "name", "")
        } else {
            icon
        };
        let nodes = icon_nodes(&name);
        let size = match get_string_or(props, "size", "md").as_str() {
            "xs" => "w-3 h-3",
            "sm" => "w-4 h-4",
            "lg" => "w-6 h-6",
            "xl" => "w-8 h-8",
            _ => "h-icon-default w-icon-default",
        };
        let color = match get_string_or(props, "color", "").as_str() {
            "primary" => "text-primary".to_string(),
            "secondary" => "text-secondary".to_string(),
            "success" => "text-success".to_string(),
            "warning" => "text-warning".to_string(),
            "error" => "text-error".to_string(),
            "muted" => "text-muted-foreground".to_string(),
            "" => "text-current".to_string(),
            other => other.to_string(),
        };
        let animation = match get_string_or(props, "animation", "none").as_str() {
            "spin" => "animate-spin",
            "pulse" => "animate-pulse",
            _ => "",
        };
        let stroke_width = get_number(props, "strokeWidth")
            .filter(|width| *width > 0.0)
            .map(|width| width.to_string())
            .unwrap_or_else(|| "var(--icon-stroke-width, 2)".to_string());
        let base = format!("{size} {color} {animation}");
        let class = merge_class_name(&base, props);
        let style = crate::styles::style_with_props("", props);
        rsx! { svg {
            class: "{class}",
            style: "{style}",
            width: "24",
            height: "24",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "{stroke_width}",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            ..crate::registry::shared_root_attrs(props),
            for node in nodes { {render_icon_node(node)} }
        } }
    }
}
