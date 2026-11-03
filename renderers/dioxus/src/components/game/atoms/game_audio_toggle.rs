use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_bool_or, get_number, get_string, get_string_or, merge_class_name,
};
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
        let accessible_name = get_string(props, "aria-label").unwrap_or(label);
        let labelled_by = get_string(props, "aria-labelledby");
        let described_by = get_string(props, "aria-describedby");
        let live = get_string(props, "aria-live");
        let current = get_string(props, "aria-current");
        let expanded = props
            .as_object()
            .and_then(|value| value.get("aria-expanded"))
            .and_then(Value::as_bool)
            .map(|value| value.to_string());
        let selected = props
            .as_object()
            .and_then(|value| value.get("aria-selected"))
            .and_then(Value::as_bool)
            .map(|value| value.to_string());
        let hidden = props
            .as_object()
            .and_then(|value| value.get("aria-hidden"))
            .and_then(Value::as_bool)
            .map(|value| value.to_string());
        let busy = get_bool_or(props, "aria-busy", disabled).to_string();
        let tab_index = get_number(props, "tabIndex").map(|value| (value as i64).to_string());
        let role = get_string(props, "role");
        let lang = get_string(props, "lang");
        let dir = get_string(props, "dir");
        rsx! {
            button {
                class: "{class}",
                "data-size": "{size}",
                aria_label: accessible_name,
                aria_labelledby: labelled_by,
                aria_describedby: described_by,
                aria_live: live,
                aria_current: current,
                aria_pressed: muted.to_string(),
                aria_expanded: expanded,
                aria_selected: selected,
                aria_hidden: hidden,
                aria_busy: busy,
                tabindex: tab_index,
                role: role,
                lang: lang,
                dir: dir,
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
                span { aria_hidden: "true", {content} }
            }
        }
    }
}
