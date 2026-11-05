use crate::registry::{shared_root_attrs_with_defaults, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_bool_or, get_string, get_string_or, merge_class_name};
use orbital_core::{Value, ValueMap};

pub struct GameAudioToggleRenderer;

impl DioxusPatternRenderer for GameAudioToggleRenderer {
    fn pattern_name(&self) -> &str {
        "game-audio-toggle"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let muted = get_bool_or(props, "muted", false);
        let label = if muted {
            get_string_or(props, "mutedLabel", "Muted")
        } else {
            get_string_or(props, "soundOnLabel", "Sound on")
        };
        let asset = props
            .as_object()
            .and_then(|value| value.get(if muted { "offAsset" } else { "onAsset" }))
            .cloned();
        let content = if let Some(asset) = asset {
            let mut spec = ValueMap::new();
            spec.insert("type".into(), Value::String("atlas-image".into()));
            spec.insert("asset".into(), asset);
            spec.insert("size".into(), Value::Number(20.0));
            registry.render_spec(&Value::Object(spec.into()), entity, view)
        } else if muted {
            rsx! { span { "🔇" } }
        } else {
            rsx! { span { "🔊" } }
        };
        let disabled = get_bool_or(props, "isLoading", false);
        let toggle_event = get_string(props, "toggleEvent");
        let sink = view.sink;
        let size = get_string_or(props, "size", "sm");
        let class = merge_class_name("almadar-button almadar-game-audio-toggle", props);
        let root_attrs = shared_root_attrs_with_defaults(
            props,
            &[
                ("aria-label", Value::String(label.into())),
                ("aria-pressed", Value::String(muted.to_string().into())),
                ("aria-busy", Value::Bool(disabled)),
            ],
        );
        rsx! {
            button {
                class: "{class}",
                "data-size": "{size}",
                disabled: disabled,
                onclick: move |_| {
                    if !disabled {
                        if let (Some(sink), Some(event)) = (sink, &toggle_event) {
                            let mut payload = ValueMap::new();
                            payload.insert("muted".into(), Value::Bool(!muted));
                            sink.call((event.clone(), Value::Object(payload.into())));
                        }
                    }
                },
                ..root_attrs,
                span { aria_hidden: "true", {content} }
            }
        }
    }
}
