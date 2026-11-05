use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `radio` pattern.
pub struct RadioRenderer;

impl RadioRenderer {
    const NAME: &str = "radio";
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
        let mut options = get_string_array(props, "options");
        let selected = get_string_or(props, "value", "");
        let label = get_string_or(props, "label", "");
        let helper = get_string_or(props, "helperText", "");
        let error = get_string_or(props, "error", "");
        let disabled = get_bool_or(props, "disabled", false);
        let required = get_bool_or(props, "required", false);
        let name = get_string_or(props, "name", "");
        let size = get_string_or(props, "size", "md");
        let (outer_size, dot_size) = match size.as_str() {
            "sm" => ("w-4 h-4", "w-2 h-2"),
            "lg" => ("w-6 h-6", "w-3 h-3"),
            _ => ("w-5 h-5", "w-2.5 h-2.5"),
        };
        if options.is_empty() {
            options.push(if selected.is_empty() {
                "on".to_string()
            } else {
                selected.clone()
            });
        }
        let single = props
            .as_object()
            .and_then(|fields| fields.get("options"))
            .is_none();
        let chosen = if single {
            if get_bool(props, "checked") == Some(true)
                || (get_bool(props, "checked").is_none()
                    && get_bool_or(props, "defaultChecked", false))
            {
                Some(options[0].clone())
            } else {
                None
            }
        } else if selected.is_empty() {
            None
        } else {
            Some(selected)
        };
        let action = get_string(props, "action");
        let sink = view.sink;
        let class = merge_class_name("flex flex-col gap-3", props);
        rsx! { div { class: "{class}",
            ..shared_root_attrs(props),
            dioxus_primitives::radio_group::RadioGroup {
                value: chosen,
                disabled: disabled,
                required: required,
                name: name,
                aria_label: "{label}",
                on_value_change: move |value: String| {
                    if let (Some(sink), Some(action)) = (sink, &action) {
                        let mut payload = orbital_core::ValueMap::new();
                        payload.insert("value".into(), Value::String(value.into()));
                        sink.call((action.clone(), Value::Object(payload.into())));
                    }
                },
                for (idx, option) in options.iter().enumerate() {
                    dioxus_primitives::radio_group::RadioItem {
                        key: "{option}", value: option.clone(), index: idx, disabled: disabled,
                        class: "group flex items-start gap-3 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed",
                        span { class: "flex items-center justify-center flex-shrink-0 mt-0.5 border-[length:var(--border-width)] border-border transition-all cursor-pointer group-data-[state=checked]:border-primary group-data-[state=checked]:bg-primary {outer_size}",
                            span { class: "hidden group-data-[state=checked]:block transition-all bg-primary-foreground {dot_size}" }
                        }
                        span { class: "block text-sm font-medium cursor-pointer select-none text-foreground", if single { "{label}" } else { "{option}" } }
                    }
                }
            }
            if !error.is_empty() { span { class: "mt-1.5 ml-8 text-sm text-error font-medium", role: "alert", "{error}" } }
            if error.is_empty() && !helper.is_empty() { span { class: "mt-1.5 ml-8 text-sm text-muted-foreground", "{helper}" } }
        } }
    }
}
