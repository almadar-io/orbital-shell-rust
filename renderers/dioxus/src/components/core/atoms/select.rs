use super::field_draft::use_field_draft;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::merge_class_name;
use crate::renderer::{pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

#[derive(Clone, PartialEq)]
struct OptionRow {
    value: String,
    label: String,
    disabled: bool,
    group: Option<String>,
}

#[derive(Props, Clone, PartialEq)]
struct SearchableSelectProps {
    options: Vec<OptionRow>,
    single: ReadSignal<Option<String>>,
    multi: ReadSignal<Option<Vec<String>>>,
    on_single_change: Callback<Option<String>>,
    on_multi_change: Callback<Vec<String>>,
    multiple: bool,
    placeholder: String,
    class: String,
    disabled: bool,
    field_id: Option<String>,
    label: String,
    names: Vec<String>,
    sink: Option<crate::renderer::EventSink>,
}

#[allow(non_snake_case)]
fn SearchableSelect(props: SearchableSelectProps) -> Element {
    let mut query = use_signal(String::new);
    let search = query.read().to_lowercase();
    let visible = props
        .options
        .iter()
        .filter(|option| option.label.to_lowercase().contains(&search))
        .cloned()
        .collect::<Vec<_>>();
    let sink = props.sink;
    let names = props.names;
    let on_single_change = props.on_single_change;
    let on_multi_change = props.on_multi_change;
    let list = rsx! {
        dioxus_primitives::select::SelectList { class: "absolute z-50 mt-1 w-full bg-card border-[length:var(--border-width)] border-border rounded-interactive shadow-elevation-popover py-1 max-h-60 overflow-y-auto",
                input { r#type: "search", class: "w-full px-3 py-2 text-sm bg-card text-foreground border-b border-border", placeholder: "Search...",
                    oninput: move |event| query.set(event.value()),
                    onkeydown: move |event| event.stop_propagation(),
                }
                for (index, option) in visible.iter().enumerate() {
                    div { key: "{option.value}",
                        if let Some(group) = &option.group {
                            if index == 0 || visible[index - 1].group.as_ref() != Some(group) {
                                div { class: "px-3 py-1 text-xs font-bold text-muted-foreground", "{group}" }
                            }
                        }
                        dioxus_primitives::select::SelectOption::<String> { value: option.value.clone(), index: index, disabled: option.disabled,
                            text_value: Some(option.label.clone()),
                            class: "w-full flex items-center justify-between px-3 py-1.5 text-sm text-start hover:bg-muted transition-colors disabled:opacity-50 disabled:cursor-not-allowed aria-selected:text-primary aria-selected:font-medium",
                            "{option.label}"
                        }
                    }
                }
        }
    };
    if props.multiple {
        return rsx! {
            dioxus_primitives::select::SelectMulti::<String> {
                class: "{props.class}",
                values: props.multi,
                disabled: props.disabled,
                on_open_change: move |open: bool| { if !open { query.set(String::new()); } },
                on_values_change: move |values: Vec<String>| {
                    on_multi_change.call(values.clone());
                    let values = values.into_iter().map(|value| Value::String(value.into())).collect::<Vec<_>>();
                    emit(sink, &names, Value::Array(values.into()));
                },
                dioxus_primitives::select::SelectTrigger {
                    id: props.field_id.clone(),
                    aria_label: props.label.clone(),
                    class: "block w-full rounded-interactive border-[length:var(--border-width)] shadow-elevation-interactive interactive-border appearance-none px-3 py-2 pr-10 text-sm text-foreground font-medium bg-card focus:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:bg-muted disabled:text-muted-foreground disabled:cursor-not-allowed border-border focus:border-primary",
                    dioxus_primitives::select::SelectValue { placeholder: props.placeholder }
                }
                {list}
            }
        };
    }
    rsx! {
        dioxus_primitives::select::Select::<String> {
            class: "{props.class}",
            value: Some(props.single),
            disabled: props.disabled,
            on_open_change: move |open: bool| { if !open { query.set(String::new()); } },
            on_value_change: move |value: Option<String>| {
                on_single_change.call(value.clone());
                if let Some(value) = value { emit(sink, &names, Value::String(value.into())); }
            },
            dioxus_primitives::select::SelectTrigger {
                id: props.field_id.clone(),
                aria_label: props.label.clone(),
                class: "block w-full rounded-interactive border-[length:var(--border-width)] shadow-elevation-interactive interactive-border appearance-none px-3 py-2 pr-10 text-sm text-foreground font-medium bg-card focus:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:bg-muted disabled:text-muted-foreground disabled:cursor-not-allowed border-border focus:border-primary",
                dioxus_primitives::select::SelectValue { placeholder: props.placeholder }
            }
            {list}
        }
    }
}

fn rows(props: &Value) -> Vec<OptionRow> {
    let mut rows = Vec::new();
    let mut add = |items: &Value, group: Option<&str>| {
        if let Some(items) = items.as_array() {
            for item in items {
                let (value, label, disabled) = if let Some(text) = item.as_str() {
                    (text.to_string(), text.to_string(), false)
                } else if let Some(fields) = item.as_object() {
                    let value = fields.get("value").and_then(Value::as_str).unwrap_or("");
                    let label = fields.get("label").and_then(Value::as_str).unwrap_or(value);
                    (
                        value.to_string(),
                        label.to_string(),
                        fields
                            .get("disabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    )
                } else {
                    continue;
                };
                rows.push(OptionRow {
                    value,
                    label,
                    disabled,
                    group: group.map(str::to_string),
                });
            }
        }
    };
    if let Some(fields) = props.as_object() {
        if let Some(options) = fields.get("options") {
            add(options, None);
        }
        if let Some(groups) = fields.get("groups").and_then(Value::as_array) {
            for group in groups {
                if let Some(group) = group.as_object() {
                    let label = group.get("label").and_then(Value::as_str).unwrap_or("");
                    if let Some(options) = group.get("options") {
                        add(options, Some(label));
                    }
                }
            }
        }
    }
    rows
}

fn emit(sink: Option<crate::renderer::EventSink>, names: &[String], value: Value) {
    if let Some(sink) = sink {
        for name in names {
            let mut payload = orbital_core::ValueMap::new();
            payload.insert("value".into(), value.clone());
            sink.call((name.clone(), Value::Object(payload.into())));
        }
    }
}

/// Dioxus renderer for the `select` pattern.
pub struct SelectRenderer;

impl SelectRenderer {
    const NAME: &str = "select";
}

impl DioxusPatternRenderer for SelectRenderer {
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
        rsx! { SelectField { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}

#[derive(Props, Clone, PartialEq)]
struct SelectFieldProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn SelectField(field: SelectFieldProps) -> Element {
    let props = &field.props;
    let view = field.view.as_view();
    let registry = &field.registry;
    let chevron_spec = pattern_spec(
        "icon",
        [
            ("name", Value::string("chevron-down")),
            (
                "className",
                Value::string("h-icon-default w-icon-default text-foreground"),
            ),
        ],
    );
    let chevron = rsx! { div { class: "absolute inset-y-0 right-0 pr-3 flex items-center pointer-events-none", {registry.render_spec(&chevron_spec, None, &view)} } };
    let options = rows(props);
    let selected = get_string_or(props, "value", "");
    let mut single_draft = use_field_draft(if selected.is_empty() {
        None
    } else {
        Some(selected)
    });
    let mut multi_draft = use_field_draft(Some(get_string_array(props, "value")));
    let selected = single_draft.value().unwrap_or_default();
    let placeholder = get_string_or(props, "placeholder", "Select...");
    let label = get_string_or(props, "label", "");
    let field_id = get_string(props, "id");
    let root_attrs = shared_root_attrs(props)
        .into_iter()
        .filter(|attribute| attribute.name != "id")
        .collect::<Vec<_>>();
    let error = get_string_or(props, "error", "");
    let disabled = get_bool_or(props, "disabled", false);
    let clearable = get_bool_or(props, "clearable", false);
    let searchable = get_bool_or(props, "searchable", false);
    let mut names = Vec::new();
    for key in ["onChange", "onValueChange", "action"] {
        if let Some(name) = get_string(props, key) {
            names.push(name);
        }
    }
    let sink = view.sink;
    let names_for_clear = names.clone();
    let class = merge_class_name("relative w-full", props);
    let multiple = get_bool_or(props, "multiple", false);
    if searchable {
        let selected_values = multi_draft.value().unwrap_or_default();
        let has_selection = if multiple {
            !selected_values.is_empty()
        } else {
            !selected.is_empty()
        };
        let clear_value = if multiple {
            Value::Array(Vec::new().into())
        } else {
            Value::String("".into())
        };
        let mut single_for_child = single_draft.clone();
        let mut multi_for_child = multi_draft.clone();
        let mut single_for_clear = single_draft.clone();
        let mut multi_for_clear = multi_draft.clone();
        return rsx! { div { class: "w-full",
            ..root_attrs,
            label { class: "block w-full", for: field_id.clone(),
                if !label.is_empty() { span { class: "block text-sm font-bold text-foreground mb-1", "{label}" } }
                SearchableSelect {
                    options, single: single_draft.signal(), multi: multi_draft.signal(),
                    on_single_change: move |value: Option<String>| single_for_child.record(value),
                    on_multi_change: move |values: Vec<String>| multi_for_child.record(Some(values)),
                    multiple, placeholder, class, disabled, field_id: field_id.clone(), label, names, sink
                }
            }
            if clearable && has_selection { button { class: "text-sm text-muted-foreground", onclick: move |_| {
                if multiple { multi_for_clear.record(Some(Vec::new())); } else { single_for_clear.record(None); }
                emit(sink, &names_for_clear, clear_value.clone());
            }, "Clear" } }
            if !error.is_empty() { span { class: "mt-1 text-xs text-error", role: "alert", "{error}" } }
        } };
    }
    if multiple {
        let values = multi_draft.value().unwrap_or_default();
        let mut multi_for_clear = multi_draft.clone();
        return rsx! { div { class: "w-full",
            ..root_attrs,
            label { class: "block w-full", for: field_id.clone(),
            if !label.is_empty() { span { class: "block text-sm font-bold text-foreground mb-1", "{label}" } }
            dioxus_primitives::select::SelectMulti::<String> {
                class: "{class}",
                values: multi_draft.signal(),
                disabled: disabled,
                on_values_change: move |values: Vec<String>| {
                    multi_draft.record(Some(values.clone()));
                    let values = values.into_iter().map(|value| Value::String(value.into())).collect::<Vec<_>>();
                    emit(sink, &names, Value::Array(values.into()));
                },
                dioxus_primitives::select::SelectTrigger {
                    id: field_id.clone(), aria_label: label.clone(),
                    class: "block w-full rounded-interactive border-[length:var(--border-width)] shadow-elevation-interactive interactive-border px-3 py-2 pr-10 text-sm bg-card border-border",
                    dioxus_primitives::select::SelectValue { placeholder: placeholder }
                }
                {chevron.clone()}
                dioxus_primitives::select::SelectList { class: "absolute z-50 mt-1 w-full bg-card border-[length:var(--border-width)] border-border rounded-interactive shadow-elevation-popover py-1 max-h-60 overflow-y-auto",
                    for (index, option) in options.iter().enumerate() {
                        div { key: "{option.value}",
                            if let Some(group) = &option.group {
                                if index == 0 || options[index - 1].group.as_ref() != Some(group) {
                                    div { class: "px-3 py-1 text-xs font-bold text-muted-foreground", "{group}" }
                                }
                            }
                            dioxus_primitives::select::SelectOption::<String> { value: option.value.clone(), index: index, disabled: option.disabled,
                                text_value: Some(option.label.clone()),
                                class: "w-full flex items-center justify-between px-3 py-1.5 text-sm hover:bg-muted", "{option.label}" }
                        }
                    }
                }
            }
            }
            if clearable && !values.is_empty() { button { class: "text-sm text-muted-foreground", onclick: move |_| {
                multi_for_clear.record(Some(Vec::new()));
                emit(sink, &names_for_clear, Value::Array(Vec::new().into()));
            }, "Clear" } }
            if !error.is_empty() { span { class: "mt-1 text-xs text-error", role: "alert", "{error}" } }
        } };
    }
    let mut single_for_clear = single_draft.clone();
    rsx! { div { class: "w-full",
        ..root_attrs,
        label { class: "block w-full", for: field_id.clone(),
        if !label.is_empty() { span { class: "block text-sm font-bold text-foreground mb-1", "{label}" } }
        dioxus_primitives::select::Select::<String> {
            class: "{class}",
            value: Some(single_draft.signal()),
            disabled: disabled,
            on_value_change: move |value: Option<String>| {
                single_draft.record(value.clone());
                if let Some(value) = value { emit(sink, &names, Value::String(value.into())); }
            },
            dioxus_primitives::select::SelectTrigger {
                id: field_id.clone(), aria_label: label.clone(),
                class: "block w-full rounded-interactive border-[length:var(--border-width)] shadow-elevation-interactive interactive-border appearance-none px-3 py-2 pr-10 text-sm text-foreground font-medium bg-card focus:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:bg-muted disabled:text-muted-foreground disabled:cursor-not-allowed border-border focus:border-primary",
                dioxus_primitives::select::SelectValue { placeholder: placeholder }
            }
            {chevron.clone()}
            dioxus_primitives::select::SelectList { class: "absolute z-50 mt-1 w-full bg-card border-[length:var(--border-width)] border-border rounded-interactive shadow-elevation-popover py-1 max-h-60 overflow-y-auto",
                for (index, option) in options.iter().enumerate() {
                    div { key: "{option.value}",
                        if let Some(group) = &option.group {
                            if index == 0 || options[index - 1].group.as_ref() != Some(group) {
                                div { class: "px-3 py-1 text-xs font-bold text-muted-foreground", "{group}" }
                            }
                        }
                        dioxus_primitives::select::SelectOption::<String> {
                            value: option.value.clone(), index: index, disabled: option.disabled,
                            text_value: Some(option.label.clone()),
                            class: "w-full flex items-center justify-between px-3 py-1.5 text-sm text-start hover:bg-muted transition-colors disabled:opacity-50 disabled:cursor-not-allowed aria-selected:text-primary aria-selected:font-medium",
                            "{option.label}"
                        }
                    }
                }
            }
        }
        }
        if clearable && !selected.is_empty() { button { class: "text-sm text-muted-foreground", onclick: move |_| {
            single_for_clear.record(None);
            emit(sink, &names_for_clear, Value::String("".into()));
        }, "Clear" } }
        if !error.is_empty() { span { class: "mt-1 text-xs text-error", role: "alert", "{error}" } }
    } }
}
