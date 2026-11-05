use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_number, get_string, get_string_or, icon_name_to_glyph, };
use orbital_core::{Value, ValueMap};

pub struct StatBadgeRenderer;

impl DioxusPatternRenderer for StatBadgeRenderer {
    fn pattern_name(&self) -> &str {
        "stat-badge"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let class = merge_class_name("almadar-stat-badge", props);
        let label = get_string_or(props, "label", "");
        let size = get_string_or(props, "size", "md");
        let variant = get_string_or(props, "variant", "default");
        let format = get_string_or(props, "format", "number");
        let padding = match size.as_str() {
            "xs" => "2px 6px",
            "sm" => "4px 8px",
            "lg" => "8px 16px",
            _ => "6px 12px",
        };
        let color = match variant.as_str() {
            "primary" => "var(--color-primary)",
            "success" => "var(--color-success)",
            "warning" => "var(--color-warning)",
            "danger" => "var(--color-error)",
            _ => "var(--almadar-line)",
        };
        let value = props.as_object().and_then(|p| p.get("value"));
        let number = value
            .and_then(|value| match value {
                Value::Number(number) => Some(*number),
                Value::String(text) => text.parse::<f64>().ok(),
                _ => None,
            })
            .unwrap_or(0.0);
        let max = get_number(props, "max");
        let icon_asset = props
            .as_object()
            .and_then(|p| p.get("iconUrl").or_else(|| p.get("assetUrl")));
        let icon = if let Some(asset) = icon_asset {
            let mut spec = ValueMap::new();
            spec.insert("type".into(), Value::String("atlas-image".into()));
            spec.insert("asset".into(), asset.clone());
            spec.insert("size".into(), Value::Number(16.0));
            registry.render_spec(&Value::Object(spec.into()), entity, view)
        } else if let Some(name) = get_string(props, "icon") {
            let glyph = icon_name_to_glyph(&name);
            rsx! { span { class: "almadar-stat-badge__icon", "{glyph}" } }
        } else {
            rsx! {}
        };
        let display = if value.is_none() {
            rsx! {}
        } else if (format == "hearts" || format == "bar") && max.is_some() {
            let mut spec = ValueMap::new();
            spec.insert("type".into(), Value::String("health-bar".into()));
            spec.insert("current".into(), Value::Number(number));
            spec.insert("max".into(), Value::Number(max.unwrap_or(0.0)));
            spec.insert("format".into(), Value::String(format.into()));
            spec.insert(
                "size".into(),
                Value::String(if size == "lg" { "md" } else { "sm" }.into()),
            );
            registry.render_spec(&Value::Object(spec.into()), entity, view)
        } else if format == "number" {
            let mut spec = ValueMap::new();
            spec.insert("type".into(), Value::String("score-display".into()));
            spec.insert("value".into(), Value::Number(number));
            if let Some(text) = get_string(props, "displayValue") {
                spec.insert("displayValue".into(), Value::String(text.into()));
            }
            spec.insert(
                "size".into(),
                Value::String(if size == "lg" { "md" } else { "sm" }.into()),
            );
            registry.render_spec(&Value::Object(spec.into()), entity, view)
        } else if format == "text" {
            let text = value.map(Value::to_string_value).unwrap_or_default();
            rsx! { span { class: "almadar-stat-badge__text", "{text}" } }
        } else {
            rsx! {}
        };
        rsx! {
            div { class: "{class}", style: "display:inline-flex;align-items:center;gap:8px;padding:{padding};border:1px solid {color};border-radius:var(--almadar-radius);background:var(--almadar-bg)",
                ..shared_root_attrs(props),
                {icon}
                span { class: "almadar-stat-badge__label almadar-muted", "{label}" }
                {display}
            }
        }
    }
}
