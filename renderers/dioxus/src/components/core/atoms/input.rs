use crate::renderer::merge_class_name;
use super::field_draft::use_field_draft;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView, OwnedDioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::cell::RefCell;
use std::rc::Rc;

fn icon_spec(name: &str) -> Value {
    let mut fields = orbital_core::ValueMap::new();
    fields.insert("type".into(), Value::String("icon".into()));
    fields.insert("name".into(), Value::String(name.to_owned().into()));
    Value::Object(fields.into())
}

/// Dioxus renderer for the `input` pattern.
pub struct InputRenderer;

impl InputRenderer {
    const NAME: &str = "input";
}

impl DioxusPatternRenderer for InputRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { InputField { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}

#[derive(Props, Clone, PartialEq)]
struct InputFieldProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn InputField(input: InputFieldProps) -> Element {
    let props = &input.props;
    let view = input.view.as_view();
    let registry = &input.registry;
    let class = merge_class_name("block w-full rounded-interactive transition-all duration-fast border-[length:var(--border-width-thin)] border-border interactive-border h-input-md px-3 text-sm bg-card text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring focus:border-ring disabled:opacity-50 disabled:cursor-not-allowed disabled:bg-muted", props);
    let placeholder = get_string_or(props, "placeholder", "");
    let label = get_string_or(props, "label", "");
    let field_id = get_string(props, "id");
    let helper = get_string_or(props, "helperText", "");
    let error = get_string_or(props, "error", "");
    let value = match props.as_object().and_then(|object| object.get("value")) {
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Int(number)) => number.to_string(),
        _ => get_string_or(props, "value", ""),
    };
    let draft = use_field_draft(value);
    let value = draft.value();
    let input_type = get_string_or(props, "inputType", "text");
    let disabled = get_bool_or(props, "disabled", false);
    let clearable = get_bool_or(props, "clearable", false);
    let left_icon = get_string(props, "leftIcon").or_else(|| get_string(props, "icon"));
    let right_icon = get_string(props, "rightIcon");
    let left_padding = if left_icon.is_some() { "pl-10" } else { "" };
    let right_padding = if right_icon.is_some() || clearable {
        "pr-10"
    } else {
        ""
    };
    let on_clear = get_string(props, "onClear");
    let action = get_string(props, "action");
    let on_change = get_string(props, "onChange");
    let sink = view.sink;
    if input_type == "checkbox" {
        let checked = get_bool_or(props, "checked", false);
        return rsx! { div { class: "w-fit",
            label { class: "inline-flex items-center gap-2 text-sm font-medium text-foreground", for: field_id.clone(),
                input { r#type: "checkbox", checked: checked, disabled: disabled,
                    onchange: move |event| {
                        if disabled { return; }
                        if let (Some(sink), Some(name)) = (sink, &on_change) {
                            let mut payload = orbital_core::ValueMap::new();
                            payload.insert("checked".into(), Value::Bool(event.checked()));
                            sink.call((name.clone(), Value::Object(payload.into())));
                        }
                    },
                    ..shared_root_attrs(props),
                }
                "{label}"
            }
            if !error.is_empty() { span { class: "mt-1 text-xs text-error", role: "alert", "{error}" } }
        } };
    }
    if input_type == "select" {
        let options = props
            .as_object()
            .and_then(|fields| fields.get("options"))
            .and_then(Value::as_array);
        return rsx! { div { class: "w-full",
            label { class: "block w-full", for: field_id.clone(),
            if !label.is_empty() { span { class: "block text-sm font-medium text-foreground mb-1", "{label}" } }
            select { class: "{class} appearance-none pr-10", value: "{value}", disabled: disabled,
                onchange: move |event| {
                    if disabled { return; }
                    let mut draft = draft.clone();
                    draft.record(event.value());
                    if let (Some(sink), Some(name)) = (sink, &on_change) {
                        let mut payload = orbital_core::ValueMap::new();
                        payload.insert("value".into(), Value::String(event.value().into()));
                        sink.call((name.clone(), Value::Object(payload.into())));
                    }
                },
                ..shared_root_attrs(props),
                option { value: "", "Select..." }
                if let Some(options) = options {
                    for option in options {
                        if let Some(fields) = option.as_object() {
                            {
                                let option_value = fields.get("value").and_then(Value::as_str).unwrap_or("");
                                let option_label = fields.get("label").and_then(Value::as_str).unwrap_or("");
                                rsx! { option { value: "{option_value}", "{option_label}" } }
                            }
                        }
                    }
                }
            }
            }
            if !error.is_empty() { span { class: "mt-1 text-xs text-error", role: "alert", "{error}" } }
            if error.is_empty() && !helper.is_empty() { span { class: "mt-1 text-xs text-muted-foreground", "{helper}" } }
        } };
    }
    if input_type == "textarea" {
        let rows = get_number_or(props, "rows", 3.0).max(1.0) as i64;
        return rsx! { div { class: "w-full",
            label { class: "block w-full", for: field_id.clone(),
            if !label.is_empty() { span { class: "block text-sm font-medium text-foreground mb-1", "{label}" } }
            textarea { class: "{class} h-auto py-2", rows: "{rows}", value: "{value}", disabled: disabled, placeholder: "{placeholder}",
                oninput: move |event| {
                    if disabled { return; }
                    let mut draft = draft.clone();
                    draft.record(event.value());
                    if let (Some(sink), Some(name)) = (sink, &on_change) {
                        let mut payload = orbital_core::ValueMap::new();
                        payload.insert("value".into(), Value::String(event.value().into()));
                        sink.call((name.clone(), Value::Object(payload.into())));
                    }
                },
                ..shared_root_attrs(props),
            }
            }
            if !error.is_empty() { span { class: "mt-1 text-xs text-error", role: "alert", "{error}" } }
            if error.is_empty() && !helper.is_empty() { span { class: "mt-1 text-xs text-muted-foreground", "{helper}" } }
        } };
    }
    let number_mode = input_type == "number";
    let current = Rc::new(RefCell::new(value.clone()));
    let on_input_value = current.clone();
    let on_key_value = current.clone();
    let mut input_draft = draft.clone();
    rsx! {
        div { class: "w-full",
            label { class: "block w-full", for: field_id.clone(),
            if !label.is_empty() { span { class: "block text-sm font-medium text-foreground mb-1", "{label}" } }
            div { class: "relative w-full",
            if let Some(name) = left_icon.as_ref() { div { class: "absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none text-muted-foreground", {registry.render_spec(&icon_spec(name), None, &view)} } }
            input {
                class: "{class} {left_padding} {right_padding}",
                aria_label: "{label}",
                aria_invalid: !error.is_empty(),
                r#type: "{input_type}",
                placeholder: "{placeholder}",
                value: "{value}",
                disabled: disabled,
                oninput: move |event| {
                    if disabled { return; }
                    let raw = event.value();
                    input_draft.record(raw.clone());
                    *on_input_value.borrow_mut() = raw.clone();
                    if let (Some(sink), Some(name)) = (sink, &on_change) {
                        let mut payload = orbital_core::ValueMap::new();
                        payload.insert("value".into(), Value::String(raw.into()));
                        sink.call((name.clone(), Value::Object(payload.into())));
                    }
                },
                onkeydown: move |event| {
                    if disabled || event.key() != Key::Enter { return; }
                    if let (Some(sink), Some(name)) = (sink, &action) {
                        let raw = on_key_value.borrow().clone();
                        let value = if number_mode {
                            if raw.trim().is_empty() { return; }
                            match raw.parse::<f64>() { Ok(number) if number.is_finite() => Value::Number(number), _ => return }
                        } else { Value::String(raw.into()) };
                        let mut payload = orbital_core::ValueMap::new();
                        payload.insert("value".into(), value);
                        sink.call((name.clone(), Value::Object(payload.into())));
                    }
                },
                ..shared_root_attrs(props),
            }
            if clearable && !value.is_empty() {
                button { r#type: "button", class: "absolute inset-y-0 right-0 px-3 text-muted-foreground hover:text-foreground", aria_label: "Clear input", disabled: disabled,
                    onclick: move |_| {
                        if let (Some(sink), Some(name)) = (sink, &on_clear) {
                            sink.call((name.clone(), Value::Object(orbital_core::ValueMap::new().into())));
                        }
                    },
                    {registry.render_spec(&icon_spec("x"), None, &view)}
                }
            } else if let Some(name) = right_icon.as_ref() {
                div { class: "absolute inset-y-0 right-0 pr-3 flex items-center pointer-events-none text-muted-foreground", {registry.render_spec(&icon_spec(name), None, &view)} }
            }
            }
            }
            if !error.is_empty() { span { class: "mt-1 text-xs text-error", role: "alert", "{error}" } }
            if error.is_empty() && !helper.is_empty() { span { class: "mt-1 text-xs text-muted-foreground", "{helper}" } }
        }
    }
}
