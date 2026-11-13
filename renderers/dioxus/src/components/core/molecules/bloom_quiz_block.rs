//! Native body of the `bloom-quiz-block` pattern, mirroring the web `BloomQuizBlock` molecule.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    cn, merge_class_name, object_value, pattern_spec, DioxusPatternRenderer, DioxusView,
    OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `bloom-quiz-block` pattern.
pub struct BloomQuizBlockRenderer;

impl BloomQuizBlockRenderer {
    const NAME: &str = "bloom-quiz-block";
}

#[derive(Props, Clone, PartialEq)]
struct BloomProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

enum Segment {
    Markdown(String),
    Code { language: String, content: String },
}

/// The web's `parseMarkdownWithCodeBlocks`: fenced blocks are ```<info line>\r?\n<code>```.
fn parse_segments(content: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut last = 0;
    let mut search = 0;
    while let Some(offset) = content[search..].find("```") {
        let start = search + offset;
        let info_start = start + 3;
        let Some(line_len) = content[info_start..].find(['\n', '\r']) else {
            break;
        };
        let info_end = info_start + line_len;
        let code_start = if content[info_end..].starts_with("\r\n") {
            info_end + 2
        } else if content[info_end..].starts_with('\n') {
            info_end + 1
        } else {
            search = start + 1;
            continue;
        };
        let Some(close) = content[code_start..].find("```") else {
            break;
        };
        let before = &content[last..start];
        if !before.trim().is_empty() {
            segments.push(Segment::Markdown(before.to_string()));
        }
        let mut tokens = content[info_start..info_end].split_whitespace();
        let first = tokens.next().unwrap_or("text");
        let language = match first.strip_suffix("-runnable") {
            Some("") => "text",
            Some(base) => base,
            None => first,
        };
        segments.push(Segment::Code {
            language: language.to_string(),
            content: content[code_start..code_start + close].trim().to_string(),
        });
        last = code_start + close + 3;
        search = last;
    }
    let remaining = &content[last..];
    if !remaining.trim().is_empty() {
        segments.push(Segment::Markdown(remaining.to_string()));
    }
    segments
}

fn segment_elements(
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
    content: &str,
) -> Vec<Element> {
    parse_segments(content)
        .into_iter()
        .map(|segment| {
            let spec = match segment {
                Segment::Markdown(content) => {
                    pattern_spec("markdown-content", [("content", Value::string(content))])
                }
                Segment::Code { language, content } => pattern_spec(
                    "code-block",
                    [
                        ("code", Value::string(content)),
                        ("language", Value::string(language)),
                    ],
                ),
            };
            registry.render_spec(&spec, None, view)
        })
        .collect()
}

fn level_config(level: &str) -> Option<(&'static str, &'static str, &'static str)> {
    match level {
        "remember" => Some((
            "bg-secondary text-secondary-foreground",
            "bg-muted",
            "Remember",
        )),
        "understand" => Some(("bg-info text-info-foreground", "bg-info/10", "Understand")),
        "apply" => Some((
            "bg-success text-success-foreground",
            "bg-success/10",
            "Apply",
        )),
        "analyze" => Some((
            "bg-warning text-warning-foreground",
            "bg-warning/10",
            "Analyze",
        )),
        "evaluate" => Some((
            "bg-accent text-accent-foreground",
            "bg-accent/10",
            "Evaluate",
        )),
        "create" => Some((
            "bg-primary text-primary-foreground",
            "bg-primary/10",
            "Create",
        )),
        _ => None,
    }
}

#[allow(non_snake_case)]
fn Bloom(input: BloomProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let revealed = use_signal(|| false);
    let level = get_string_or(props, "level", "");
    let Some((color, bg, label)) = level_config(&level) else {
        tracing::error!(%level, "bloom-quiz-block level is not a Bloom level");
        return rsx! {};
    };
    let question = get_string_or(props, "question", "");
    let answer = get_string_or(props, "answer", "");
    let event = get_string_or(props, "answerEvent", "ANSWER_BLOOM");
    let index = get_number(props, "index");
    let index_payload = props.get("index").cloned().unwrap_or_else(|| Value::Int(0));
    let answered = get_bool_or(props, "isAnswered", false);
    let is_revealed = revealed();
    let class = merge_class_name(
        &format!("rounded-container border border-primary p-4 my-4 transition-all {bg}"),
        props,
    );
    let badge_class = cn(&[color, "text-xs px-2 py-1 rounded-full font-medium"]);
    let question_label = index.map(|index| {
        let number = index + 1.0;
        if number.fract() == 0.0 {
            format!("Question {}", number as i64)
        } else {
            format!("Question {number}")
        }
    });
    let check = answered.then(|| {
        registry.render_spec(
            &pattern_spec(
                "icon",
                [
                    ("name", Value::string("check-circle")),
                    ("size", Value::string("sm")),
                    (
                        "className",
                        Value::string("w-5 h-5 text-success flex-shrink-0"),
                    ),
                ],
            ),
            None,
            &view,
        )
    });
    let question_parts = segment_elements(registry, &view, &question);
    let answer_parts = segment_elements(registry, &view, &answer);
    let sink = view.sink;
    rsx! {
        div { class: "{class}", ..shared_root_attrs(props),
            div { class: "flex items-center justify-between mb-3",
                div { class: "flex items-center gap-2 flex-wrap",
                    if let Some(question_label) = question_label {
                        span { class: "text-muted-foreground font-medium text-sm", "{question_label}" }
                    }
                    span { class: "{badge_class}", "{label}" }
                }
                if let Some(check) = check { {check} }
            }
            div { class: "font-semibold text-primary mb-3 space-y-2",
                for part in question_parts { {part} }
            }
            button {
                r#type: "button",
                class: "inline-flex items-center rounded-interactive bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary-hover transition-colors",
                onclick: move |_| {
                    let mut revealed = revealed;
                    if !revealed() {
                        if let Some(sink) = sink {
                            sink.call((
                                event.clone(),
                                object_value([
                                    ("index", index_payload.clone()),
                                    ("level", Value::string(level.clone())),
                                ]),
                            ));
                        }
                    }
                    revealed.set(!revealed());
                },
                {if is_revealed { crate::i18n::t("bloomQuiz.hideAnswer") } else { crate::i18n::t("bloomQuiz.revealAnswer") }}
            }
            if is_revealed {
                div { class: "rounded-container bg-card/80 p-3 text-sm text-foreground shadow-elevation-card surface-material border border-primary mt-3 space-y-2",
                    div { class: "text-xs text-muted-foreground mb-1 font-medium uppercase tracking-wide", {crate::i18n::t("bloomQuiz.answer")} }
                    for part in answer_parts { {part} }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for BloomQuizBlockRenderer {
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
        rsx! { Bloom { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
