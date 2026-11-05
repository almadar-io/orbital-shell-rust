use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_number, get_string, get_string_or, icon_name_to_glyph, merge_class_name,
};
use orbital_core::{Value, ValueMap};

pub struct ScoreDisplayRenderer;

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
        let class = merge_class_name("almadar-score-display", props);
        let size = get_string_or(props, "size", "md");
        let font_size = match size.as_str() {
            "sm" => "14px",
            "lg" => "24px",
            "xl" => "36px",
            _ => "18px",
        };
        let label = get_string(props, "label");
        let value = get_string(props, "displayValue").unwrap_or_else(|| {
            get_number(props, "value")
                .or_else(|| get_number(props, "score"))
                .unwrap_or(0.0)
                .to_string()
        });
        let asset = props.as_object().and_then(|p| p.get("assetUrl"));
        let icon = if let Some(asset) = asset {
            let mut spec = ValueMap::new();
            spec.insert("type".into(), Value::String("atlas-image".into()));
            spec.insert("asset".into(), asset.clone());
            spec.insert("size".into(), Value::Number(20.0));
            registry.render_spec(&Value::Object(spec.into()), entity, view)
        } else if let Some(name) = get_string(props, "icon") {
            let glyph = icon_name_to_glyph(&name);
            rsx! { span { class: "almadar-score-display__icon", "{glyph}" } }
        } else {
            rsx! {}
        };
        rsx! {
            div { class: "{class}", style: "display:flex;align-items:center;gap:8px;font-weight:700;font-size:{font_size}",
                ..shared_root_attrs(props),
                {icon}
                if let Some(label) = label { span { class: "almadar-score-display__label almadar-muted", "{label}" } }
                span { class: "almadar-score-display__value", style: "font-variant-numeric:tabular-nums", "{value}" }
            }
        }
    }
}
