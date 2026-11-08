//! Dioxus renderer for the `segment-renderer` pattern: each lesson segment composed over its block pattern.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, layout_box_spec, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `segment-renderer` pattern.
pub struct SegmentRendererRenderer;

impl SegmentRendererRenderer {
    const NAME: &str = "segment-renderer";
}

fn put(fields: &mut Vec<(&'static str, Value)>, key: &'static str, value: Option<Value>) {
    if let Some(value) = value.filter(|value| !value.is_null()) {
        fields.push((key, value));
    }
}

fn string_field(segment: &Value, key: &str) -> Value {
    Value::string(get_string_or(segment, key, ""))
}

impl DioxusPatternRenderer for SegmentRendererRenderer {
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
        let segments = get_array(props, "segments");
        if segments.is_empty() {
            return rsx! {};
        }
        let progress = props.get("userProgress").cloned().unwrap_or(Value::Null);
        let event = |key: &str| props.get(key).filter(|value| !value.is_null()).cloned();

        let annotations = get_array(props, "annotations");
        let mut owned: Vec<Vec<Value>> = vec![Vec::new(); segments.len()];
        for annotation in &annotations {
            let needle = get_string_or(annotation, "text", "");
            let owner = segments.iter().position(|segment| {
                get_string(segment, "type").as_deref() == Some("markdown")
                    && get_string_or(segment, "content", "").contains(needle.as_str())
            });
            if let Some(owner) = owner {
                owned[owner].push(annotation.clone());
            }
        }

        let mut reflect_index = 0usize;
        let mut bloom_index = 0usize;
        let mut specs: Vec<Value> = Vec::new();
        for (index, segment) in segments.iter().enumerate() {
            let kind = get_string_or(segment, "type", "");
            match kind.as_str() {
                "markdown" => {
                    let mut fields = vec![("content", string_field(segment, "content"))];
                    if !owned[index].is_empty() {
                        fields.push(("annotations", Value::Array(owned[index].clone().into())));
                    }
                    put(&mut fields, "annotationEvent", event("annotationEvent"));
                    specs.push(pattern_spec("markdown-content", fields));
                }
                "code" => {
                    let language = get_string(segment, "language");
                    if language.as_deref() == Some("mermaid") {
                        specs.push(layout_box_spec(
                            "not-prose my-4",
                            None,
                            Vec::new(),
                            vec![layout_box_spec(
                                "overflow-x-auto",
                                None,
                                vec![("data-testid".to_string(), Value::string("mermaid-diagram"))],
                                Vec::new(),
                            )],
                        ));
                        continue;
                    }
                    let run_event = event("runCodeEvent");
                    if let (true, Some(run_event)) =
                        (get_bool_or(segment, "runnable", false), run_event)
                    {
                        let run_id = index.to_string();
                        let mut fields = vec![
                            ("language", string_field(segment, "language")),
                            ("code", string_field(segment, "content")),
                            ("runnable", Value::Bool(true)),
                            ("runEvent", run_event),
                            ("runId", Value::string(run_id.clone())),
                            (
                                "running",
                                Value::Bool(
                                    get_string(props, "runningCode").as_deref()
                                        == Some(run_id.as_str()),
                                ),
                            ),
                        ];
                        put(&mut fields, "resetEvent", event("resetCodeEvent"));
                        put(
                            &mut fields,
                            "output",
                            props
                                .get("codeOutputs")
                                .and_then(|outputs| outputs.get(&run_id))
                                .cloned(),
                        );
                        specs.push(pattern_spec("code-runner-panel", fields));
                    } else {
                        specs.push(pattern_spec(
                            "code-block",
                            [
                                (
                                    "language",
                                    Value::string(language.unwrap_or_else(|| "text".to_string())),
                                ),
                                ("code", string_field(segment, "content")),
                            ],
                        ));
                    }
                }
                "quiz" => specs.push(pattern_spec(
                    "quiz-block",
                    [
                        ("question", string_field(segment, "question")),
                        ("answer", string_field(segment, "answer")),
                    ],
                )),
                "activate" => {
                    let mut fields = vec![("question", string_field(segment, "question"))];
                    put(
                        &mut fields,
                        "savedResponse",
                        progress.get("activationResponse").cloned(),
                    );
                    put(&mut fields, "saveEvent", event("activationSaveEvent"));
                    specs.push(pattern_spec("activation-block", fields));
                }
                "connect" => specs.push(pattern_spec(
                    "connection-block",
                    [("content", string_field(segment, "content"))],
                )),
                "reflect" => {
                    let at = reflect_index;
                    reflect_index += 1;
                    let mut fields = vec![
                        ("prompt", string_field(segment, "prompt")),
                        ("index", Value::Int(at as i64)),
                    ];
                    put(
                        &mut fields,
                        "savedNote",
                        progress
                            .get("reflectionNotes")
                            .and_then(Value::as_array)
                            .and_then(|notes| notes.get(at))
                            .cloned(),
                    );
                    put(&mut fields, "saveEvent", event("reflectionSaveEvent"));
                    specs.push(pattern_spec("reflection-block", fields));
                }
                "bloom" => {
                    let at = bloom_index;
                    bloom_index += 1;
                    let mut fields = vec![
                        ("level", string_field(segment, "level")),
                        ("question", string_field(segment, "question")),
                        ("answer", string_field(segment, "answer")),
                        ("index", Value::Int(at as i64)),
                    ];
                    put(
                        &mut fields,
                        "isAnswered",
                        progress
                            .get("bloomAnswered")
                            .and_then(|answered| answered.get(&at.to_string()))
                            .cloned(),
                    );
                    put(&mut fields, "answerEvent", event("bloomAnswerEvent"));
                    specs.push(pattern_spec("bloom-quiz-block", fields));
                }
                _ => {}
            }
        }

        let class = cn(&[
            "border border-border rounded-container p-2 md:p-4 overflow-x-auto space-y-6",
            get_string_or(props, "containerClassName", "").as_str(),
            get_string_or(props, "className", "").as_str(),
        ]);
        let annotator = event("askEvent").is_some() || event("noteEvent").is_some();
        let inner = rsx! {
            div { class: "{class}", ..shared_root_attrs(props),
                for spec in specs.iter() { {registry.render_spec(spec, None, view)} }
            }
        };
        if annotator {
            rsx! { div { class: "bg-transparent rounded-none shadow-none relative", {inner} } }
        } else {
            inner
        }
    }
}
