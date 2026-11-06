//! Native body of the `activation-block` pattern, mirroring the web `ActivationBlock` molecule.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    merge_class_name, object_value, pattern_spec, DioxusPatternRenderer, DioxusView,
    OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `activation-block` pattern.
pub struct ActivationBlockRenderer;

impl ActivationBlockRenderer {
    const NAME: &str = "activation-block";
}

#[derive(Props, Clone, PartialEq)]
struct ActivationProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn Activation(input: ActivationProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let question = get_string_or(props, "question", "");
    let saved = get_string_or(props, "savedResponse", "");
    let event = get_string_or(props, "saveEvent", "SAVE_ACTIVATION");
    let initial = saved.clone();
    let response = use_signal(|| initial);
    let expanded = use_signal(|| saved.is_empty());
    let class = merge_class_name(
        "bg-primary/10 border-heavy border-primary rounded-container p-5 mb-6",
        props,
    );
    let icon = registry.render_spec(
        &pattern_spec(
            "icon",
            [
                ("name", Value::string("lightbulb")),
                ("size", Value::string("lg")),
                (
                    "className",
                    Value::string("text-primary flex-shrink-0 mt-1"),
                ),
            ],
        ),
        None,
        &view,
    );
    let current = response();
    let is_expanded = expanded();
    let sink = view.sink;
    let submit_event = event.clone();
    let skip_event = event.clone();
    rsx! {
        div { class: "{class}", ..shared_root_attrs(props),
            div { class: "flex items-start gap-3",
                {icon}
                div { class: "flex-1",
                    h4 { class: "heading-voice text-primary mb-2", "Before You Begin..." }
                    p { class: "text-foreground mb-3 text-sm md:text-base", "{question}" }
                    if is_expanded {
                        textarea {
                            class: "w-full p-3 border border-input rounded-interactive bg-card text-foreground focus:ring-2 focus:ring-ring focus:border-transparent text-sm",
                            placeholder: "Jot down your thoughts...",
                            value: "{current}",
                            rows: "3",
                            oninput: move |event| {
                                let mut response = response;
                                response.set(event.value());
                            },
                        }
                        div { class: "flex gap-2 mt-3",
                            button {
                                class: "px-4 py-2 bg-primary text-primary-foreground rounded-interactive hover:bg-primary-hover text-sm font-medium transition-colors",
                                onclick: move |_| {
                                    let mut expanded = expanded;
                                    if let Some(sink) = sink {
                                        sink.call((submit_event.clone(), object_value([("response", Value::string(response()))])));
                                    }
                                    expanded.set(false);
                                },
                                "Continue to Lesson →"
                            }
                            button {
                                class: "px-4 py-2 text-primary hover:underline text-sm",
                                onclick: move |_| {
                                    let mut expanded = expanded;
                                    if let Some(sink) = sink {
                                        sink.call((skip_event.clone(), object_value([("response", Value::string(""))])));
                                    }
                                    expanded.set(false);
                                },
                                "Skip for now"
                            }
                        }
                    } else {
                        button {
                            class: "text-sm text-primary hover:underline font-medium",
                            onclick: move |_| {
                                let mut expanded = expanded;
                                expanded.set(true);
                            },
                            "✓ Answered · Edit response"
                        }
                    }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for ActivationBlockRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { Activation { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
