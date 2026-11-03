use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::avl_flow::{trait_flow_graph, AvlTraitFlow};
use orbital_core::client::helpers::{
    from_json, get_array, get_string, get_string_or, merge_class_name, to_json,
};
use orbital_core::schema::OrbitalSchema;
use orbital_core::Value;
use std::collections::HashMap;

pub struct FlowCanvasRenderer;

#[derive(Props, Clone, PartialEq)]
struct FlowLensProps {
    flow: AvlTraitFlow,
    selected: Option<String>,
    select_event: Option<String>,
    sink: Option<EventSink>,
    class_name: String,
}

#[allow(non_snake_case)]
fn FlowLens(props: FlowLensProps) -> Element {
    let mut local_selection = use_signal(|| None::<String>);
    let selected = props
        .selected
        .as_ref()
        .or(local_selection().as_ref())
        .cloned();
    let flow = props.flow;
    rsx! {
        div { class: "{props.class_name}",
            if flow.units.is_empty() { div { class: "almadar-muted", "No traits" } }
            for unit in flow.units.iter() {
                {
                    let id = unit.id.clone();
                    let payload = serde_json::to_value(unit).ok().map(from_json);
                    let select_event = props.select_event.clone();
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
                                if let (Some(sink), Some(event), Some(payload)) = (sink, select_event.clone(), payload.clone()) {
                                    sink.call((event, payload));
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
        let schema_value = props.as_object().and_then(|object| object.get("schema"));
        let schema = schema_value.and_then(|value| match value {
            Value::String(text) => serde_json::from_str::<OrbitalSchema>(text.as_str()).ok(),
            other => serde_json::from_value::<OrbitalSchema>(to_json(other)).ok(),
        });
        let Some(schema) = schema else {
            return rsx! { div { class: "{class_name}", role: "alert", "Invalid program" } };
        };
        let lens = props
            .as_object()
            .and_then(|object| object.get("traitView"))
            .and_then(|value| get_string(value, "lens"));
        let orbital_name = get_string(props, "initialOrbital");
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
                    sink: view.sink,
                    class_name,
                } };
            }
        }
        let orbitals = from_json(serde_json::to_value(&schema).unwrap_or_default());
        rsx! {
            div { class: "{class_name}",
                if get_array(&orbitals, "orbitals").is_empty() { span { class: "almadar-muted", "No program" } }
                for orbital in get_array(&orbitals, "orbitals").iter() {
                    {
                        let name = get_string_or(orbital, "name", "orbital");
                        let entity_name = orbital.as_object().and_then(|object| object.get("entity"))
                            .map(|entity| get_string_or(entity, "name", ""))
                            .unwrap_or_default();
                        let counts = format!("{} traits  {} pages", get_array(orbital, "traits").len(), get_array(orbital, "pages").len());
                        rsx! { div { class: "almadar-flow-canvas__orbital",
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
