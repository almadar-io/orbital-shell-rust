use crate::registry::{shared_root_attrs_with_defaults, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_bool_or, get_number, get_string, get_string_or, icon_name_to_glyph, merge_class_name,
};
use orbital_core::Value;

pub struct ChoiceButtonRenderer;

impl DioxusPatternRenderer for ChoiceButtonRenderer {
    fn pattern_name(&self) -> &str {
        "choice-button"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let class = merge_class_name("almadar-button almadar-choice-button", props);
        let text = get_string_or(props, "text", "");
        let index = get_number(props, "index");
        let asset = props
            .as_object()
            .and_then(|value| value.get("assetUrl"))
            .cloned();
        let asset_content = asset.map(|asset| {
            let mut spec = orbital_core::ValueMap::new();
            spec.insert("type".into(), Value::String("atlas-image".into()));
            spec.insert("asset".into(), asset);
            spec.insert("size".into(), Value::Number(16.0));
            registry.render_spec(&Value::Object(spec.into()), entity, view)
        });
        let icon = get_string(props, "icon").map(|name| icon_name_to_glyph(&name));
        let disabled = get_bool_or(props, "disabled", false);
        let selected = get_bool_or(props, "selected", false);
        let class = if selected {
            format!("{class} almadar-choice-button--selected")
        } else {
            class
        };
        let action = get_string(props, "action");
        let on_click = get_string(props, "onClick");
        let payload = props
            .as_object()
            .and_then(|value| value.get("payload"))
            .cloned()
            .unwrap_or_else(|| Value::object(orbital_core::ValueMap::new()));
        let sink = view.sink;
        let root_attrs = shared_root_attrs_with_defaults(
            props,
            &[("aria-pressed", Value::String(selected.to_string().into()))],
        );
        rsx! {
            button {
                class: "{class}",
                disabled: disabled,
                onclick: move |_| {
                    if disabled {
                        return;
                    }
                    if let Some(sink) = sink {
                        if let Some(event) = &action {
                            sink.call((event.clone(), payload.clone()));
                        }
                        if let Some(event) = &on_click {
                            sink.call((event.clone(), Value::object(orbital_core::ValueMap::new())));
                        }
                    }
                },
                ..root_attrs,
                if let Some(index) = index {
                    span { class: "almadar-choice-button__index", "{index}." }
                }
                if let Some(content) = asset_content {
                    span { class: "almadar-choice-button__icon", {content} }
                } else if let Some(glyph) = icon {
                    span { class: "almadar-choice-button__icon", "{glyph}" }
                }
                span { "{text}" }
            }
        }
    }
}
