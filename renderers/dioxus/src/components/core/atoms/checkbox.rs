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
        let input_class = merge_class_name(
            "peer h-4 w-4 border-[length:var(--border-width)] border-border accent-primary focus:ring-ring focus:ring-offset-0 bg-card checked:bg-primary disabled:opacity-50 disabled:cursor-not-allowed",
            props,
        );
        let label = get_string(props, "label").filter(|text| !text.is_empty());
        let controlled = get_bool(props, "checked");
        let checked = controlled.unwrap_or_else(|| get_bool_or(props, "defaultChecked", false));
        let error = get_string(props, "error").filter(|text| !text.is_empty());
        let disabled = get_bool_or(props, "disabled", false);
        let required = get_bool_or(props, "required", false);
        let field_id = get_string(props, "id");
        let error_id = field_id.as_ref().map(|id| format!("{id}-error"));
        let described_by = {
            let extra = get_string(props, "aria-describedby");
            let joined = [extra, error_id.clone().filter(|_| error.is_some())]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            (!joined.is_empty()).then_some(joined)
        };
        let attrs: Vec<Attribute> = passthrough_attrs(props, &[])
            .into_iter()
            .filter(|attr| {
                !matches!(
                    attr.name,
                    "aria-required" | "aria-invalid" | "aria-describedby"
                )
            })
            .collect();
        let invalid = if error.is_some() {
            Some("true".to_string())
        } else {
            get_string(props, "aria-invalid")
        };
        let name = get_string(props, "name");
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
        let label_for = field_id.clone();

        rsx! {
            div { class: "flex flex-col",
                div { class: "flex items-center",
                    div { class: "relative flex items-center",
                        input {
                            r#type: "checkbox",
                            class: "{input_class}",
                            checked: checked,
                            disabled: disabled,
                            required: required,
                            id: field_id.clone(),
                            name: name,
                            aria_required: required.then_some("true"),
                            aria_invalid: invalid,
                            aria_describedby: described_by,
                            onchange: move |event| emit_change(event.checked()),
                            ..attrs,
                        }
                    }
                    if let Some(text) = label.as_ref() {
                        label {
                            r#for: label_for.clone(),
                            class: "ml-2 text-sm text-foreground font-medium cursor-pointer select-none",
                            onclick: move |_| { if label_for.is_none() && !disabled { emit_label_change(!checked) } },
                            "{text}"
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
