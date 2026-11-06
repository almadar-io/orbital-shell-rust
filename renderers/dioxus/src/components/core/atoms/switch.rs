//! Native Dioxus renderer for the `switch` pattern (web: Switch.tsx): a `button role="switch"` with a thumb span.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    cn, merge_class_name, passthrough_attrs, DioxusPatternRenderer, DioxusView, EventSink,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Dioxus renderer for the `switch` pattern.
pub struct SwitchRenderer;

impl SwitchRenderer {
    const NAME: &str = "switch";
}

static NEXT_SWITCH_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Props, Clone, PartialEq)]
struct SwitchBodyProps {
    props: Value,
    sink: Option<EventSink>,
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
        rsx! { SwitchBody { props: props.clone(), sink: view.sink } }
    }
}

#[allow(non_snake_case)]
fn SwitchBody(input: SwitchBodyProps) -> Element {
    let props = &input.props;
    let generated = use_hook(|| NEXT_SWITCH_ID.fetch_add(1, Ordering::Relaxed));
    let controlled = get_bool(props, "checked");
    let internal =
        use_signal(|| controlled.unwrap_or_else(|| get_bool_or(props, "defaultChecked", false)));
    let is_checked = controlled.unwrap_or_else(|| internal());
    let disabled = get_bool_or(props, "disabled", false);
    let label = get_string(props, "label").filter(|text| !text.is_empty());
    let switch_id = get_string(props, "id").unwrap_or_else(|| format!("switch-{generated}"));
    let name = get_string(props, "name");
    let class = merge_class_name("inline-flex items-center gap-2", props);
    let button_class = cn(&[
        "relative inline-flex h-[1.5rem] w-[2.75rem] shrink-0 cursor-pointer items-center rounded-full border-heavy border-transparent transition-colors duration-fast",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background",
        if is_checked { "bg-primary" } else { "bg-muted" },
        if disabled { "cursor-not-allowed opacity-50" } else { "" },
    ]);
    let thumb_class = cn(&[
        "pointer-events-none block h-[1.25rem] w-[1.25rem] rounded-full bg-background shadow-elevation-interactive ring-0 transition-transform duration-fast",
        if is_checked { "translate-x-[1.25rem]" } else { "translate-x-0" },
    ]);
    let label_class = cn(&[
        "text-sm font-medium leading-none cursor-pointer",
        if disabled {
            "cursor-not-allowed opacity-70"
        } else {
            ""
        },
    ]);
    let attrs: Vec<Attribute> = passthrough_attrs(props, &[])
        .into_iter()
        .filter(|attr| !matches!(attr.name, "role" | "aria-checked"))
        .collect();
    let on_change = get_string(props, "onChange").filter(|name| !name.is_empty());
    let action = get_string(props, "action");
    let action_payload = props
        .as_object()
        .and_then(|fields| fields.get("actionPayload"))
        .and_then(Value::as_object)
        .cloned();
    let sink = input.sink;
    let toggle = move || {
        if disabled {
            return;
        }
        let next = !is_checked;
        if controlled.is_none() {
            let mut state = internal;
            state.set(next);
        }
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
    };
    rsx! {
        div { class: "{class}",
            button {
                r#type: "button",
                role: "switch",
                aria_checked: "{is_checked}",
                id: "{switch_id}",
                name: name,
                disabled: disabled,
                class: "{button_class}",
                onclick: move |_| toggle(),
                ..attrs,
                span { class: "{thumb_class}" }
            }
            if let Some(text) = label.as_ref() {
                label {
                    r#for: "{switch_id}",
                    class: "{label_class}",
                    "{text}"
                }
            }
        }
    }
}
