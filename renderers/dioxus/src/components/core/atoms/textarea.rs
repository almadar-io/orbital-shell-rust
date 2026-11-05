use super::field_draft::use_field_draft;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView, OwnedDioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::cell::RefCell;
use std::rc::Rc;

/// Dioxus renderer for the `textarea` pattern.
pub struct TextareaRenderer;

impl TextareaRenderer {
    const NAME: &str = "textarea";
}

impl DioxusPatternRenderer for TextareaRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { TextareaField { props: props.clone(), view: view.to_owned() } }
    }
}

#[derive(Props, Clone, PartialEq)]
struct TextareaFieldProps {
    props: Value,
    view: OwnedDioxusView,
}

#[allow(non_snake_case)]
fn TextareaField(field: TextareaFieldProps) -> Element {
    let props = &field.props;
    let view = field.view.as_view();
    let class = merge_class_name("block w-full rounded-interactive border-[length:var(--border-width)] shadow-elevation-interactive interactive-border px-3 py-2 text-sm text-foreground bg-card placeholder:text-[var(--color-placeholder)] focus:outline-none focus:ring-2 focus:ring-offset-0 focus:ring-ring disabled:bg-muted disabled:text-muted-foreground disabled:cursor-not-allowed resize-y min-h-20 border-border focus:border-primary", props);
    let placeholder = get_string_or(props, "placeholder", "");
    let value = get_string_or(props, "value", "");
    let draft = use_field_draft(value);
    let value = draft.value();
    let label = get_string_or(props, "label", "");
    let field_id = get_string(props, "id");
    let helper = get_string_or(props, "helperText", "");
    let error = get_string_or(props, "error", "");
    let rows = get_number_or(props, "rows", 3.0).max(1.0) as u32;
    let on_change = get_string(props, "onChange");
    let action = get_string(props, "action");
    let sink = view.sink;
    let current = Rc::new(RefCell::new(value.clone()));
    let input_value = current.clone();
    let key_value = current.clone();
    let mut input_draft = draft.clone();
    rsx! { div { class: "w-full",
        label { class: "block w-full", for: field_id,
        if !label.is_empty() { span { class: "block text-sm font-bold text-foreground mb-1", "{label}" } }
        textarea {
            class: "{class}",
            placeholder: "{placeholder}", value: "{value}", rows: "{rows}", aria_label: "{label}", aria_invalid: !error.is_empty(),
            oninput: move |event| {
                let raw = event.value();
                input_draft.record(raw.clone());
                *input_value.borrow_mut() = raw.clone();
                if let (Some(sink), Some(name)) = (sink, &on_change) {
                    let mut payload = orbital_core::ValueMap::new();
                    payload.insert("value".into(), Value::String(raw.into()));
                    sink.call((name.clone(), Value::Object(payload.into())));
                }
            },
            onkeydown: move |event| {
                if event.key() != Key::Enter || !(event.modifiers().meta() || event.modifiers().ctrl()) { return; }
                if let (Some(sink), Some(name)) = (sink, &action) {
                    event.prevent_default();
                    let mut payload = orbital_core::ValueMap::new();
                    payload.insert("value".into(), Value::String(key_value.borrow().clone().into()));
                    sink.call((name.clone(), Value::Object(payload.into())));
                }
            },
            ..shared_root_attrs(props),
        }
        }
        if !error.is_empty() { span { class: "mt-1 text-xs text-error", role: "alert", "{error}" } }
        if error.is_empty() && !helper.is_empty() { span { class: "mt-1 text-xs text-muted-foreground", "{helper}" } }
    } }
}
