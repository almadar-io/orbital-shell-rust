use crate::registry::DioxusPatternRegistry;
use crate::renderer::merge_class_name;
use crate::renderer::{cn, passthrough_attrs, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `switch` pattern.
pub struct SwitchRenderer;

impl SwitchRenderer {
    const NAME: &str = "switch";
}

impl DioxusPatternRenderer for SwitchRenderer {
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
        let class = merge_class_name("inline-flex items-center gap-2", props);
        let label = get_string_or(props, "label", "");
        let controlled = get_bool(props, "checked");
        let default_checked = get_bool_or(props, "defaultChecked", false);
        let disabled = get_bool_or(props, "disabled", false);
        let id = get_string_or(props, "id", "");
        let attrs = passthrough_attrs(props, &[]);
        let on_change = get_string(props, "onChange").filter(|name| !name.is_empty());
        let label_class = cn(&[
            "text-sm font-medium leading-none cursor-pointer",
            if disabled {
                "cursor-not-allowed opacity-70"
            } else {
                ""
            },
        ]);
        let name = get_string_or(props, "name", "");
        let action = get_string(props, "action");
        let action_payload = props
            .as_object()
            .and_then(|fields| fields.get("actionPayload"))
            .and_then(Value::as_object)
            .cloned();
        let sink = view.sink;
        rsx! {
            div { class: "{class}", ..attrs,
                dioxus_primitives::switch::Switch {
                    class: "group relative inline-flex h-[1.5rem] w-[2.75rem] shrink-0 cursor-pointer items-center rounded-full border-heavy border-transparent transition-colors duration-fast bg-muted data-[state=checked]:bg-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:cursor-not-allowed disabled:opacity-50",
                    checked: controlled,
                    default_checked: default_checked,
                    disabled: disabled,
                    name: name,
                    on_checked_change: move |next: bool| {
                        if let (Some(sink), Some(name)) = (sink, &on_change) {
                            let mut payload = orbital_core::ValueMap::new();
                            payload.insert("checked".into(), Value::Bool(next));
                            sink.call((name.clone(), Value::Object(payload.into())));
                        }
                        if let (Some(sink), Some(action)) = (sink, &action) {
                            let mut payload = action_payload.clone().unwrap_or_default();
                            payload.insert("checked".into(), Value::Bool(next));
                            sink.call((action.clone(), Value::Object(payload.into())));
                        }
                    },
                    dioxus_primitives::switch::SwitchThumb { class: "pointer-events-none block h-[1.25rem] w-[1.25rem] rounded-full bg-background shadow-elevation-interactive ring-0 transition-transform duration-fast translate-x-0 group-data-[state=checked]:translate-x-[1.25rem]" }
                }
                if !label.is_empty() { label { class: "{label_class}", for: "{id}", "{label}" } }
            }
        }
    }
}
