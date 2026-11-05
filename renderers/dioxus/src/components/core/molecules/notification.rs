use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    object_value, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_bool_or, get_string, get_string_or, };
use orbital_core::Value;

/// Dioxus renderer for the `notification` pattern (web `Toast`).
pub struct ToastRenderer;

#[derive(Props, Clone, PartialEq)]
struct ToastProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn Toast(input: ToastProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let variant = get_string_or(props, "variant", "info");
    let message = get_string_or(props, "message", "");
    let title = get_string(props, "title").filter(|title| !title.is_empty());
    let dismissible = get_bool_or(props, "dismissible", true);
    let action_label = get_string(props, "actionLabel").filter(|label| !label.is_empty());
    let dismiss_event = get_string(props, "dismissEvent");
    let action_event = get_string(props, "actionEvent");
    let badge = props
        .as_object()
        .and_then(|fields| fields.get("badge"))
        .cloned();
    let mut leaving = use_signal(|| false);

    let (variant_class, icon_name, icon_color) = match variant.as_str() {
        "success" => (
            "bg-card border-[length:var(--border-width)] border-success",
            "check-circle",
            "text-success",
        ),
        "error" => (
            "bg-card border-[length:var(--border-width)] border-error",
            "alert-circle",
            "text-error",
        ),
        "warning" => (
            "bg-card border-[length:var(--border-width)] border-warning",
            "alert-triangle",
            "text-warning",
        ),
        _ => (
            "bg-card border-[length:var(--border-width)] border-info",
            "info",
            "text-info",
        ),
    };
    let role = if variant == "error" || variant == "warning" {
        "alert"
    } else {
        "status"
    };
    let class = merge_class_name(
        &format!(
            "border-l-heavy p-4 shadow-elevation-toast min-w-0 sm:min-w-[300px] max-w-md max-w-[calc(100vw-2rem)] rounded-container surface-material {} {variant_class}",
            if leaving() { "animate-toast-out" } else { "animate-toast-in" }
        ),
        props,
    );
    let attrs: Vec<Attribute> = shared_root_attrs(props)
        .into_iter()
        .filter(|attr| attr.name != "role")
        .collect();

    let icon = pattern_spec(
        "icon",
        [
            ("name", Value::string(icon_name)),
            ("size", Value::string("md")),
            ("className", Value::string(icon_color)),
        ],
    );
    let title_spec = title.map(|title| {
        pattern_spec(
            "typography",
            [
                ("variant", Value::string("h6")),
                ("className", Value::string("mb-1")),
                ("content", Value::String(title.into())),
            ],
        )
    });
    let message_spec = pattern_spec(
        "typography",
        [
            ("variant", Value::string("small")),
            ("className", Value::string("text-sm")),
            ("content", Value::String(message.into())),
        ],
    );
    let action_spec = action_label
        .as_ref()
        .filter(|_| action_event.is_some())
        .map(|label| {
            pattern_spec(
                "button",
                [
                    ("variant", Value::string("ghost")),
                    ("size", Value::string("sm")),
                    ("label", Value::String(label.clone().into())),
                ],
            )
        });
    let badge_spec = badge.map(|badge| {
        pattern_spec(
            "badge",
            [
                ("variant", Value::string("default")),
                ("size", Value::string("sm")),
                ("label", badge),
            ],
        )
    });
    let dismiss_spec = pattern_spec(
        "button",
        [
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("icon", Value::string("x")),
            ("label", Value::string("")),
            ("aria-label", Value::string("Dismiss toast")),
            ("className", Value::string("flex-shrink-0")),
        ],
    );
    let sink = view.sink;
    let action_sink = sink;
    let action_event_name = action_event.clone();

    rsx! {
        div { class: "{class}", role: "{role}", ..attrs,
            div { class: "flex items-start gap-3",
                div { class: "flex-shrink-0 mt-0.5", {registry.render_spec(&icon, None, &view)} }
                div { class: "flex-1 min-w-0",
                    if let Some(spec) = title_spec.as_ref() { {registry.render_spec(spec, None, &view)} }
                    {registry.render_spec(&message_spec, None, &view)}
                    if let Some(spec) = action_spec.as_ref() {
                        div { class: "mt-3",
                            div { style: "display:contents",
                                onclick: move |_| {
                                    if let (Some(sink), Some(event)) = (action_sink, &action_event_name) {
                                        sink.call((event.clone(), object_value::<&str>([])));
                                    }
                                },
                                {registry.render_spec(spec, None, &view)}
                            }
                        }
                    }
                }
                div { class: "flex items-start gap-2 flex-shrink-0",
                    if let Some(spec) = badge_spec.as_ref() { {registry.render_spec(spec, None, &view)} }
                    if dismissible {
                        div { style: "display:contents",
                            onclick: move |_| {
                                if leaving() { return; }
                                leaving.set(true);
                                if let (Some(sink), Some(event)) = (sink, &dismiss_event) {
                                    sink.call((event.clone(), object_value::<&str>([])));
                                }
                            },
                            {registry.render_spec(&dismiss_spec, None, &view)}
                        }
                    }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for ToastRenderer {
    fn pattern_name(&self) -> &str {
        "notification"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { Toast { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
