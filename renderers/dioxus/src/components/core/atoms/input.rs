use super::field_draft::use_field_draft;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    cn, merge_class_name, passthrough_attrs, pattern_spec, DioxusPatternRenderer, DioxusView,
    OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_INPUT: AtomicU64 = AtomicU64::new(0);

fn icon_spec(name: &str) -> Value {
    pattern_spec(
        "icon",
        [
            ("name", Value::string(name)),
            ("className", Value::string("h-icon-default w-icon-default")),
        ],
    )
}

/// The HTML props the web `Input` spreads onto its element, plus the id and description links it computes.
fn field_attrs(
    props: &Value,
    id: &str,
    description_id: Option<&str>,
    invalid: bool,
) -> Vec<Attribute> {
    let mut attrs = passthrough_attrs(props, &[]);
    attrs.retain(|attr| {
        !matches!(
            attr.name,
            "id" | "aria-describedby" | "aria-invalid" | "aria-required"
        )
    });
    attrs.push(Attribute::new("id", id.to_string(), None, false));
    let described = [
        get_string(props, "aria-describedby"),
        description_id.map(str::to_string),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    if !described.is_empty() {
        attrs.push(Attribute::new("aria-describedby", described, None, false));
    }
    if invalid || get_bool_or(props, "aria-invalid", false) {
        attrs.push(Attribute::new("aria-invalid", "true", None, false));
    }
    if get_bool_or(props, "required", false) {
        attrs.push(Attribute::new("aria-required", "true", None, false));
        attrs.push(Attribute::new("required", true, None, false));
    }
    if get_bool_or(props, "readOnly", false) {
        attrs.push(Attribute::new("readonly", true, None, false));
    }
    for (key, name) in [
        ("name", "name"),
        ("autoComplete", "autocomplete"),
        ("pattern", "pattern"),
        ("min", "min"),
        ("max", "max"),
        ("step", "step"),
        ("maxLength", "maxlength"),
        ("minLength", "minlength"),
        ("inputMode", "inputmode"),
        ("enterKeyHint", "enterkeyhint"),
    ] {
        let text = match props.get(key) {
            Some(Value::String(text)) => text.to_string(),
            Some(Value::Int(number)) => number.to_string(),
            Some(Value::Number(number)) => number.to_string(),
            _ => continue,
        };
        attrs.push(Attribute::new(name, text, None, false));
    }
    attrs
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
    let generated = use_hook(|| format!("input-{}", NEXT_INPUT.fetch_add(1, Ordering::Relaxed)));
    let placeholder = get_string_or(props, "placeholder", "");
    let label = get_string_or(props, "label", "");
    let field_id = get_string(props, "id").unwrap_or(generated);
    let helper = get_string(props, "helperText").filter(|text| !text.is_empty());
    let error = get_string(props, "error").filter(|text| !text.is_empty());
    let description = error.clone().or(helper);
    let description_id = description
        .as_ref()
        .map(|_| format!("{field_id}-description"));
    let value = match props.as_object().and_then(|object| object.get("value")) {
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Int(number)) => number.to_string(),
        _ => get_string_or(props, "value", ""),
    };
    let draft = use_field_draft(value);
    let value = draft.value();
    let input_type = get_string(props, "inputType")
        .filter(|kind| !kind.is_empty())
        .unwrap_or_else(|| "text".to_string());
    let disabled = get_bool_or(props, "disabled", false);
    let auto_focus = get_bool_or(props, "autoFocus", false);
    let clearable = get_bool_or(props, "clearable", false);
    let left_icon = get_string(props, "leftIcon")
        .filter(|name| !name.is_empty())
        .or_else(|| get_string(props, "icon").filter(|name| !name.is_empty()));
    let right_icon = get_string(props, "rightIcon").filter(|name| !name.is_empty());
    let show_clear = clearable && !value.is_empty();
    let multiline = input_type == "textarea";
    let class = cn(&[
        "block w-full rounded-interactive transition-all duration-fast border-[length:var(--border-width-thin)] border-border interactive-border",
        if multiline { "px-3 py-2 text-sm" } else { "h-input-md px-3 text-sm" },
        "bg-card hover:bg-muted focus:bg-card text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring focus:border-ring disabled:opacity-50 disabled:cursor-not-allowed disabled:bg-muted",
        if error.is_some() { "border-error focus:border-error focus:ring-error" } else { "" },
        if left_icon.is_some() { "pl-10" } else { "" },
        if right_icon.is_some() || show_clear { "pr-10" } else { "" },
        &get_string_or(props, "className", ""),
    ]);
    let checkbox_class = merge_class_name(
        "h-icon-default w-icon-default rounded-interactive border-border text-primary focus:ring-ring disabled:opacity-50 disabled:cursor-not-allowed",
        props,
    );
    let attrs = field_attrs(props, &field_id, description_id.as_deref(), error.is_some());
    let on_clear = get_string(props, "onClear");
    let action = get_string(props, "action");
    let on_change = get_string(props, "onChange");
    let sink = view.sink;
    let description_class = cn(&[
        "mt-1 text-xs",
        if error.is_some() {
            "text-error"
        } else {
            "text-muted-foreground"
        },
    ]);
    let label_view = rsx! {
        if !label.is_empty() { label { for: "{field_id}", class: "block text-sm font-medium text-foreground mb-1", "{label}" } }
    };
    let description_view = rsx! {
        if let (Some(text), Some(id)) = (description.as_ref(), description_id.as_ref()) {
            p { id: "{id}", class: "{description_class}", "{text}" }
        }
    };
    let left_view = rsx! {
        if let Some(name) = left_icon.as_ref() { div { class: "absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none text-muted-foreground", {registry.render_spec(&icon_spec(name), None, &view)} } }
    };
    if input_type == "checkbox" {
        let checked = get_bool_or(props, "checked", false);
        return rsx! { div { class: "w-fit",
            {label_view}
            input { r#type: "checkbox", class: "{checkbox_class}", checked: checked, disabled: disabled, autofocus: auto_focus,
                onchange: move |event| {
                    if disabled { return; }
                    if let (Some(sink), Some(name)) = (sink, &on_change) {
                        let mut payload = orbital_core::ValueMap::new();
                        payload.insert("checked".into(), Value::Bool(event.checked()));
                        sink.call((name.clone(), Value::Object(payload.into())));
                    }
                },
                ..attrs,
            }
            {description_view}
        } };
    }
    if input_type == "select" {
        let options = props
            .as_object()
            .and_then(|fields| fields.get("options"))
            .and_then(Value::as_array);
        let select_class = cn(&[&class, "appearance-none pr-10"]);
        let chevron = icon_spec("chevron-down");
        return rsx! { div { class: "w-full",
            {label_view}
            div { class: "relative w-full",
                {left_view}
                select { class: "{select_class}", value: "{value}", disabled: disabled, autofocus: auto_focus,
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
                    ..attrs,
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
                div { class: "absolute inset-y-0 right-0 pr-3 flex items-center pointer-events-none text-muted-foreground", {registry.render_spec(&chevron, None, &view)} }
            }
            {description_view}
        } };
    }
    if multiline {
        let rows = get_number_or(props, "rows", 3.0).max(1.0) as i64;
        return rsx! { div { class: "w-full",
            {label_view}
            div { class: "relative w-full",
                textarea { class: "{class}", rows: "{rows}", value: "{value}", disabled: disabled, placeholder: "{placeholder}", autofocus: auto_focus,
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
                    ..attrs,
                }
            }
            {description_view}
        } };
    }
    let number_mode = input_type == "number";
    let current = Rc::new(RefCell::new(value.clone()));
    let on_input_value = current.clone();
    let on_key_value = current.clone();
    let mut input_draft = draft.clone();
    let clear_button = on_clear.as_ref().map(|name| {
        pattern_spec(
            "button",
            [
                ("variant", Value::string("ghost")),
                ("size", Value::string("sm")),
                ("icon", Value::string("x")),
                ("aria-label", Value::string("Clear input")),
                (
                    "className",
                    Value::string(
                        "absolute inset-y-0 right-0 text-muted-foreground hover:text-foreground",
                    ),
                ),
                ("action", Value::string(name.clone())),
            ],
        )
    });
    let clear_inert = pattern_spec(
        "button",
        [
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("icon", Value::string("x")),
            ("aria-label", Value::string("Clear input")),
            (
                "className",
                Value::string(
                    "absolute inset-y-0 right-0 text-muted-foreground hover:text-foreground",
                ),
            ),
        ],
    );
    rsx! {
        div { class: "w-full",
            {label_view}
            div { class: "relative w-full",
                {left_view}
                input {
                    class: "{class}",
                    r#type: "{input_type}",
                    placeholder: "{placeholder}",
                    value: "{value}",
                    disabled: disabled,
                    autofocus: auto_focus,
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
                    ..attrs,
                }
                if show_clear {
                    {registry.render_spec(clear_button.as_ref().unwrap_or(&clear_inert), None, &view)}
                }
                if !show_clear {
                    if let Some(name) = right_icon.as_ref() {
                        div { class: "absolute inset-y-0 right-0 pr-3 flex items-center text-muted-foreground", {registry.render_spec(&icon_spec(name), None, &view)} }
                    }
                }
            }
            {description_view}
        }
    }
}
