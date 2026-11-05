use crate::components::avl::schema::schema_from_props;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::avl_flow::{trait_flow_graph, AvlFlowUnit, AvlTraitFlow};
use orbital_core::client::helpers::{
    from_json, get_array, get_string, get_string_or, merge_class_name, to_json,
};
use orbital_core::schema::avl_projection::program_trait_levels;
use orbital_core::Value;
use std::collections::HashMap;

pub struct FlowCanvasRenderer;

#[derive(Props, Clone, PartialEq)]
pub(crate) struct FlowLensProps {
    pub(crate) flow: AvlTraitFlow,
    pub(crate) selected: Option<String>,
    pub(crate) select_event: Option<String>,
    pub(crate) select_callback: Option<String>,
    pub(crate) view_change_callback: Option<String>,
    pub(crate) on_select: Option<Callback<AvlFlowUnit>>,
    pub(crate) sink: Option<EventSink>,
    pub(crate) class_name: String,
    pub(crate) root_props: Value,
}

#[allow(non_snake_case)]
pub(crate) fn FlowLens(props: FlowLensProps) -> Element {
    let mut local_selection = use_signal(|| None::<String>);
    let selected = props
        .selected
        .as_ref()
        .or(local_selection().as_ref())
        .cloned();
    let flow = props.flow;
    let selected_unit = flow
        .units
        .iter()
        .find(|unit| selected.as_deref() == Some(unit.id.as_str()))
        .cloned();
    rsx! {
        div { class: "{props.class_name}", ..shared_root_attrs(&props.root_props),
            if flow.units.is_empty() { div { class: "almadar-muted", "No traits" } }
            if let (Some(unit), Some(callback)) = (selected_unit, props.view_change_callback.clone()) {
                {
                    let sink = props.sink;
                    let name = unit.name.clone();
                    let traits: Vec<_> = unit.traits.iter().chain(unit.render_pieces.iter()).cloned().collect();
                    rsx! { button {
                        r#type: "button",
                        class: "almadar-flow-canvas__show-traits",
                        onclick: move |_| {
                            if let Some(sink) = sink {
                                sink.call((callback.clone(), from_json(serde_json::json!({
                                    "view": {"lens":"traits", "filter":{"unit":name.clone(),"traits":traits.clone()}}
                                }))));
                            }
                        },
                        "Show traits"
                    } }
                }
            }
            for unit in flow.units.iter() {
                {
                    let id = unit.id.clone();
                    let payload = serde_json::to_value(unit).ok().map(from_json);
                    let select_event = props.select_event.clone();
                    let select_callback = props.select_callback.clone();
                    let on_select = props.on_select;
                    let selected_unit = unit.clone();
                    let sink = props.sink;
                    let active = selected.as_deref() == Some(id.as_str());
                    let events: Vec<_> = flow.edges.iter().filter(|edge| edge.source == id).collect();
                    rsx! {
                        button {
                            key: "{id}",
                            r#type: "button",
                            class: if active { "almadar-flow-canvas__unit almadar-flow-canvas__unit--selected" } else { "almadar-flow-canvas__unit" },
                            aria_pressed: if active { "true" } else { "false" },
                            onclick: move |_| {
                                local_selection.set(Some(id.clone()));
                                if let Some(on_select) = on_select {
                                    on_select.call(selected_unit.clone());
                                }
                                if let (Some(sink), Some(payload)) = (sink, payload.clone()) {
                                    if let Some(event) = select_callback.clone() {
                                        let mut callback_payload = orbital_core::ValueMap::new();
                                        callback_payload.insert("unit".into(), payload.clone());
                                        sink.call((event, Value::Object(callback_payload.into())));
                                    }
                                    if let Some(event) = select_event.clone() {
                                        sink.call((event, payload));
                                    }
                                }
                            },
                            span { class: "almadar-flow-canvas__column", "{unit.column}" }
                            strong { "{unit.name}" }
                            for trait_name in unit.traits.iter() { span { class: "almadar-flow-canvas__trait", "{trait_name}" } }
                            for piece in unit.render_pieces.iter() { span { class: "almadar-flow-canvas__piece", "{piece}" } }
                            for edge in events.iter() {
                                {
                                    let event_names = edge.events.join(", ");
                                    rsx! { span { class: "almadar-flow-canvas__wire", "{event_names} → {edge.target}" } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for FlowCanvasRenderer {
    fn pattern_name(&self) -> &str {
        "flow-canvas"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let class_name = merge_class_name("almadar-flow-canvas", props);
        let Some(schema) = schema_from_props(props) else {
            return rsx! { div { class: "{class_name}", role: "alert", ..shared_root_attrs(props), "Invalid program" } };
        };
        let lens = props
            .as_object()
            .and_then(|object| object.get("traitView"))
            .and_then(|value| get_string(value, "lens"));
        let orbital_name = get_string(props, "initialOrbital");
        let initial_level = get_string(props, "initialLevel");
        if lens.as_deref() == Some("flow") {
            if let Some(orbital_name) = orbital_name {
                let layers: HashMap<String, String> = props
                    .as_object()
                    .and_then(|object| object.get("systemDependencies"))
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
                    .unwrap_or_default();
                let flow = trait_flow_graph(&schema, &orbital_name, &layers);
                return rsx! { FlowLens {
                    flow,
                    selected: get_string(props, "flowSelection"),
                    select_event: get_string(props, "flowSelectEvent"),
                    select_callback: get_string(props, "onFlowSelect"),
                    view_change_callback: get_string(props, "onTraitViewChange"),
                    on_select: None,
                    sink: view.sink,
                    class_name,
                    root_props: props.clone(),
                } };
            }
        }
        if initial_level.as_deref() == Some("trait-expanded") {
            let Some(orbital_name) = orbital_name else {
                return rsx! { div { class: "{class_name}", role: "alert", ..shared_root_attrs(props), "No orbital selected" } };
            };
            let Ok(entries) = program_trait_levels(&schema) else {
                return rsx! { div { class: "{class_name}", role: "alert", ..shared_root_attrs(props), "Invalid machines" } };
            };
            let entries: Vec<_> = entries
                .into_iter()
                .filter(|entry| entry.orbital == orbital_name)
                .collect();
            let on_node_click = get_string(props, "onNodeClick");
            let sink = view.sink;
            return rsx! {
                div { class: "{class_name}", ..shared_root_attrs(props),
                    if entries.is_empty() { span { class: "almadar-muted", "No traits" } }
                    for entry in entries.iter() {
                        section { key: "{entry.trait_ref.name}", class: "almadar-flow-canvas__trait-card",
                            h3 { "{entry.trait_ref.name}" }
                            div { class: "almadar-flow-canvas__states",
                                for state in entry.level.states.iter() {
                                    span { key: "{state.name}", class: "almadar-flow-canvas__state", "{state.name}" }
                                }
                            }
                            for transition in entry.level.transitions.iter() {
                                {
                                    let orbital = entry.orbital.clone();
                                    let trait_name = entry.trait_ref.name.clone();
                                    let from = transition.from.to_string();
                                    let to = transition.to.clone();
                                    let event = transition.event.clone();
                                    let index = transition.index;
                                    let callback = on_node_click.clone();
                                    rsx! {
                                        button {
                                            key: "{index}",
                                            r#type: "button",
                                            class: "almadar-flow-canvas__transition",
                                            onclick: move |_| {
                                                if let (Some(sink), Some(callback)) = (sink, callback.clone()) {
                                                    let payload = from_json(serde_json::json!({
                                                        "context": {
                                                            "level": "transition",
                                                            "orbital": orbital.clone(),
                                                            "trait": trait_name.clone(),
                                                            "transition": event.clone(),
                                                            "transitionIndex": index,
                                                        }
                                                    }));
                                                    sink.call((callback, payload));
                                                }
                                            },
                                            "{from} — {event} → {to}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            };
        }
        let orbitals = from_json(serde_json::to_value(&schema).unwrap_or_default());
        let on_node_click = get_string(props, "onNodeClick");
        let on_node_select = get_string(props, "onNodeSelect");
        let open_traits_event = get_string(props, "openTraitsEvent");
        let system_level = initial_level.as_deref() == Some("system");
        let sink = view.sink;
        rsx! {
            div { class: "{class_name}", ..shared_root_attrs(props),
                if get_array(&orbitals, "orbitals").is_empty() { span { class: "almadar-muted", "No program" } }
                for orbital in get_array(&orbitals, "orbitals").iter() {
                    {
                        let name = get_string_or(orbital, "name", "orbital");
                        let entity_name = orbital.as_object().and_then(|object| object.get("entity"))
                            .map(|entity| get_string_or(entity, "name", ""))
                            .unwrap_or_default();
                        let counts = format!("{} traits  {} pages", get_array(orbital, "traits").len(), get_array(orbital, "pages").len());
                        let click = on_node_click.clone();
                        let select = on_node_select.clone();
                        let open = open_traits_event.clone();
                        let orbital_name = name.clone();
                        rsx! { button { r#type: "button", class: "almadar-flow-canvas__orbital",
                            onclick: move |_| {
                                if let Some(sink) = sink {
                                    if system_level {
                                        if let Some(open) = open.clone() {
                                            sink.call((open, from_json(serde_json::json!({"orbital": orbital_name.clone()}))));
                                        }
                                    } else {
                                        if let Some(click) = click.clone() {
                                            sink.call((click, from_json(serde_json::json!({"context": {"level": "overview", "orbital": orbital_name.clone()}}))));
                                        }
                                        if let Some(select) = select.clone() {
                                            sink.call((select, from_json(serde_json::json!({"orbitalName": orbital_name.clone()}))));
                                        }
                                    }
                                }
                            },
                            div { class: "almadar-flow-canvas__name", "{name}" }
                            if !entity_name.is_empty() { div { class: "almadar-muted", "{entity_name}" } }
                            div { class: "almadar-muted", "{counts}" }
                        } }
                    }
                }
            }
        }
    }
}
