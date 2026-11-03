use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::avl_playback::{trait_level_from_description, AvlTraitDescription};
use orbital_core::client::helpers::{from_json, get_bool_or, get_string, to_json};
use orbital_core::{Value, ValueMap};

pub struct StateMachineViewRenderer;

impl DioxusPatternRenderer for StateMachineViewRenderer {
    fn pattern_name(&self) -> &str {
        "state-machine-view"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        if get_bool_or(props, "isLoading", false) {
            return rsx! { div { class: "almadar-loading-state", role: "status", "Loading…" } };
        }
        if let Some(error) = props.as_object().and_then(|obj| obj.get("error")) {
            if !matches!(error, Value::Null) {
                let message = get_string(error, "message")
                    .or_else(|| error.as_str().map(str::to_string))
                    .unwrap_or_else(|| to_json(error).to_string());
                return rsx! { div { class: "almadar-alert almadar-alert--error", role: "alert", "{message}" } };
            }
        }
        let trait_value = props
            .as_object()
            .and_then(|obj| obj.get("trait"))
            .filter(|value| !matches!(value, Value::Null))
            .cloned()
            .or_else(|| {
                let description = props.as_object()?.get("description")?;
                if matches!(description, Value::Null) {
                    return None;
                }
                let description: AvlTraitDescription =
                    serde_json::from_value(to_json(description)).ok()?;
                let level = trait_level_from_description(description).ok()??;
                serde_json::to_value(level).ok().map(from_json)
            });
        let Some(trait_value) = trait_value else {
            return rsx! { div { class: "almadar-state-machine-view-empty", "No state machine" } };
        };
        let mut spec = ValueMap::new();
        spec.insert("type".into(), Value::string("avl-state-machine"));
        spec.insert("trait".into(), trait_value);
        if let Some(object) = props.as_object() {
            for (key, value) in object {
                if !matches!(
                    key.as_str(),
                    "type" | "trait" | "description" | "isLoading" | "error"
                ) {
                    spec.insert(key.clone(), value.clone());
                }
            }
        }
        registry.render_spec(&Value::Object(spec.into()), entity, view)
    }
}
