use crate::registry::DioxusPatternRegistry;
use crate::renderer::merge_class_name;
use crate::renderer::{passthrough_attrs, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct CheckboxRenderer;

impl CheckboxRenderer {
    const NAME: &str = "checkbox";

    pub(crate) fn render_with_change(
        &self,
        props: &Value,
        view: &DioxusView<'_>,
        on_change: Option<Callback<bool>>,
    ) -> Element {
        let class = merge_class_name("flex flex-col", props);
        let label = get_string_or(props, "label", "");
        let accessible_label = get_string(props, "aria-label").unwrap_or_else(|| label.clone());
        let controlled = get_bool(props, "checked");
        let checked = controlled.unwrap_or_else(|| get_bool_or(props, "defaultChecked", false));
        let error = get_string(props, "error").filter(|text| !text.is_empty());
        let disabled = get_bool_or(props, "disabled", false);
        let required = get_bool_or(props, "required", false);
        let field_id = get_string(props, "id");
        let error_id = field_id.as_ref().map(|id| format!("{id}-error"));
        let mut defaults: Vec<(&'static str, Value)> = Vec::new();
        if required {
            defaults.push(("aria-required", Value::Bool(true)));
        }
        if error.is_some() {
            defaults.push(("aria-invalid", Value::Bool(true)));
        }
        let attrs = passthrough_attrs(props, &defaults);
        let action = get_string(props, "onChange");
        let sink = view.sink;
        let emit_change = move |next_checked: bool| {
            if let Some(on_change) = on_change {
                on_change.call(next_checked);
            }
            if let (Some(sink), Some(action)) = (sink, &action) {
                let mut payload = orbital_core::ValueMap::new();
                payload.insert("checked".into(), Value::Bool(next_checked));
                sink.call((action.clone(), Value::Object(payload.into())));
            }
        };
        let emit_label_change = emit_change.clone();
        let state = if checked {
            dioxus_primitives::checkbox::CheckboxState::Checked
        } else {
            dioxus_primitives::checkbox::CheckboxState::Unchecked
        };

        rsx! {
            div {
                class: "{class}",
                ..attrs,
                div { class: "flex items-center",
                dioxus_primitives::checkbox::Checkbox {
                    class: "peer h-4 w-4 shrink-0 flex items-center justify-center border-[length:var(--border-width)] border-border bg-card focus:ring-ring focus:ring-offset-0 disabled:opacity-50 disabled:cursor-not-allowed data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground",
                    checked: controlled.map(|_| state),
                    default_checked: state,
                    aria_label: "{accessible_label}",
                    disabled: disabled,
                    required: required,
                    on_checked_change: move |state: dioxus_primitives::checkbox::CheckboxState| {
                        emit_change(bool::from(state));
                    },
                    dioxus_primitives::checkbox::CheckboxIndicator {
                        class: "hidden data-[state=checked]:block data-[state=indeterminate]:block text-xs leading-none",
                        span {
                            style: "display:block;width:5px;height:9px;border-right:2px solid currentColor;border-bottom:2px solid currentColor;transform:rotate(45deg);margin-top:-2px"
                        }
                    }
                }
                if !label.is_empty() {
                    span {
                        class: "ml-2 text-sm text-foreground font-medium cursor-pointer select-none",
                        onclick: move |_| { if !disabled { emit_label_change(!checked) } },
                        "{label}"
                        if required { span { aria_hidden: "true", class: "text-error ml-1", "*" } }
                    }
                }
                }
                if let Some(text) = error.as_ref() {
                    span { id: error_id.clone(), class: "text-xs font-normal text-error mt-1", "{text}" }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for CheckboxRenderer {
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
        self.render_with_change(props, view, None)
    }
}
