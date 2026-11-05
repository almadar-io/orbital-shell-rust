use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::merge_class_name;
use orbital_core::Value;

pub struct InputGroupRenderer;

fn addon(value: &Value, registry: &DioxusPatternRegistry, view: &DioxusView<'_>) -> Element {
    if let Some(text) = value.as_str() {
        let mut fields = orbital_core::ValueMap::new();
        fields.insert("type".into(), Value::string("typography"));
        fields.insert("variant".into(), Value::string("small"));
        fields.insert("content".into(), Value::string(text));
        return registry.render_spec(&Value::Object(fields.into()), None, view);
    }
    if value.get("type").and_then(Value::as_str).is_some() {
        return registry.render_spec(value, None, view);
    }
    rsx! {}
}

impl DioxusPatternRenderer for InputGroupRenderer {
    fn pattern_name(&self) -> &str {
        "input-group"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let Some(fields) = props.as_object() else {
            return rsx! {};
        };
        let left = fields
            .get("leftAddon")
            .filter(|value| !matches!(value, Value::Null) && value.as_str() != Some(""));
        let right = fields
            .get("rightAddon")
            .filter(|value| !matches!(value, Value::Null) && value.as_str() != Some(""));
        let mut input_fields = fields.clone();
        input_fields.insert("type".into(), Value::string("input"));
        input_fields.shift_remove("leftAddon");
        input_fields.shift_remove("rightAddon");
        let input_class = format!(
            "{} {}",
            if left.is_some() { "rounded-l-none" } else { "" },
            if right.is_some() {
                "rounded-r-none"
            } else {
                ""
            }
        );
        input_fields.insert("className".into(), Value::string(input_class));
        let input = Value::Object(input_fields.into());
        let class = merge_class_name("flex items-stretch", props);
        rsx! { div { class: "{class}",
            if let Some(left) = left { div { class: "flex items-center justify-center px-3 bg-muted border border-border text-foreground rounded-l-interactive border-r-0", {addon(left, registry, view)} } }
            div { class: "flex-1", {registry.render_spec(&input, None, view)} }
            if let Some(right) = right { div { class: "flex items-center justify-center px-3 bg-muted border border-border text-foreground rounded-r-interactive border-l-0", {addon(right, registry, view)} } }
        } }
    }
}
