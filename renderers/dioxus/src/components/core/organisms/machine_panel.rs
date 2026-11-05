use crate::components::avl::schema::schema_from_props;
use crate::components::core::organisms::flow_canvas::FlowLens;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView, OwnedDioxusView};
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use crate::timer::Delay;
use dioxus::prelude::*;
use orbital_core::client::avl_flow::{trait_flow_graph, AvlFlowUnit};
use orbital_core::client::avl_playback::{trace_playback, AvlTransitionTrace};
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use orbital_core::client::helpers::get_number;
use orbital_core::client::helpers::{from_json, get_string, to_json};
use orbital_core::schema::avl_projection::{
    described_transition_lines, program_trait_levels, AvlTraitDescription,
};
use orbital_core::schema::trait_machine;
use orbital_core::{Value, ValueMap};
use std::collections::{HashMap, HashSet};
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use std::time::Duration;

pub struct MachinePanelRenderer;

#[derive(Clone, PartialEq)]
struct Replay {
    step: usize,
    playing: bool,
}

#[derive(Props, Clone, PartialEq)]
struct MachinePanelProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn dependency_layers(props: &Value) -> HashMap<String, String> {
    props
        .as_object()
        .and_then(|object| object.get("dependencies"))
        .and_then(|value| value.as_object())
        .and_then(|object| object.get("behaviors"))
        .and_then(|value| value.as_object())
        .map(|behaviors| {
            behaviors
                .iter()
                .filter_map(|(name, behavior)| {
                    get_string(behavior, "layer").map(|layer| (name.to_string(), layer))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn traces_from_props(props: &Value) -> Vec<AvlTransitionTrace> {
    props
        .as_object()
        .and_then(|object| object.get("transitions"))
        .and_then(|value| serde_json::from_value(to_json(value)).ok())
        .unwrap_or_default()
}

fn description_from_props(
    props: &Value,
    behavior: Option<&str>,
    trait_name: &str,
) -> Option<AvlTraitDescription> {
    let behavior = behavior?;
    let key = format!("{behavior}.traits.{trait_name}");
    let description = props
        .as_object()?
        .get("descriptions")?
        .as_object()?
        .get(key.as_str())?;
    serde_json::from_value(to_json(description)).ok()
}

#[allow(non_snake_case)]
fn MachinePanel(props: MachinePanelProps) -> Element {
    let mut pinned = use_signal(|| None::<String>);
    let mut replay = use_signal(|| None::<Replay>);
    let mut timer_generation = use_signal(|| 0u64);
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    let interval_ms = get_number(&props.props, "replayStepMs")
        .filter(|ms| ms.is_finite() && *ms >= 0.0)
        .unwrap_or(800.0) as u64;
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    let timer_trait = pinned();
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    let timer_hop_count = traces_from_props(&props.props)
        .iter()
        .filter(|hop| timer_trait.as_deref() == Some(hop.trait_name.as_str()))
        .count();
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    use_effect(move || {
        let generation = timer_generation();
        if replay().as_ref().is_some_and(|state| state.playing) {
            spawn(async move {
                Delay::new(Duration::from_millis(interval_ms)).await;
                if timer_generation() != generation {
                    return;
                }
                if let Some(state) = replay() {
                    if state.playing {
                        let next = state.step.saturating_add(1);
                        replay.set(Some(Replay {
                            step: next.min(timer_hop_count.saturating_sub(1)),
                            playing: next.saturating_add(1) < timer_hop_count,
                        }));
                    }
                }
            });
        }
    });
    let Some(schema) = schema_from_props(&props.props) else {
        return rsx! { div { role: "alert", ..shared_root_attrs(&props.props), "Invalid program" } };
    };
    let Ok(entries) = program_trait_levels(&schema) else {
        return rsx! { div { role: "alert", ..shared_root_attrs(&props.props), "Invalid machine" } };
    };
    if entries.is_empty() {
        return rsx! { div { class: "almadar-machine-panel-empty", ..shared_root_attrs(&props.props), "No machines" } };
    }
    let names: HashSet<_> = entries
        .iter()
        .map(|entry| entry.trait_ref.name.as_str())
        .collect();
    let ran: Vec<_> = traces_from_props(&props.props)
        .into_iter()
        .filter(|hop| names.contains(hop.trait_name.as_str()))
        .collect();
    let latest = ran.last().map(|hop| hop.trait_name.as_str());
    let chosen = pinned()
        .or_else(|| latest.map(str::to_string))
        .unwrap_or_else(|| entries[0].trait_ref.name.clone());
    let Some(entry) = entries.iter().find(|entry| entry.trait_ref.name == chosen) else {
        return rsx! { div { role: "alert", ..shared_root_attrs(&props.props), "Machine missing" } };
    };
    let Some(machine) = trait_machine(&schema, &entry.orbital, &entry.trait_ref.name) else {
        return rsx! { div { role: "alert", ..shared_root_attrs(&props.props), "Machine missing" } };
    };
    let hops: Vec<_> = ran
        .iter()
        .filter(|hop| hop.trait_name == chosen)
        .cloned()
        .collect();
    let replay_state = replay();
    let shown: Vec<_> = if let Some(state) = &replay_state {
        hops.iter()
            .take(state.step.saturating_add(1))
            .cloned()
            .collect()
    } else {
        hops.clone()
    };
    let playback = trace_playback(machine, &chosen, &shown);
    let flow = trait_flow_graph(&schema, &entry.orbital, &dependency_layers(&props.props));
    let selected_unit = flow
        .units
        .iter()
        .find(|unit| {
            unit.traits
                .iter()
                .chain(unit.render_pieces.iter())
                .any(|name| name == &chosen)
        })
        .map(|unit| unit.id.clone());
    let ran_for_select = ran.clone();
    let on_select = Callback::new(move |unit: AvlFlowUnit| {
        let members: Vec<_> = unit
            .traits
            .iter()
            .chain(unit.render_pieces.iter())
            .collect();
        if pinned()
            .as_ref()
            .is_some_and(|name| members.contains(&name))
        {
            pinned.set(None);
            replay.set(None);
        } else {
            let selected = ran_for_select
                .iter()
                .rev()
                .find(|hop| members.contains(&&hop.trait_name))
                .map(|hop| hop.trait_name.clone())
                .or_else(|| members.first().map(|name| (*name).clone()));
            if let Some(selected) = selected {
                let count = ran_for_select
                    .iter()
                    .filter(|hop| hop.trait_name == selected)
                    .count();
                pinned.set(Some(selected));
                replay.set((count > 0).then_some(Replay {
                    step: 0,
                    playing: count > 1,
                }));
            }
        }
        timer_generation.set(timer_generation().wrapping_add(1));
    });
    let hop_count = hops.len();
    let mut machine_spec = ValueMap::new();
    machine_spec.insert("type".into(), Value::string("state-machine-view"));
    machine_spec.insert(
        "trait".into(),
        from_json(serde_json::to_value(&entry.level).unwrap_or_default()),
    );
    machine_spec.insert("showHeader".into(), Value::Bool(false));
    if let Some(state) = &playback.active_state {
        machine_spec.insert("activeState".into(), Value::string(state.clone()));
    }
    if let Some(index) = playback.active_transition {
        machine_spec.insert("activeTransition".into(), Value::Number(index as f64));
    }
    machine_spec.insert(
        "visitedStates".into(),
        Value::Array(
            playback
                .visited_states
                .iter()
                .cloned()
                .map(Value::string)
                .collect::<Vec<_>>()
                .into(),
        ),
    );
    let machine_element = props.registry.render_spec(
        &Value::Object(machine_spec.into()),
        None,
        &props.view.as_view(),
    );
    let source_behavior = entry
        .trait_ref
        .source_behavior
        .as_ref()
        .map(|source| (source.behavior.as_str(), source.original_name.as_str()));
    let description = if let Some((behavior, original)) = source_behavior {
        description_from_props(&props.props, Some(behavior), original)
    } else {
        description_from_props(
            &props.props,
            get_string(&props.props, "behavior").as_deref(),
            &chosen,
        )
    };
    let title = source_behavior
        .map(|(_, original)| original)
        .unwrap_or(&chosen);
    let source = description.as_ref().and_then(|description| {
        description.source.as_ref().map(|source| {
            let highlight = playback
                .active_transition
                .and_then(|index| described_transition_lines(description, machine, index));
            let lines: Vec<_> = source
                .text
                .split_inclusive('\n')
                .enumerate()
                .map(|(index, text)| {
                    let line = index + 1;
                    (
                        text.to_string(),
                        highlight
                            .as_ref()
                            .is_some_and(|span| line >= span.start && line <= span.end),
                    )
                })
                .collect();
            lines
        })
    });
    let following = pinned().is_none();
    let replay_step = replay_state.as_ref().map(|state| state.step);
    let replaying = replay_state.as_ref().is_some_and(|state| state.playing);
    let replay_controls = replay_state
        .as_ref()
        .map(|state| (state.step, state.playing));
    rsx! {
        section {
            class: "almadar-machine-panel",
            "data-trait": "{chosen}",
            "data-following": if following { "true" } else { "false" },
            "data-replay-step": replay_step.map(|step| step.to_string()),
            "data-replaying": replay_state.as_ref().map(|_| if replaying { "true" } else { "false" }),
            ..shared_root_attrs(&props.props),
            header {
                strong { "{chosen}" }
                if let Some((step, playing)) = replay_controls {
                    div { role: "toolbar", aria_label: "Replay",
                        button { r#type: "button", disabled: step == 0, onclick: move |_| {
                            replay.set(Some(Replay { step: step.saturating_sub(1), playing: false }));
                            timer_generation.set(timer_generation().wrapping_add(1));
                        }, "Back" }
                        button { r#type: "button", onclick: move |_| {
                            replay.set(Some(Replay {
                                step: if !playing && step.saturating_add(1) >= hop_count { 0 } else { step },
                                playing: !playing && hop_count > 1,
                            }));
                            timer_generation.set(timer_generation().wrapping_add(1));
                        }, if playing { "Pause" } else { "Play" } }
                        button { r#type: "button", disabled: step.saturating_add(1) >= hop_count, onclick: move |_| {
                            replay.set(Some(Replay { step: (step + 1).min(hop_count.saturating_sub(1)), playing: false }));
                            timer_generation.set(timer_generation().wrapping_add(1));
                        }, "Forward" }
                        input { r#type: "range", min: "0", max: "{hop_count.saturating_sub(1)}", value: "{step}", oninput: move |event| {
                            if let Ok(step) = event.value().parse::<usize>() {
                                replay.set(Some(Replay { step: step.min(hop_count.saturating_sub(1)), playing: false }));
                                timer_generation.set(timer_generation().wrapping_add(1));
                            }
                        } }
                        span { "{step + 1}/{hop_count}" }
                    }
                }
            }
            div { class: "almadar-machine-panel__columns",
                FlowLens {
                    flow,
                    selected: if pinned().is_some() || latest.is_some() { selected_unit } else { None },
                    select_event: None,
                    select_callback: None,
                    view_change_callback: None,
                    on_select: Some(on_select),
                    sink: None,
                    class_name: "almadar-machine-panel__flow".to_string(),
                    root_props: Value::Null,
                }
                if let Some(source) = source {
                    pre { class: "almadar-machine-panel__code", aria_label: "Source for {title}", code {
                        for (index, (line, highlighted)) in source.iter().enumerate() {
                            span {
                                key: "{index}",
                                class: if *highlighted { "almadar-machine-panel__code-line almadar-machine-panel__code-line--highlight" } else { "almadar-machine-panel__code-line" },
                                "{line}"
                            }
                        }
                    } }
                }
                div { class: "almadar-machine-panel__machine", {machine_element} }
            }
        }
    }
}

impl DioxusPatternRenderer for MachinePanelRenderer {
    fn pattern_name(&self) -> &str {
        "machine-panel"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { MachinePanel { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
