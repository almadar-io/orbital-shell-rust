use dioxus::prelude::*;
use orbital_core::Value;
use orbital_core::client::helpers::*;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use crate::registry::DioxusPatternRegistry;

pub struct CheckboxRenderer;

impl CheckboxRenderer {
    const NAME: &str = "checkbox";
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
        let class = merge_class_name("flex items-center", props);
        let label = get_string_or(props, "label", "");
        let checked = get_bool_or(props, "checked", false);
        let action = get_string(props, "onChange");
        let sink = view.sink;
        let emit_change = move |next_checked: bool| {
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
                dioxus_primitives::checkbox::Checkbox {
                    class: "peer h-4 w-4 shrink-0 flex items-center justify-center border-[length:var(--border-width)] border-border bg-card focus:ring-ring focus:ring-offset-0 disabled:opacity-50 disabled:cursor-not-allowed data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground",
                    checked: Some(state),
                    aria_label: "{label}",
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
                        onclick: move |_| emit_label_change(!checked),
                        "{label}"
                    }
                }
            }
        }
    }
}
