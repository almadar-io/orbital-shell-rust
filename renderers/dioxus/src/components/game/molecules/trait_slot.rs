use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_bool_or, get_number_or, get_string, get_string_or, merge_class_name,
};
use orbital_core::{Value, ValueMap};

const DRAG_MIME: &str = "application/x-almadar-slot-item";

pub struct TraitSlotRenderer;

#[derive(Props, Clone, PartialEq)]
struct SlotProps {
    class: String,
    style: String,
    slot_number: usize,
    event_slot_number: usize,
    item_id: Option<String>,
    item_name: Option<String>,
    item_description: Option<String>,
    lock_label: Option<String>,
    empty_label: Option<String>,
    remove_label: Option<String>,
    root_attrs: Vec<Attribute>,
    icon_content: Element,
    tooltip_frame: Option<Element>,
    drag_json: Option<String>,
    drag_item: Option<Value>,
    machine_states: Vec<String>,
    machine_transitions: Vec<String>,
    locked: bool,
    selected: bool,
    draggable: bool,
    show_tooltip: bool,
    click_event: Option<String>,
    click_callback: Option<String>,
    remove_event: Option<String>,
    remove_callback: Option<String>,
    sequence_remove_callback: Option<String>,
    drop_event: Option<String>,
    drop_callback: Option<String>,
    sequence_drop_callback: Option<String>,
    drag_start_callback: Option<String>,
    sink: Option<EventSink>,
}

fn slot_payload(number: usize, item_id: Option<&str>) -> Value {
    let mut payload = ValueMap::new();
    payload.insert("slotNumber".into(), Value::Number(number as f64));
    if let Some(id) = item_id {
        payload.insert("itemId".into(), Value::String(id.to_string().into()));
    }
    Value::Object(payload.into())
}

#[allow(non_snake_case)]
fn Slot(props: SlotProps) -> Element {
    let mut over = use_signal(|| false);
    let mut hovered = use_signal(|| false);
    let class = if over() {
        format!("{} almadar-trait-slot--drag-over", props.class)
    } else {
        props.class.clone()
    };
    let item_name = props.item_name.clone();
    let item_description = props.item_description.clone();
    let root_attrs = props.root_attrs.clone();
    let lock_label = props.lock_label.clone();
    let remove_label = props.remove_label.clone();
    let icon_content = props.icon_content.clone();
    let tooltip_frame = props.tooltip_frame.clone();
    let slot_number = props.slot_number;
    let event_slot_number = props.event_slot_number;
    let locked = props.locked;
    let draggable = props.draggable && props.item_id.is_some() && !locked;
    let click_event = props.click_event.clone();
    let click_callback = props.click_callback.clone();
    let remove_event = props.remove_event.clone();
    let remove_callback = props.remove_callback.clone();
    let sequence_remove_callback = props.sequence_remove_callback.clone();
    let drop_event = props.drop_event.clone();
    let drop_callback = props.drop_callback.clone();
    let sequence_drop_callback = props.sequence_drop_callback.clone();
    let drag_start_callback = props.drag_start_callback.clone();
    let accepts_drop =
        drop_event.is_some() || drop_callback.is_some() || sequence_drop_callback.is_some();
    let drag_json = props.drag_json.clone();
    let drag_item = props.drag_item.clone();
    let sink = props.sink;
    let sink_remove = sink;
    let sink_drop = sink;
    let machine_states = props.machine_states.clone();
    let machine_transitions = props.machine_transitions.clone();
    let item_present = props.item_id.is_some();
    let tooltip_name = item_name.clone().unwrap_or_default();
    let states_line = machine_states.join(" → ");
    rsx! {
        div { class: "almadar-trait-slot-wrap", style: "position:relative;display:inline-flex;align-items:center;justify-content:center",
            button {
                class: "{class}",
                style: "{props.style}",
                disabled: locked,
                draggable: draggable,
                onclick: move |_| {
                    if !locked {
                        if let Some(sink) = sink {
                            if let Some(event) = &click_event {
                                sink.call((event.clone(), slot_payload(event_slot_number, None)));
                            }
                            if let Some(event) = &click_callback {
                                sink.call((event.clone(), Value::Null));
                            }
                        }
                    }
                },
                onmouseenter: move |_| hovered.set(true),
                onmouseleave: move |_| hovered.set(false),
                ondragstart: move |event| {
                    if !draggable {
                        event.prevent_default();
                        return;
                    }
                    if let Some(data) = &drag_json {
                        let transfer = event.data().data_transfer();
                        if let Err(error) = transfer.set_data(DRAG_MIME, data) {
                            tracing::error!(%error, "trait-slot drag payload could not be stored");
                            event.prevent_default();
                            return;
                        }
                        transfer.set_effect_allowed("move");
                        if let (Some(sink), Some(name), Some(item)) = (sink, &drag_start_callback, &drag_item) {
                            let mut payload = ValueMap::new();
                            payload.insert("item".into(), item.clone());
                            sink.call((name.clone(), Value::Object(payload.into())));
                        }
                    }
                },
                ondragover: move |event| {
                    if locked || !accepts_drop {
                        return;
                    }
                    let transfer = event.data().data_transfer();
                    if transfer.get_data(DRAG_MIME).is_some() {
                        event.prevent_default();
                        transfer.set_drop_effect(if transfer.effect_allowed() == "copy" { "copy" } else { "move" });
                        over.set(true);
                    }
                },
                ondragleave: move |_| over.set(false),
                ondrop: move |event| {
                    over.set(false);
                    if locked {
                        return;
                    }
                    if !accepts_drop { return; }
                    let Some(raw) = event.data().data_transfer().get_data(DRAG_MIME) else { return };
                    let Ok(item) = serde_json::from_str::<Value>(&raw) else { return };
                    let Some(id) = get_string(&item, "id") else { return };
                    event.prevent_default();
                    if let Some(sink) = sink_drop {
                        if let Some(name) = &drop_event {
                            sink.call((name.clone(), slot_payload(event_slot_number, Some(&id))));
                        }
                        if let Some(name) = &drop_callback {
                            let mut payload = ValueMap::new();
                            payload.insert("item".into(), item.clone());
                            sink.call((name.clone(), Value::Object(payload.into())));
                        }
                        if let Some(name) = &sequence_drop_callback {
                            let mut payload = ValueMap::new();
                            payload.insert("index".into(), Value::Number(event_slot_number as f64));
                            payload.insert("item".into(), item);
                            sink.call((name.clone(), Value::Object(payload.into())));
                        }
                    }
                },
                ..root_attrs,
                if locked {
                    span { "🔒" }
                    if let Some(label) = &lock_label { span { "{label}" } }
                } else if item_present {
                    {icon_content}
                } else {
                    span { "+" }
                }
            }
            if item_present && (remove_event.is_some() || remove_callback.is_some() || sequence_remove_callback.is_some()) && !locked {
                button {
                    class: "almadar-trait-slot__remove",
                    style: "position:absolute;top:-6px;right:-6px;border-radius:50%;width:18px;height:18px",
                    "aria-label": remove_label,
                    onclick: move |_| {
                        if let Some(sink) = sink_remove {
                            if let Some(event) = &remove_event {
                                sink.call((event.clone(), slot_payload(event_slot_number, None)));
                            }
                            if let Some(event) = &remove_callback {
                                sink.call((event.clone(), Value::Null));
                            }
                            if let Some(event) = &sequence_remove_callback {
                                let mut payload = ValueMap::new();
                                payload.insert("index".into(), Value::Number(event_slot_number as f64));
                                sink.call((event.clone(), Value::Object(payload.into())));
                            }
                        }
                    },
                    "×"
                }
            }
            span { class: "almadar-trait-slot__number", style: "position:absolute;bottom:-7px;left:-7px", "{slot_number}" }
            if props.show_tooltip && hovered() && item_present {
                div { class: "almadar-trait-slot__tooltip", style: "position:absolute;bottom:100%;left:50%;transform:translateX(-50%);z-index:10;padding:12px;min-width:200px;background:var(--color-background);border:1px solid var(--color-border)",
                    if let Some(frame) = tooltip_frame {
                        div { style: "position:absolute;inset:0;pointer-events:none", {frame} }
                    }
                    div { style: "position:relative",
                        strong { "{tooltip_name}" }
                        if let Some(description) = &item_description { p { "{description}" } }
                        if !machine_states.is_empty() { p { "{states_line}" } }
                        for transition in machine_transitions.iter() { p { "{transition}" } }
                    }
                }
            }
        }
    }
}

pub(crate) fn render_slot(
    props: &Value,
    event_slot_number: usize,
    view: &DioxusView<'_>,
    registry: &DioxusPatternRegistry,
) -> Element {
    let slot_number = get_number_or(props, "slotNumber", 1.0).max(0.0) as usize;
    let item = props.as_object().and_then(|p| p.get("equippedItem"));
    let item_id = item.and_then(|item| get_string(item, "id"));
    let item_name = item.and_then(|item| get_string(item, "name"));
    let item_description = item.and_then(|item| get_string(item, "description"));
    let lock_label = get_string(props, "lockLabel");
    let empty_label = get_string(props, "emptyLabel");
    let remove_label = get_string(props, "removeLabel");
    let mut root_attrs = shared_root_attrs(props);
    if get_string(props, "aria-label").is_none() {
        if let Some(name) = item_name.clone().or_else(|| empty_label.clone()) {
            root_attrs.push(Attribute::new("aria-label", name, None, false));
        }
    }
    let category = item
        .map(|item| get_string_or(item, "category", ""))
        .unwrap_or_default();
    let colors = props
        .as_object()
        .and_then(|p| p.get("categoryColors"))
        .and_then(|value| value.as_object())
        .and_then(|colors| colors.get(category.as_str()));
    let background = colors.and_then(|value| get_string(value, "bg"));
    let border = colors.and_then(|value| get_string(value, "border"));
    let size = match get_string_or(props, "size", "md").as_str() {
        "sm" => 40,
        "lg" => 72,
        _ => 56,
    };
    let locked = get_bool_or(props, "locked", false);
    let selected = get_bool_or(props, "selected", false);
    let feedback = get_string_or(props, "feedback", "");
    let edge = match feedback.as_str() {
        "correct" => "var(--color-success)",
        "wrong" => "var(--color-error)",
        _ => border.as_deref().unwrap_or("var(--color-border)"),
    };
    let ring = if selected {
        "box-shadow:0 0 0 2px var(--color-primary);"
    } else {
        ""
    };
    let opacity = if locked { "0.5" } else { "1" };
    let style = format!(
        "width:{size}px;height:{size}px;border:2px {} {edge};border-radius:var(--radius-container);background:{};opacity:{opacity};{ring}touch-action:none",
        if item_id.is_none() { "dashed" } else { "solid" },
        background.as_deref().unwrap_or("var(--color-card)")
    );
    let icon_asset = item
        .and_then(|item| item.as_object())
        .and_then(|item| item.get("iconUrl"))
        .cloned();
    let icon = item
        .map(|item| get_string_or(item, "iconEmoji", "✦"))
        .unwrap_or_default();
    let icon_content = if let Some(asset) = icon_asset {
        let mut spec = ValueMap::new();
        spec.insert("type".into(), Value::String("atlas-image".into()));
        spec.insert("asset".into(), asset);
        spec.insert("size".into(), Value::Number((size as f64) * 0.6));
        registry.render_spec(&Value::Object(spec.into()), None, view)
    } else {
        rsx! { span { "{icon}" } }
    };
    let tooltip_frame = props
        .as_object()
        .and_then(|props| props.get("tooltipFrameUrl"))
        .map(|asset| {
            crate::components::game::asset_panel::fill_parent(asset, "slice", view, registry)
        });
    let drag_json = item.and_then(|item| serde_json::to_string(item).ok());
    let machine = item
        .and_then(|item| item.as_object())
        .and_then(|item| item.get("stateMachine"));
    let machine_states = machine
        .map(|machine| orbital_core::client::helpers::get_string_array(machine, "states"))
        .unwrap_or_default();
    let machine_transitions = machine
        .map(|machine| orbital_core::client::helpers::get_array(machine, "transitions"))
        .unwrap_or_default()
        .iter()
        .map(|tr| {
            format!(
                "{} —{}→ {}",
                get_string_or(tr, "from", ""),
                get_string_or(tr, "event", ""),
                get_string_or(tr, "to", "")
            )
        })
        .collect();
    let class = merge_class_name("almadar-trait-slot", props);
    let click_event = get_string(props, "clickEvent").filter(|event| !event.is_empty());
    let click_callback = get_string(props, "onClick").filter(|event| !event.is_empty());
    let remove_event = get_string(props, "removeEvent").filter(|event| !event.is_empty());
    let remove_callback = get_string(props, "onRemove").filter(|event| !event.is_empty());
    let sequence_remove_callback =
        get_string(props, "onSlotRemove").filter(|event| !event.is_empty());
    let drop_event = get_string(props, "dropEvent").filter(|event| !event.is_empty());
    let drop_callback = get_string(props, "onItemDrop").filter(|event| !event.is_empty());
    let sequence_drop_callback = get_string(props, "onSlotDrop").filter(|event| !event.is_empty());
    let drag_start_callback = get_string(props, "onDragStart").filter(|event| !event.is_empty());
    rsx! {
        Slot {
            class, style, slot_number, event_slot_number,
            item_id, item_name, item_description, lock_label, empty_label, remove_label, icon_content, tooltip_frame, drag_json,
            drag_item: item.cloned(),
            root_attrs,
            machine_states, machine_transitions, locked, selected,
            draggable: get_bool_or(props, "draggable", false),
            show_tooltip: get_bool_or(props, "showTooltip", true),
            click_event, click_callback, remove_event, remove_callback, sequence_remove_callback,
            drop_event, drop_callback, sequence_drop_callback, drag_start_callback, sink: view.sink
        }
    }
}

impl DioxusPatternRenderer for TraitSlotRenderer {
    fn pattern_name(&self) -> &str {
        "trait-slot"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let slot_number = get_number_or(props, "slotNumber", 1.0).max(0.0) as usize;
        render_slot(props, slot_number, view, registry)
    }
}
