//! Native body of the `quiz-block` pattern, mirroring the web `QuizBlock` molecule.

use crate::components::core::atoms::button::button_class;
use crate::components::core::atoms::card::card_class;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    cn, object_value, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
    BOX_ATOM_BASE,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `quiz-block` pattern.
pub struct QuizBlockRenderer;

impl QuizBlockRenderer {
    const NAME: &str = "quiz-block";
}

#[derive(Props, Clone, PartialEq)]
struct QuizProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn Quiz(input: QuizProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let question = get_string_or(props, "question", "");
    let answer = get_string_or(props, "answer", "");
    let revealed = use_signal(|| false);
    let is_revealed = revealed();
    let card = card_class(&object_value([
        ("padding", Value::string("none")),
        (
            "className",
            Value::string(cn(&[
                "my-4 border-primary",
                get_string_or(props, "className", "").trim(),
            ])),
        ),
    ]));
    let box_p4 = cn(&[BOX_ATOM_BASE, "p-4"]);
    let box_fg = cn(&[BOX_ATOM_BASE, "text-card-foreground"]);
    let box_answer = cn(&[BOX_ATOM_BASE, "pl-7 pt-2 border-t border-border"]);
    let toggle = button_class(&object_value([
        ("variant", Value::string("ghost")),
        ("size", Value::string("sm")),
        ("className", Value::string("self-start ml-7")),
    ]));
    let help = registry.render_spec(
        &pattern_spec(
            "icon",
            [
                ("name", Value::string("help-circle")),
                ("size", Value::string("sm")),
                ("className", Value::string("text-primary mt-0.5 shrink-0")),
            ],
        ),
        None,
        &view,
    );
    let question_text = registry.render_spec(
        &pattern_spec(
            "typography",
            [
                ("variant", Value::string("body")),
                ("className", Value::string("font-medium")),
                ("content", Value::string(question)),
            ],
        ),
        None,
        &view,
    );
    let answer_text = registry.render_spec(
        &pattern_spec(
            "typography",
            [
                ("variant", Value::string("body")),
                ("className", Value::string("text-foreground")),
                ("content", Value::string(answer)),
            ],
        ),
        None,
        &view,
    );
    let label = registry.render_spec(
        &pattern_spec(
            "typography",
            [
                ("variant", Value::string("caption")),
                (
                    "content",
                    Value::string(if is_revealed {
                        crate::i18n::t("quiz.hideAnswer")
                    } else {
                        crate::i18n::t("quiz.showAnswer")
                    }),
                ),
            ],
        ),
        None,
        &view,
    );
    let chevron = registry.render_spec(
        &pattern_spec(
            "icon",
            [
                (
                    "name",
                    Value::string(if is_revealed {
                        "chevron-up"
                    } else {
                        "chevron-down"
                    }),
                ),
                ("className", Value::string("w-3.5 h-3.5")),
            ],
        ),
        None,
        &view,
    );
    rsx! {
        div { class: "{card}",
            div { class: "{box_p4}",
                div { class: "{box_fg}",
                    div { class: "flex flex-col gap-2 items-stretch justify-start p-4", style: "height:auto",
                        div { class: "flex flex-row gap-2 items-start justify-start", style: "height:auto",
                            {help}
                            {question_text}
                        }
                        if is_revealed {
                            div { class: "{box_answer}", {answer_text} }
                        }
                        button {
                            class: "{toggle}",
                            r#type: "button",
                            onclick: move |_| {
                                let mut revealed = revealed;
                                revealed.set(!revealed());
                            },
                            span { class: "inline-flex flex-1 min-w-0 items-center [justify-content:inherit] gap-2",
                                div { class: "flex flex-row gap-1 items-center justify-start", style: "height:auto",
                                    {label}
                                    {chevron}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for QuizBlockRenderer {
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
        rsx! { Quiz { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
