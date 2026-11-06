//! Native body of the `score-display` pattern, mirroring the web `ScoreDisplay` atom.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{merge_class_name, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_number, get_string, get_string_or};
use orbital_core::{Value, ValueMap};

pub struct ScoreDisplayRenderer;

fn grouped(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let rounded = format!("{:.3}", value.abs());
    let (whole, fraction) = rounded.split_once('.').unwrap_or((rounded.as_str(), ""));
    let mut digits = String::new();
    for (index, ch) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index) % 3 == 0 {
            digits.push(',');
        }
        digits.push(ch);
    }
    let fraction = fraction.trim_end_matches('0');
    let sign = if value < 0.0 && (digits != "0" || !fraction.is_empty()) {
        "-"
    } else {
        ""
    };
    if fraction.is_empty() {
        format!("{sign}{digits}")
    } else {
        format!("{sign}{digits}.{fraction}")
    }
}

impl DioxusPatternRenderer for ScoreDisplayRenderer {
    fn pattern_name(&self) -> &str {
        "score-display"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let size = get_string_or(props, "size", "md");
        let size_class = match size.as_str() {
            "sm" => "text-sm",
            "lg" => "text-2xl",
            "xl" => "text-4xl",
            _ => "text-lg",
        };
        let class = merge_class_name(
            &format!(
                "bg-transparent rounded-none shadow-none flex items-center gap-2 font-bold {size_class}"
            ),
            props,
        );
        let label = get_string(props, "label").filter(|label| !label.is_empty());
        let value = get_string(props, "displayValue").unwrap_or_else(|| {
            grouped(
                get_number(props, "value")
                    .filter(|n| !n.is_nan())
                    .or_else(|| get_number(props, "score").filter(|n| !n.is_nan()))
                    .unwrap_or(0.0),
            )
        });
        let asset = props.as_object().and_then(|p| p.get("assetUrl"));
        let icon = if let Some(asset) = asset {
            let mut spec = ValueMap::new();
            spec.insert("type".into(), Value::String("atlas-image".into()));
            spec.insert("asset".into(), asset.clone());
            spec.insert("size".into(), Value::Number(20.0));
            let image = registry.render_spec(&Value::Object(spec.into()), entity, view);
            rsx! { span { class: "flex-shrink-0", {image} } }
        } else if let Some(name) = get_string(props, "icon").filter(|name| !name.is_empty()) {
            let spec = pattern_spec("icon", [("name", Value::string(name))]);
            let image = registry.render_spec(&spec, entity, view);
            rsx! { span { class: "flex-shrink-0", {image} } }
        } else {
            rsx! {}
        };
        let label_node = label.map(|label| {
            let spec = pattern_spec(
                "typography",
                [
                    ("as", Value::string("span")),
                    ("className", Value::string("text-muted-foreground")),
                    ("content", Value::string(label)),
                ],
            );
            registry.render_spec(&spec, entity, view)
        });
        let value_spec = pattern_spec(
            "typography",
            [
                ("as", Value::string("span")),
                ("className", Value::string("tabular-nums")),
                ("content", Value::string(value)),
            ],
        );
        let value_node = registry.render_spec(&value_spec, entity, view);
        rsx! {
            div { class: "{class}", ..shared_root_attrs(props),
                {icon}
                if let Some(node) = label_node { {node} }
                {value_node}
            }
        }
    }
}
