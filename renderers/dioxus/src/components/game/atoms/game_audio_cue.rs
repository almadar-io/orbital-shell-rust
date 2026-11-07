use crate::audio::{AudioPlay, AudioProps, AudioSessionHandle};
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
        let field = |name: &str| value.as_object().and_then(|object| object.get(name));
        let plans = (
            AudioPlay::from_value(field("cuePlay")),
            AudioPlay::from_value(field("musicPlay")),
            AudioPlay::list_from_value(field("autostart")),
        );
        let (cue_play, music_play, autostart) = match plans {
            (Ok(cue), Ok(music), Ok(autostart)) => (cue, music, autostart),
            (cue, music, autostart) => {
                for error in [cue.err(), music.err(), autostart.err()].into_iter().flatten() {
                    tracing::error!(%error, "invalid game-audio-cue plan");
                }
                return;
            }
        };
        let audio = AudioProps {
            source_trait: props.source_trait.clone(),
            cue_seq: get_number(&value, "cueSeq"),
            cue_play,
            music_play,
            autostart,
            muted: get_bool_or(&value, "muted", false),
            volume: get_number_or(&value, "volume", 1.0).clamp(0.0, 1.0) as f32,
            manifest: field("manifest").cloned().unwrap_or(Value::Null),
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
