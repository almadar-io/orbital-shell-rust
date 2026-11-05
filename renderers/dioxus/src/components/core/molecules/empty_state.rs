use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::{Value, ValueMap};

pub struct EmptyStateRenderer;

fn pattern(name: &str, fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    let mut props = ValueMap::new();
    props.insert("type".into(), Value::string(name));
    for (key, value) in fields {
        props.insert(key.into(), value);
    }
    Value::Object(props.into())
}

impl DioxusPatternRenderer for EmptyStateRenderer {
    fn pattern_name(&self) -> &str {
        "empty-state"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let title = get_string(props, "title")
            .filter(|text| !text.is_empty())
            .or_else(|| get_string(props, "message").filter(|text| !text.is_empty()))
            .unwrap_or_else(|| "No items".to_string());
        let description = get_string(props, "description").filter(|text| !text.is_empty());
        let destructive = get_bool_or(props, "destructive", false)
            || get_string_or(props, "variant", "default") == "error";
        let success = !destructive && get_string_or(props, "variant", "default") == "success";
        let color = if destructive {
            "error"
        } else if success {
            "success"
        } else {
            "primary"
        };
        let icon_name = get_string(props, "icon")
            .filter(|name| !name.is_empty())
            .map(|name| match name.as_str() {
                "check" => "check-circle".to_string(),
                "error" => "x-circle".to_string(),
                "warning" => "alert-circle".to_string(),
                _ => name,
            });
        let icon = icon_name.map(|name| {
            pattern(
                "icon",
                [
                    ("name", Value::string(name)),
                    ("size", Value::string("xl")),
                    (
                        "color",
                        Value::string(if destructive {
                            "error"
                        } else if success {
                            "success"
                        } else {
                            "muted"
                        }),
                    ),
                ],
            )
        });
        let title_spec = pattern(
            "typography",
            [
                ("variant", Value::string("h3")),
                ("content", Value::string(title)),
                ("color", Value::string(color)),
                ("className", Value::string("text-lg")),
            ],
        );
        let description_spec = description.map(|text| {
            pattern(
                "typography",
                [
                    ("variant", Value::string("small")),
                    ("content", Value::string(text)),
                    ("color", Value::string("muted")),
                    ("className", Value::string("mt-1 max-w-sm")),
                ],
            )
        });
        let action_event = get_string(props, "actionEvent").filter(|event| !event.is_empty());
        let on_action = get_string(props, "onAction").filter(|event| !event.is_empty());
        let action_label = get_string(props, "actionLabel").filter(|label| !label.is_empty());
        let action = action_label
            .zip(action_event.clone().or_else(|| on_action.clone()))
            .map(|(label, event)| {
                pattern(
                    "button",
                    [
                        ("label", Value::string(label)),
                        ("action", Value::string(event)),
                        ("actionPayload", Value::Object(ValueMap::new().into())),
                        (
                            "variant",
                            Value::string(if destructive { "danger" } else { "primary" }),
                        ),
                        ("className", Value::string("mt-4")),
                    ],
                )
            });
        let second_event = if action_event.is_some() {
            on_action
        } else {
            None
        };
        let action_view = view.sink.zip(second_event).map(|(sink, second_event)| {
            let relay: EventSink = Callback::new(move |(event, payload): (String, Value)| {
                sink.call((event, payload));
                sink.call((second_event.clone(), Value::Object(ValueMap::new().into())));
            });
            view.for_source(view.source_trait.clone(), relay)
        });
        let look = get_string_or(props, "look", "icon-only");
        let look_class = match look.as_str() {
            "illustrated" => "[&_svg]:w-32 [&_svg]:h-32",
            "text-only" => "[&_svg]:hidden",
            "mascot" => "[&_svg]:w-24 [&_svg]:h-24 [&_svg]:rounded-pill",
            _ => "",
        };
        let icon_bg = if destructive {
            "bg-error/10"
        } else if success {
            "bg-success/10"
        } else {
            "bg-muted"
        };
        let class = merge_class_name(
            &format!("flex flex-col items-center justify-center py-12 text-center {look_class}"),
            props,
        );
        rsx! {
            div { class: "contents", "data-empty-state": "", ..shared_root_attrs(props),
                div { class: "{class}",
                    if let Some(icon) = icon {
                        div { class: "mb-4 rounded-full p-3 {icon_bg}", {registry.render_spec(&icon, None, view)} }
                    }
                    {registry.render_spec(&title_spec, None, view)}
                    if let Some(description) = description_spec { {registry.render_spec(&description, None, view)} }
                    if let Some(action) = action {
                        {registry.render_spec(&action, None, action_view.as_ref().unwrap_or(view))}
                    }
                }
            }
        }
    }
}
