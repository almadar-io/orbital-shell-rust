use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    fanout_sink, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_number_or, get_string};
use orbital_core::client::recurse::child_patterns;
use orbital_core::Value;

const SLOT_DISMISS: &str = "slot:dismiss";

pub struct ToastSlotRenderer;

#[derive(Props, Clone, PartialEq)]
struct ToastSlotProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

#[allow(non_snake_case)]
fn ToastSlot(input: ToastSlotProps) -> Element {
    let dismiss = use_hook(|| fanout_sink(input.view.sink, SLOT_DISMISS, &["DISMISS", "CLOSE"]));
    let view = input.view.as_view();
    let props = &input.props;
    let children = child_patterns(props, view.trait_content);
    if children.is_empty() {
        return rsx! {};
    }
    let only = match children.as_slice() {
        [(only, _)] => Some(only),
        _ => None,
    };
    let element = only.filter(|child| child.as_object().is_some());
    let (message, extracted_variant, extracted_title) = match only {
        Some(child) if element.is_some() => (
            non_empty(get_string(child, "message")),
            non_empty(get_string(child, "variant")),
            non_empty(get_string(child, "title")),
        ),
        Some(Value::String(text)) => (non_empty(Some(text.to_string())), None, None),
        _ => (None, None, None),
    };
    let variant = non_empty(get_string(props, "variant"))
        .or(extracted_variant)
        .unwrap_or_else(|| "info".to_string());
    let title = non_empty(get_string(props, "title")).or(extracted_title);
    let custom = element.is_some() && message.is_none();
    let toast = (!custom).then(|| {
        let mut fields = vec![
            ("variant", Value::string(variant)),
            (
                "message",
                Value::string(message.unwrap_or_else(|| "Notification".to_string())),
            ),
            (
                "duration",
                Value::Number(get_number_or(props, "duration", 5000.0)),
            ),
            ("dismissEvent", Value::string(SLOT_DISMISS)),
        ];
        if let Some(title) = title {
            fields.push(("title", Value::string(title)));
        }
        if let Some(class) = get_string(props, "className") {
            fields.push(("className", Value::string(class)));
        }
        pattern_spec("notification", fields)
    });
    let slot_view = view.for_source(view.source_trait.clone(), dismiss);
    rsx! {
        div { class: "fixed bottom-4 right-4 z-50", ..shared_root_attrs(props),
            if let Some(toast) = toast.as_ref() {
                {input.registry.render_spec(toast, None, &slot_view)}
            } else {
                for (child, source) in children.iter() {
                    {slot_view.render_child(&input.registry, child, source.as_deref())}
                }
            }
        }
    }
}

impl DioxusPatternRenderer for ToastSlotRenderer {
    fn pattern_name(&self) -> &str {
        "toast-slot"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { ToastSlot { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
