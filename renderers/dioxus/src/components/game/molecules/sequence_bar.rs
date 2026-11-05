use crate::renderer::merge_class_name;
use crate::components::game::molecules::trait_slot::render_slot;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_array, get_bool_or, get_number_or, get_string, };
use orbital_core::{Value, ValueMap};

pub struct SequenceBarRenderer;

impl DioxusPatternRenderer for SequenceBarRenderer {
    fn pattern_name(&self) -> &str {
        "sequence-bar"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let root_attrs = shared_root_attrs(props);
        let slots = get_array(props, "slots");
        let feedback = get_array(props, "slotFeedback");
        let max_slots = get_number_or(props, "maxSlots", 0.0).max(0.0) as usize;
        let playing = get_bool_or(props, "playing", false);
        let current_step = get_number_or(props, "currentStep", -1.0) as isize;
        let size = get_string(props, "size");
        let empty_label = get_string(props, "emptyLabel");
        let remove_label = get_string(props, "removeLabel");
        let colors = props
            .as_object()
            .and_then(|props| props.get("categoryColors"))
            .cloned();
        let drop_event = get_string(props, "slotDropEvent").filter(|event| !event.is_empty());
        let remove_event = get_string(props, "slotRemoveEvent").filter(|event| !event.is_empty());
        let drop_callback = get_string(props, "onSlotDrop").filter(|event| !event.is_empty());
        let remove_callback = get_string(props, "onSlotRemove").filter(|event| !event.is_empty());
        let class = merge_class_name("almadar-sequence-bar", props);
        let children: Vec<(usize, Element)> = (0..max_slots)
            .map(|index| {
                let item = slots
                    .get(index)
                    .filter(|item| !matches!(item, Value::Null))
                    .cloned();
                let mut slot = ValueMap::new();
                slot.insert("slotNumber".into(), Value::Number((index + 1) as f64));
                slot.insert("locked".into(), Value::Bool(playing));
                slot.insert(
                    "selected".into(),
                    Value::Bool(current_step == index as isize),
                );
                slot.insert("draggable".into(), Value::Bool(!playing && item.is_some()));
                if let Some(item) = item {
                    slot.insert("equippedItem".into(), item);
                    if let Some(event) = &remove_event {
                        slot.insert("removeEvent".into(), Value::String(event.clone().into()));
                    }
                    if let Some(event) = &remove_callback {
                        slot.insert("onSlotRemove".into(), Value::String(event.clone().into()));
                    }
                }
                if let Some(event) = &drop_event {
                    slot.insert("dropEvent".into(), Value::String(event.clone().into()));
                }
                if let Some(event) = &drop_callback {
                    slot.insert("onSlotDrop".into(), Value::String(event.clone().into()));
                }
                if let Some(size) = &size {
                    slot.insert("size".into(), Value::String(size.clone().into()));
                }
                if let Some(label) = &empty_label {
                    slot.insert("emptyLabel".into(), Value::String(label.clone().into()));
                }
                if let Some(label) = &remove_label {
                    slot.insert("removeLabel".into(), Value::String(label.clone().into()));
                }
                if let Some(colors) = &colors {
                    slot.insert("categoryColors".into(), colors.clone());
                }
                if let Some(value) = feedback.get(index) {
                    slot.insert("feedback".into(), value.clone());
                }
                let child = render_slot(&Value::Object(slot.into()), index + 1, view, registry);
                (index, child)
            })
            .collect();
        rsx! {
            div { class: "{class}", style: "display:flex;align-items:center;gap:8px", ..root_attrs,
                for (index, child) in children.into_iter() {
                    if index > 0 {
                        span {
                            class: if current_step >= 0 && index as isize <= current_step { "text-primary" } else { "text-muted-foreground" },
                            "→"
                        }
                    }
                    {child}
                }
            }
        }
    }
}
