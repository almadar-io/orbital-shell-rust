//! Native Dioxus renderer for the `radio` pattern (web: Radio.tsx): sr-only native radio inputs with a styled label as the visible control.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{cn, passthrough_attrs, DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Dioxus renderer for the `radio` pattern.
pub struct RadioRenderer;

impl RadioRenderer {
    const NAME: &str = "radio";
}

static NEXT_RADIO_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Props, Clone, PartialEq)]
struct RadioBodyProps {
    props: Value,
    sink: Option<EventSink>,
}

impl DioxusPatternRenderer for RadioRenderer {
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
        rsx! { RadioBody { props: props.clone(), sink: view.sink } }
    }
}

fn control_class(size: &str, has_error: bool, is_checked: bool, disabled: bool) -> String {
    let size_class = match size {
        "sm" => "w-4 h-4",
        "lg" => "w-6 h-6",
        _ => "w-5 h-5",
    };
    cn(&[
        "flex items-center justify-center",
        "border-[length:var(--border-width)] transition-all cursor-pointer",
        size_class,
        if has_error {
            "border-error peer-focus-visible:ring-error/20"
        } else {
            "border-border peer-focus-visible:ring-ring/20"
        },
        if is_checked {
            if has_error {
                "border-error"
            } else {
                "border-primary bg-primary"
            }
        } else {
            ""
        },
        "peer-focus:outline-none peer-focus-visible:ring-2 peer-focus-visible:ring-offset-2",
        if disabled {
            "opacity-50 cursor-not-allowed"
        } else {
            ""
        },
        if disabled {
            ""
        } else {
            "hover:border-[var(--color-border-hover)]"
        },
    ])
}

fn dot_class(size: &str, has_error: bool) -> String {
    let size_class = match size {
        "sm" => "w-2 h-2",
        "lg" => "w-3 h-3",
        _ => "w-2.5 h-2.5",
    };
    cn(&[
        "transition-all",
        size_class,
        if has_error {
            "bg-error"
        } else {
            "bg-primary-foreground"
        },
    ])
}

fn label_class(has_error: bool, disabled: bool) -> String {
    cn(&[
        "block text-sm font-medium cursor-pointer select-none",
        if has_error {
            "text-error"
        } else {
            "text-foreground"
        },
        if disabled {
            "opacity-50 cursor-not-allowed"
        } else {
            ""
        },
    ])
}

fn input_attrs(props: &Value) -> Vec<Attribute> {
    passthrough_attrs(props, &[])
        .into_iter()
        .filter(|attr| {
            !matches!(
                attr.name,
                "role" | "aria-label" | "aria-invalid" | "aria-required" | "aria-describedby"
            )
        })
        .collect()
}

#[allow(non_snake_case)]
fn RadioBody(input: RadioBodyProps) -> Element {
    let props = &input.props;
    let generated = use_hook(|| NEXT_RADIO_ID.fetch_add(1, Ordering::Relaxed));
    let base_id = get_string(props, "id").unwrap_or_else(|| format!("radio-{generated}"));
    let options = get_string_array(props, "options");
    let value = get_string(props, "value");
    let label = get_string(props, "label").filter(|text| !text.is_empty());
    let helper = get_string(props, "helperText").filter(|text| !text.is_empty());
    let error = get_string(props, "error").filter(|text| !text.is_empty());
    let disabled = get_bool_or(props, "disabled", false);
    let required = get_bool_or(props, "required", false);
    let size = get_string_or(props, "size", "md");
    let has_error = error.is_some();
    let action = get_string(props, "action");
    let sink = input.sink;
    let clicked = use_signal(|| None::<(Option<String>, String)>);
    let selected: Option<String> = match clicked() {
        Some((base, picked)) if base == value => Some(picked),
        _ => value.clone(),
    };
    let described_by = {
        let extra = get_string(props, "aria-describedby");
        let own = if error.is_some() {
            Some(format!("{base_id}-error"))
        } else if helper.is_some() {
            Some(format!("{base_id}-helper"))
        } else {
            None
        };
        let joined = [extra, own]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        (!joined.is_empty()).then_some(joined)
    };
    let pick = {
        let base = value.clone();
        move |next: String| {
            let mut state = clicked;
            state.set(Some((base.clone(), next.clone())));
            if let (Some(sink), Some(action)) = (sink, &action) {
                let mut payload = orbital_core::ValueMap::new();
                payload.insert("value".into(), Value::String(next.into()));
                sink.call((action.clone(), Value::Object(payload.into())));
            }
        }
    };
    let helper_node = if error.is_some() || helper.is_some() {
        rsx! {
            div { class: "mt-1.5 ml-8",
                if let Some(text) = error.as_ref() {
                    p { id: "{base_id}-error", class: "text-sm text-error font-medium", role: "alert", "{text}" }
                }
                if error.is_none() {
                    if let Some(text) = helper.as_ref() {
                        p { id: "{base_id}-helper", class: "text-sm text-muted-foreground", "{text}" }
                    }
                }
            }
        }
    } else {
        rsx! {}
    };
    let class_name = get_string_or(props, "className", "");
    let input_class = cn(&["sr-only peer", &class_name]);
    let dot = dot_class(&size, has_error);
    let text_class = label_class(has_error, disabled);

    if !options.is_empty() {
        let group_name = get_string(props, "name").unwrap_or_else(|| base_id.clone());
        let group_label = get_string(props, "aria-label").or_else(|| {
            if get_string(props, "aria-labelledby").is_some() {
                None
            } else {
                label.clone()
            }
        });
        let group_attrs = input_attrs(props);
        let rows = options.iter().enumerate().map(|(index, option)| {
            let input_id = format!("{base_id}-{index}");
            let is_checked = selected.as_deref() == Some(option.as_str());
            let control = control_class(&size, has_error, is_checked, disabled);
            let option_value = option.clone();
            let on_pick = pick.clone();
            rsx! {
                div { key: "{option}", class: "flex items-start gap-3",
                    div { class: "relative flex-shrink-0 mt-0.5",
                        input {
                            r#type: "radio",
                            id: "{input_id}",
                            name: "{group_name}",
                            value: "{option}",
                            checked: is_checked,
                            disabled: disabled,
                            required: required,
                            class: "{input_class}",
                            onchange: move |_| on_pick(option_value.clone()),
                        }
                        label { r#for: "{input_id}", class: "{control}",
                            if is_checked { div { class: "{dot}" } }
                        }
                    }
                    div { class: "flex-1 min-w-0",
                        label { r#for: "{input_id}", class: "{text_class}", "{option}" }
                    }
                }
            }
        });
        return rsx! {
            div {
                class: "flex flex-col gap-3",
                role: "radiogroup",
                aria_label: group_label,
                aria_invalid: "{has_error}",
                aria_required: required.then_some("true"),
                aria_describedby: described_by,
                ..group_attrs,
                for row in rows { {row} }
            }
            {helper_node}
        };
    }

    let own_value = value.clone().unwrap_or_else(|| base_id.clone());
    let is_checked = match get_bool(props, "checked") {
        Some(flag) => flag,
        None => selected.as_deref() == Some(own_value.as_str()),
    };
    let control = control_class(&size, has_error, is_checked, disabled);
    let attrs = input_attrs(props);
    rsx! {
        div { class: "flex items-start gap-3",
            div { class: "relative flex-shrink-0 mt-0.5",
                input {
                    r#type: "radio",
                    id: "{base_id}",
                    checked: is_checked,
                    disabled: disabled,
                    required: required,
                    class: "{input_class}",
                    aria_invalid: "{has_error}",
                    aria_required: required.then_some("true"),
                    aria_describedby: described_by,
                    onchange: move |_| pick(own_value.clone()),
                    ..attrs,
                }
                label { r#for: "{base_id}", class: "{control}",
                    if is_checked { div { class: "{dot}" } }
                }
            }
            if let Some(text) = label.as_ref() {
                div { class: "flex-1 min-w-0",
                    label { r#for: "{base_id}", class: "{text_class}", "{text}" }
                }
            }
        }
        {helper_node}
    }
}
