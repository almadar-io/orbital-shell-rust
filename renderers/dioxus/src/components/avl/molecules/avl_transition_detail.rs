use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::avl_playback::{
    transition_playback, AvlSceneStep, AvlTransitionPlayback,
};
use orbital_core::client::helpers::{
    from_json, get_array, get_string, get_string_or, merge_class_name, to_json,
};
use orbital_core::schema::{format_sexpr, trait_machine, transition_at, OrbitalSchema};
use orbital_core::{Value, ValueMap};

pub struct AvlTransitionDetailRenderer;

#[derive(Clone, PartialEq)]
struct Detail {
    orbital: String,
    trait_name: String,
    from: Vec<String>,
    to: String,
    event: String,
    guard: Option<String>,
    effects: Vec<(String, String)>,
}

fn schema_from_props(props: &Value) -> Option<OrbitalSchema> {
    let schema = props.as_object()?.get("schema")?;
    match schema {
        Value::String(text) => serde_json::from_str(text.as_str()).ok(),
        other => serde_json::from_value(to_json(other)).ok(),
    }
}

fn scene_playback(props: &Value, detail: &Detail) -> Option<AvlTransitionPlayback> {
    let schema = schema_from_props(props)?;
    let machine = trait_machine(&schema, &detail.orbital, &detail.trait_name)?;
    let index = props.as_object()?.get("transitionIndex")?.as_number()?;
    if !index.is_finite() || index < 0.0 || index.fract() != 0.0 {
        return None;
    }
    let scene_value = props.as_object()?.get("scene")?;
    let scene: Vec<AvlSceneStep> = serde_json::from_value(to_json(scene_value)).ok()?;
    if scene.is_empty() {
        return None;
    }
    let cursor = props
        .as_object()
        .and_then(|obj| obj.get("cursor"))
        .and_then(Value::as_number)
        .filter(|value| value.is_finite() && *value >= 0.0 && value.fract() == 0.0)
        .map(|value| value as usize)
        .unwrap_or(scene.len() - 1);
    transition_playback(
        machine,
        index as usize,
        &detail.orbital,
        &detail.trait_name,
        &scene,
        cursor,
    )
}

fn sources(value: &Value) -> Vec<String> {
    match value {
        Value::String(source) => vec![source.to_string()],
        Value::Array(sources) => sources
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn note_text(note: &Value) -> Option<String> {
    let title = get_string(note, "title");
    let body = get_string(note, "body");
    match (title, body) {
        (Some(title), Some(body)) => Some(format!("{title}: {body}")),
        (Some(title), None) => Some(title),
        (None, Some(body)) => Some(body),
        (None, None) => None,
    }
}

fn detail_from_props(props: &Value) -> Option<Detail> {
    let orbital = get_string(props, "orbital")?;
    let trait_name = get_string(props, "trait")?;
    if let Some(transition) = props.as_object().and_then(|obj| obj.get("transition")) {
        let effects = get_array(transition, "effects")
            .iter()
            .map(|effect| {
                let kind = get_string_or(effect, "type", "");
                let args = get_array(effect, "args")
                    .iter()
                    .map(|value| to_json(value).to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                let line = if args.is_empty() {
                    format!("({kind})")
                } else {
                    format!("({kind} {args})")
                };
                (kind, line)
            })
            .collect();
        return Some(Detail {
            orbital,
            trait_name,
            from: transition
                .as_object()
                .and_then(|object| object.get("from"))
                .map(sources)
                .unwrap_or_default(),
            to: get_string_or(transition, "to", ""),
            event: get_string_or(transition, "event", ""),
            guard: transition
                .as_object()
                .and_then(|obj| obj.get("guard"))
                .filter(|value| !matches!(value, Value::Null))
                .map(|value| to_json(value).to_string()),
            effects,
        });
    }
    let schema = schema_from_props(props)?;
    let index = props.as_object()?.get("transitionIndex")?.as_number()?;
    if !index.is_finite() || index < 0.0 || index.fract() != 0.0 {
        return None;
    }
    let transition = transition_at(&schema, &orbital, &trait_name, index as usize)?;
    let effects = transition
        .effects
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|effect| {
            let line = format_sexpr(effect);
            let kind = line
                .trim_start_matches('(')
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            (kind, line)
        })
        .collect();
    Some(Detail {
        orbital,
        trait_name,
        from: transition
            .from
            .states()
            .into_iter()
            .map(str::to_string)
            .collect(),
        to: transition.to.clone(),
        event: transition.event.clone(),
        guard: transition.guard.as_ref().map(format_sexpr),
        effects,
    })
}

#[derive(Props, Clone, PartialEq)]
struct StepProps {
    detail: Detail,
    from: String,
    event: String,
    sink: Option<EventSink>,
}

#[allow(non_snake_case)]
fn StepControl(props: StepProps) -> Element {
    let mut payload_text = use_signal(|| "{}".to_string());
    let mut error = use_signal(String::new);
    let detail = props.detail.clone();
    rsx! {
        div { class: "almadar-avl-step-control",
            label { "Payload"
                textarea {
                    value: "{payload_text}",
                    oninput: move |e| payload_text.set(e.value()),
                }
            }
            button {
                onclick: move |_| {
                    let Ok(serde_json::Value::Object(payload)) = serde_json::from_str::<serde_json::Value>(&payload_text()) else {
                        error.set("Payload must be a JSON object".to_string());
                        return;
                    };
                    error.set(String::new());
                    let mut request = ValueMap::new();
                    request.insert("orbital".into(), Value::string(detail.orbital.clone()));
                    request.insert("trait".into(), Value::string(detail.trait_name.clone()));
                    request.insert("from".into(), Value::string(props.from.clone()));
                    request.insert("event".into(), Value::string(detail.event.clone()));
                    request.insert("payload".into(), from_json(serde_json::Value::Object(payload)));
                    if let Some(sink) = props.sink {
                        sink.call((props.event.clone(), Value::Object(request.into())));
                    }
                },
                "Step"
            }
            if !error().is_empty() { span { role: "alert", "{error}" } }
        }
    }
}

impl DioxusPatternRenderer for AvlTransitionDetailRenderer {
    fn pattern_name(&self) -> &str {
        "avl-transition-detail"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let Some(detail) = detail_from_props(props) else {
            return rsx! { div { class: "almadar-avl-transition-missing", "Transition missing" } };
        };
        let class = merge_class_name("almadar-avl-transition-detail", props);
        let step_event = get_string(props, "stepEvent");
        let playback = props
            .as_object()
            .and_then(|obj| obj.get("playback"))
            .cloned()
            .or_else(|| {
                scene_playback(props, &detail)
                    .and_then(|played| serde_json::to_value(played).ok())
                    .map(from_json)
            });
        let guard_run = playback
            .as_ref()
            .and_then(|played| played.as_object())
            .and_then(|played| played.get("guard"))
            .filter(|guard| !matches!(guard, Value::Null));
        let fired = playback
            .as_ref()
            .and_then(|played| played.as_object())
            .and_then(|played| played.get("fired"))
            == Some(&Value::Bool(true));
        let verdict = playback.as_ref().map(|_| {
            if fired {
                "Fired"
            } else if guard_run
                .and_then(Value::as_object)
                .and_then(|guard| guard.get("passed"))
                == Some(&Value::Bool(false))
            {
                "Blocked"
            } else {
                "Not reached"
            }
        });
        let guard_verdict = guard_run
            .and_then(Value::as_object)
            .and_then(|guard| guard.get("passed"))
            .map(|passed| {
                if passed == &Value::Bool(true) {
                    "Passed"
                } else {
                    "Failed"
                }
            });
        let effect_runs = playback
            .as_ref()
            .map(|played| get_array(played, "effects"))
            .unwrap_or_default();
        let effects = detail.effects.clone();
        let guard = detail.guard.clone();
        let annotations = props.as_object().and_then(|obj| obj.get("annotations"));
        let event_note = annotations
            .and_then(|value| value.as_object())
            .and_then(|obj| obj.get("transitions"))
            .and_then(|value| value.as_object())
            .and_then(|obj| obj.get(detail.event.as_str()))
            .and_then(note_text);
        let effect_notes: Vec<_> = effects
            .iter()
            .map(|(kind, _)| {
                annotations
                    .and_then(|value| value.as_object())
                    .and_then(|obj| obj.get("effects"))
                    .and_then(|value| value.as_object())
                    .and_then(|obj| obj.get(kind.as_str()))
                    .and_then(note_text)
            })
            .collect();
        rsx! {
            section { class: "{class}",
                header { class: "almadar-avl-transition-detail__header",
                    for (index, from) in detail.from.iter().enumerate() {
                        span { key: "{from}-{index}", "{from}" }
                    }
                    span { "{detail.event}" }
                    span { "→" }
                    span { "{detail.to}" }
                    if let Some(verdict) = verdict { span { "{verdict}" } }
                }
                if let Some(note) = event_note { p { class: "almadar-avl-transition-detail__note", "{note}" } }
                if let Some(guard) = guard {
                    section { class: "almadar-avl-transition-detail__guard",
                        h3 { "Guard" }
                        code { "{guard}" }
                        if let Some(guard_verdict) = guard_verdict { span { "{guard_verdict}" } }
                    }
                }
                if !effects.is_empty() {
                    section { class: "almadar-avl-transition-detail__effects",
                        h3 { "Effects ({effects.len()})" }
                        for (index, (kind, line)) in effects.iter().enumerate() {
                            div { key: "{index}",
                                strong { "{kind}" }
                                code { "{line}" }
                                if let Some(status) = effect_runs.get(index).and_then(|run| get_string(run, "status")) {
                                    span { "{status}" }
                                }
                                if let Some(note) = effect_notes.get(index).and_then(|note| note.as_ref()) {
                                    p { class: "almadar-avl-transition-detail__note", "{note}" }
                                }
                            }
                        }
                    }
                }
                if let Some(event) = step_event {
                    for (index, from) in detail.from.iter().enumerate() {
                        StepControl { key: "{from}-{index}", detail: detail.clone(), from: from.clone(), event: event.clone(), sink: view.sink }
                    }
                }
            }
        }
    }
}
