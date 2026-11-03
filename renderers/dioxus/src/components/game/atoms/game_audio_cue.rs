use crate::audio::{AudioProps, AudioSessionHandle};
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_bool_or, get_number, get_number_or, get_string_or};
use orbital_core::Value;
use std::rc::Rc;

pub struct GameAudioCueRenderer;

#[derive(Props, Clone, PartialEq)]
struct CueProps {
    value: Value,
    source_trait: String,
}

#[allow(non_snake_case)]
fn Cue(props: CueProps) -> Element {
    let handle = use_hook(|| Rc::new(AudioSessionHandle::new()));
    let value = props.value;
    let mut effect = use_effect(move || {
        let audio = AudioProps {
            source_trait: props.source_trait.clone(),
            cue: get_string_or(&value, "cue", ""),
            cue_seq: get_number(&value, "cueSeq"),
            music: get_string_or(&value, "music", ""),
            muted: get_bool_or(&value, "muted", false),
            volume: get_number_or(&value, "volume", 1.0).clamp(0.0, 1.0) as f32,
            manifest: value
                .as_object()
                .and_then(|object| object.get("manifest"))
                .cloned()
                .unwrap_or(Value::Null),
            base_url: get_string_or(&value, "baseUrl", ""),
        };
        handle.sync(audio);
    });
    // Props are plain values, so Dioxus does not track them as effect dependencies.
    effect.mark_dirty();
    rsx! {}
}

impl DioxusPatternRenderer for GameAudioCueRenderer {
    fn pattern_name(&self) -> &str {
        "game-audio-cue"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { Cue { value: props.clone(), source_trait: view.source_trait.as_deref().unwrap_or(view.trait_name).to_string() } }
    }
}
