//! Native body of the `content-renderer` pattern, mirroring the web `ContentRenderer` molecule
//! and `lib/parseContentSegments.ts`.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{array_value, object_value, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `content-renderer` pattern.
pub struct ContentRendererRenderer;

impl ContentRendererRenderer {
    const NAME: &str = "content-renderer";
}

enum Segment {
    Markdown(String),
    Code {
        language: String,
        content: String,
    },
    Orbital {
        language: String,
        content: String,
        schema: Value,
    },
    Quiz {
        question: String,
        answer: String,
    },
}

fn orbital_schema(code: &str) -> Option<Value> {
    let parsed: Value = serde_json::from_str(code).ok()?;
    let fields = parsed.as_object()?;
    let has_pair = fields.get("states").is_some() && fields.get("transitions").is_some();
    let has_orbitals = fields
        .get("orbitals")
        .is_some_and(|value| value.as_array().is_some());
    (has_pair || has_orbitals).then_some(parsed)
}

fn is_word(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn parse_markdown_with_code_blocks(content: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut last = 0;
    let mut search = 0;
    while let Some(offset) = content[search..].find("```") {
        let start = search + offset;
        let after_fence = start + 3;
        let word_len = content[after_fence..]
            .chars()
            .take_while(|c| is_word(*c))
            .map(char::len_utf8)
            .sum::<usize>();
        let body_start = after_fence + word_len;
        if !content[body_start..].starts_with('\n') {
            search = start + 1;
            continue;
        }
        let code_start = body_start + 1;
        let Some(close) = content[code_start..].find("```") else {
            search = start + 1;
            continue;
        };
        let end = code_start + close + 3;
        let before = &content[last..start];
        if !before.trim().is_empty() {
            segments.push(Segment::Markdown(before.to_string()));
        }
        let language = if word_len == 0 {
            "text".to_string()
        } else {
            content[after_fence..body_start].to_string()
        };
        let code = content[code_start..code_start + close].trim().to_string();
        if language == "json" || language == "orb" {
            if let Some(schema) = orbital_schema(&code) {
                segments.push(Segment::Orbital {
                    language,
                    content: code,
                    schema,
                });
                last = end;
                search = end;
                continue;
            }
        }
        segments.push(Segment::Code {
            language,
            content: code,
        });
        last = end;
        search = end;
    }
    let remaining = &content[last..];
    if !remaining.trim().is_empty() {
        segments.push(Segment::Markdown(remaining.to_string()));
    }
    segments
}

fn find_quiz(lower: &str, from: usize) -> Option<(usize, usize, usize, usize, usize)> {
    let mut search = from;
    while let Some(offset) = lower[search..].find("<question>") {
        let start = search + offset;
        let q_start = start + "<question>".len();
        let attempt = (|| {
            let q_len = lower[q_start..].find("</question>")?;
            let q_end = q_start + q_len;
            let mut cursor = q_end + "</question>".len();
            cursor += lower[cursor..].len() - lower[cursor..].trim_start().len();
            if !lower[cursor..].starts_with("<answer>") {
                return None;
            }
            let a_start = cursor + "<answer>".len();
            let a_len = lower[a_start..].find("</answer>")?;
            let a_end = a_start + a_len;
            Some((start, q_start, q_end, a_start, a_end))
        })();
        if attempt.is_some() {
            return attempt;
        }
        search = start + 1;
    }
    None
}

fn parse_content_segments(content: &str) -> Vec<Segment> {
    let lower = content.to_ascii_lowercase();
    let mut segments = Vec::new();
    let mut last = 0;
    let mut from = 0;
    while let Some((start, q_start, q_end, a_start, a_end)) = find_quiz(&lower, from) {
        let before = &content[last..start];
        if !before.trim().is_empty() {
            segments.extend(parse_markdown_with_code_blocks(before));
        }
        segments.push(Segment::Quiz {
            question: content[q_start..q_end].trim().to_string(),
            answer: content[a_start..a_end].trim().to_string(),
        });
        last = a_end + "</answer>".len();
        from = last;
    }
    let remaining = &content[last..];
    if !remaining.trim().is_empty() {
        segments.extend(parse_markdown_with_code_blocks(remaining));
    }
    segments
}

fn from_segment_value(value: &Value) -> Option<Segment> {
    match get_string(value, "type")?.as_str() {
        "markdown" => Some(Segment::Markdown(get_string_or(value, "content", ""))),
        "code" => Some(Segment::Code {
            language: get_string_or(value, "language", ""),
            content: get_string_or(value, "content", ""),
        }),
        "orbital" => Some(Segment::Orbital {
            language: get_string_or(value, "language", ""),
            content: get_string_or(value, "content", ""),
            schema: value.get("schema").cloned()?,
        }),
        "quiz" => Some(Segment::Quiz {
            question: get_string_or(value, "question", ""),
            answer: get_string_or(value, "answer", ""),
        }),
        _ => None,
    }
}

fn to_code_language(language: &str) -> String {
    if language.is_empty() {
        "text".to_string()
    } else {
        language.to_lowercase()
    }
}

fn full_schema(parsed: &Value) -> Value {
    let is_full = parsed
        .get("orbitals")
        .is_some_and(|value| value.as_array().is_some());
    if is_full {
        return parsed.clone();
    }
    let inline = || Value::string("inline");
    object_value([
        ("name", inline()),
        (
            "orbitals",
            array_value(vec![object_value([
                ("name", inline()),
                (
                    "entity",
                    object_value([("name", inline()), ("fields", array_value(Vec::new()))]),
                ),
                (
                    "traits",
                    array_value(vec![object_value([
                        ("name", inline()),
                        ("stateMachine", parsed.clone()),
                    ])]),
                ),
                ("pages", array_value(Vec::new())),
            ])]),
        ),
    ])
}

impl DioxusPatternRenderer for ContentRendererRenderer {
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
        let segments: Vec<Segment> = match props.get("segments").and_then(Value::as_array) {
            Some(items) => items.iter().filter_map(from_segment_value).collect(),
            None => parse_content_segments(&get_string_or(props, "content", "")),
        };
        if segments.is_empty() {
            return rsx! {};
        }
        let direction = get_string(props, "direction");
        let with_direction = |mut fields: Vec<(&'static str, Value)>| {
            if let Some(direction) = &direction {
                fields.push(("direction", Value::string(direction.to_string())));
            }
            fields
        };
        let class = format!(
            "flex flex-col gap-4 items-stretch justify-start w-full {}",
            get_string_or(props, "className", "")
        );
        let code = |language: &str, content: &str| {
            pattern_spec(
                "code-block",
                [
                    ("code", Value::string(content)),
                    ("language", Value::string(to_code_language(language))),
                ],
            )
        };
        let elements: Vec<Element> = segments
            .iter()
            .map(|segment| match segment {
                Segment::Markdown(content) => {
                    let spec = pattern_spec(
                        "markdown-content",
                        with_direction(vec![("content", Value::string(content.to_string()))]),
                    );
                    registry.render_spec(&spec, None, view)
                }
                Segment::Code { language, content } => {
                    registry.render_spec(&code(language, content), None, view)
                }
                Segment::Orbital {
                    language,
                    content,
                    schema,
                } => {
                    let block = code(language, content);
                    let machine = pattern_spec(
                        "jazari-state-machine",
                        with_direction(vec![("schema", full_schema(schema))]),
                    );
                    rsx! {
                        div { class: "flex flex-col gap-2 items-stretch justify-start",
                            {registry.render_spec(&block, None, view)}
                            {registry.render_spec(&machine, None, view)}
                        }
                    }
                }
                Segment::Quiz { question, answer } => {
                    let spec = pattern_spec(
                        "quiz-block",
                        [
                            ("question", Value::string(question.to_string())),
                            ("answer", Value::string(answer.to_string())),
                        ],
                    );
                    registry.render_spec(&spec, None, view)
                }
            })
            .collect();
        rsx! {
            div { class: "{class}", ..shared_root_attrs(props),
                for element in elements { {element} }
            }
        }
    }
}
