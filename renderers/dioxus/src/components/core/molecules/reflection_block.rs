//! Native body of the `reflection-block` pattern, mirroring the web `ReflectionBlock` molecule.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    merge_class_name, object_value, pattern_spec, DioxusPatternRenderer, DioxusView,
    OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `reflection-block` pattern.
pub struct ReflectionBlockRenderer;

impl ReflectionBlockRenderer {
    const NAME: &str = "reflection-block";
}

#[derive(Props, Clone, PartialEq)]
struct ReflectionProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn Reflection(input: ReflectionProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let prompt = get_string_or(props, "prompt", "");
    let saved = get_string_or(props, "savedNote", "");
    let event = get_string_or(props, "saveEvent", "SAVE_REFLECTION");
    let index = props.get("index").cloned();
    let initial = saved.clone();
    let note = use_signal(|| initial);
    let expanded = use_signal(|| false);
    let class = merge_class_name(
        "my-6 border-l-heavy border-warning bg-warning/10 rounded-r-container p-4",
        props,
    );
    let icon = registry.render_spec(
        &pattern_spec(
            "icon",
            [
                ("name", Value::string("pause-circle")),
                ("size", Value::string("sm")),
                (
                    "className",
                    Value::string("w-5 h-5 text-warning flex-shrink-0 mt-1"),
                ),
            ],
        ),
        None,
        &view,
    );
    let toggle_label = if saved.is_empty() {
        crate::i18n::t("reflection.answerQuestion")
    } else {
        crate::i18n::t("reflection.answeredEdit")
    };
    let current = note();
    let is_expanded = expanded();
    let sink = view.sink;
    rsx! {
        div { class: "{class}", ..shared_root_attrs(props),
            div { class: "flex items-start gap-3",
                {icon}
                div { class: "flex-1",
                    div { class: "font-medium text-warning mb-2", {crate::i18n::t("reflection.title")} }
                    p { class: "text-foreground text-sm mb-3", "{prompt}" }
                    if is_expanded {
                        textarea {
                            class: "w-full p-2 border border-input rounded-interactive text-sm bg-card text-foreground focus:ring-2 focus:ring-ring focus:border-transparent",
                            placeholder: crate::i18n::t("reflection.placeholder"),
                            value: "{current}",
                            rows: "2",
                            oninput: move |event| {
                                let mut note = note;
                                note.set(event.value());
                            },
                        }
                        button {
                            class: "mt-2 text-sm px-3 py-1 bg-warning text-warning-foreground rounded-interactive hover:opacity-90 transition-colors",
                            onclick: move |_| {
                                let mut expanded = expanded;
                                if let Some(sink) = sink {
                                    let mut fields = Vec::new();
                                    if let Some(index) = index.clone() {
                                        fields.push(("index", index));
                                    }
                                    fields.push(("note", Value::string(note())));
                                    sink.call((event.clone(), object_value(fields)));
                                }
                                expanded.set(false);
                            },
                            {crate::i18n::t("reflection.saveContinue")}
                        }
                    } else {
                        button {
                            class: "text-sm text-warning hover:underline",
                            onclick: move |_| {
                                let mut expanded = expanded;
                                expanded.set(true);
                            },
                            "{toggle_label}"
                        }
                    }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for ReflectionBlockRenderer {
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
        rsx! { Reflection { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
