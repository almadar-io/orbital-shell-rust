use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_bool_or, get_number_or, get_string, get_string_or, };
use orbital_core::{Value, ValueMap};

pub struct TimerDisplayRenderer;

impl DioxusPatternRenderer for TimerDisplayRenderer {
    fn pattern_name(&self) -> &str {
        "timer-display"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let class = merge_class_name("almadar-timer-display", props);
        let size = get_string_or(props, "size", "md");
        let (font_size, padding) = match size.as_str() {
            "sm" => ("14px", "2px 8px"),
            "lg" => ("24px", "6px 16px"),
            _ => ("18px", "4px 12px"),
        };
        let seconds = get_number_or(props, "seconds", 90.0);
        let running = get_bool_or(props, "running", true);
        let threshold = get_number_or(props, "lowThreshold", 0.0);
        let low = threshold > 0.0 && seconds > 0.0 && seconds <= threshold;
        let text = get_string(props, "displayText").unwrap_or_else(|| seconds.to_string());
        let border_color = if low {
            "var(--color-error)"
        } else if running {
            "var(--color-success)"
        } else {
            "var(--almadar-line)"
        };
        let color = if low {
            "var(--color-error)"
        } else {
            "var(--almadar-text)"
        };
        let icon = props.as_object().and_then(|p| p.get("iconAsset"));
        let icon = if let Some(asset) = icon {
            let mut spec = ValueMap::new();
            spec.insert("type".into(), Value::String("atlas-image".into()));
            spec.insert("asset".into(), asset.clone());
            spec.insert("size".into(), Value::Number(16.0));
            registry.render_spec(&Value::Object(spec.into()), entity, view)
        } else {
            rsx! {}
        };
        rsx! {
            div {
                class: "{class}",
                style: "display:inline-flex;align-items:center;justify-content:center;gap:4px;padding:{padding};border:1px solid {border_color};border-radius:var(--almadar-radius);font-family:monospace;font-weight:700;font-size:{font_size};color:{color};font-variant-numeric:tabular-nums",
                ..shared_root_attrs(props),
                {icon}
                "{text}"
            }
        }
    }
}
