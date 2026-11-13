//! Native Dioxus renderer for the `error-state` pattern: the web `ErrorState`.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, merge_class_name};
use crate::renderer::{pattern_spec as pattern, DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::{Value, ValueMap};

/// Dioxus renderer for the `error-state` pattern.
pub struct ErrorStateRenderer;

impl DioxusPatternRenderer for ErrorStateRenderer {
    fn pattern_name(&self) -> &str {
        "error-state"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let title = get_string(props, "title").unwrap_or_else(|| crate::i18n::t("error.generic"));
        let message = get_string(props, "message")
            .or_else(|| get_string(props, "description"))
            .unwrap_or_else(|| crate::i18n::t("error.occurred"));
        let icon = pattern(
            "icon",
            [
                ("name", Value::string("alert-circle")),
                ("className", Value::string("h-8 w-8 text-error")),
            ],
        );
        let title_spec = pattern(
            "typography",
            [
                ("variant", Value::string("h3")),
                ("content", Value::string(title)),
                ("className", Value::string("text-lg text-foreground")),
            ],
        );
        let message_spec = pattern(
            "typography",
            [
                ("variant", Value::string("small")),
                ("content", Value::string(message)),
                (
                    "className",
                    Value::string("mt-1 text-muted-foreground max-w-sm"),
                ),
            ],
        );
        let retry_event = get_string(props, "retryEvent").filter(|event| !event.is_empty());
        let on_retry = get_string(props, "onRetry").filter(|event| !event.is_empty());
        let retry = retry_event
            .clone()
            .or_else(|| on_retry.clone())
            .map(|event| {
                pattern(
                    "button",
                    [
                        ("label", Value::string(crate::i18n::t("error.retry"))),
                        ("action", Value::string(event)),
                        ("actionPayload", Value::Object(ValueMap::new().into())),
                        ("variant", Value::string("secondary")),
                        ("className", Value::string("mt-4")),
                    ],
                )
            });
        let second_event = if retry_event.is_some() {
            on_retry
        } else {
            None
        };
        let retry_view = view.sink.zip(second_event).map(|(sink, second_event)| {
            let relay: EventSink = Callback::new(move |(event, payload): (String, Value)| {
                sink.call((event, payload));
                sink.call((second_event.clone(), Value::Object(ValueMap::new().into())));
            });
            view.for_source(view.source_trait.clone(), relay)
        });
        let class = merge_class_name(
            &cn(&[
                "flex flex-col gap-4 items-center",
                "justify-center py-12 text-center",
            ]),
            props,
        );
        let icon_box = cn(&[
            "bg-transparent rounded-none shadow-none",
            "mb-4 rounded-full bg-error/10 p-3",
        ]);
        rsx! {
            div { class: "contents", ..shared_root_attrs(props),
                div { class: "{class}",
                    div { class: "{icon_box}",
                        {registry.render_spec(&icon, None, view)}
                    }
                    {registry.render_spec(&title_spec, None, view)}
                    {registry.render_spec(&message_spec, None, view)}
                    if let Some(retry) = retry {
                        {registry.render_spec(&retry, None, retry_view.as_ref().unwrap_or(view))}
                    }
                }
            }
        }
    }
}
