//! Native Dioxus renderer for the `agent-activity-row` pattern (web: AgentActivityFeed.tsx `AgentActivityRow`).
//! Text is the web's default-locale (en) catalog; the host has no translation catalog.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{object_value, pattern_spec, plain_text, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::collections::{HashMap, HashSet};

pub struct AgentActivityRowRenderer;

#[derive(Clone, Copy, Default)]
pub(crate) struct RowFacts {
    pub settled: bool,
    pub ready_count: usize,
}

pub(crate) fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_object().and_then(|fields| fields.get(key))
}

pub(crate) fn plain_text_of(value: Option<&Value>) -> String {
    value.map(plain_text).unwrap_or_default()
}

pub(crate) fn kind_of(activity: &Value) -> String {
    get_string_or(activity, "type", "")
}

pub(crate) fn is_user_message(activity: &Value) -> bool {
    kind_of(activity) == "message" && get_string_or(activity, "role", "") == "user"
}

pub(crate) fn render_pattern(
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
    name: &str,
    fields: Vec<(&str, Value)>,
) -> Element {
    registry.render_spec(&pattern_spec(name, fields), None, view)
}

pub(crate) fn typography_spec(
    variant: &str,
    text: &str,
    color: Option<&str>,
    weight: Option<&str>,
    class: Option<&str>,
) -> Value {
    let mut fields = vec![
        ("variant", Value::string(variant)),
        ("content", Value::string(text)),
    ];
    if let Some(color) = color {
        fields.push(("color", Value::string(color)));
    }
    if let Some(weight) = weight {
        fields.push(("weight", Value::string(weight)));
    }
    if let Some(class) = class {
        fields.push(("className", Value::string(class)));
    }
    pattern_spec("typography", fields)
}

pub(crate) fn typography(
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
    variant: &str,
    text: &str,
    color: Option<&str>,
    weight: Option<&str>,
    class: Option<&str>,
) -> Element {
    registry.render_spec(
        &typography_spec(variant, text, color, weight, class),
        None,
        view,
    )
}

/// An `agent-activity-row` spec for `activity` with the list facts it was derived with.
pub(crate) fn row_spec(activity: &Value, facts: RowFacts) -> Value {
    pattern_spec(
        "agent-activity-row",
        [
            ("activity", activity.clone()),
            (
                "facts",
                object_value([
                    ("settled", Value::Bool(facts.settled)),
                    ("readyCount", Value::Number(facts.ready_count as f64)),
                ]),
            ),
        ],
    )
}

pub(crate) fn activity_row_facts(items: &[Value]) -> Vec<RowFacts> {
    let mut facts = vec![RowFacts::default(); items.len()];
    let mut open: HashMap<String, Vec<usize>> = HashMap::new();
    let mut ready: HashSet<String> = HashSet::new();
    for (index, activity) in items.iter().enumerate() {
        let name = get_string_or(activity, "orbitalName", "");
        match kind_of(activity).as_str() {
            "orbital_started" => open.entry(name).or_default().push(index),
            "orbital_done" => {
                let starts = open.entry(name.clone()).or_default();
                if let Some(last) = starts.pop() {
                    facts[last].settled = true;
                }
                ready.insert(name);
            }
            "done" => {
                facts[index].ready_count = ready.len();
                ready.clear();
            }
            _ => {}
        }
    }
    facts
}

struct RowSpec {
    icon: String,
    color: &'static str,
    animation: Option<&'static str>,
    text: String,
    details: Vec<String>,
    raw: Option<String>,
}

fn spec(icon: &str, color: &'static str, text: String, details: Vec<String>) -> RowSpec {
    RowSpec {
        icon: icon.to_string(),
        color,
        animation: None,
        text,
        details,
        raw: None,
    }
}

fn change_kind(kind: &str, orbital: &str) -> Option<(&'static str, String)> {
    Some(match kind {
        "orbital-added" => ("circle-plus", format!("Added {orbital}")),
        "orbital-removed" => ("circle-minus", format!("Removed {orbital}")),
        "entity-fields-changed" => ("database", format!("Changed the data fields of {orbital}")),
        "trait-added" => ("plus-square", format!("Added a behavior to {orbital}")),
        "trait-removed" => ("minus-square", format!("Removed a behavior from {orbital}")),
        "trait-config-changed" => (
            "sliders-horizontal",
            format!("Changed the settings of {orbital}"),
        ),
        "state-machine-changed" => ("git-branch", format!("Changed the flow of {orbital}")),
        "guard-changed" => ("shield", format!("Changed a rule in {orbital}")),
        "effect-changed" => ("zap", format!("Changed what {orbital} does")),
        "render-ui-changed" => (
            "layout-panel-top",
            format!("Changed the design of {orbital}"),
        ),
        "event-wiring-changed" => ("cable", format!("Changed the connections of {orbital}")),
        "page-changed" => ("file", format!("Changed the pages of {orbital}")),
        "theme-changed" => ("palette", format!("Changed the theme of {orbital}")),
        "behavior-composed" => ("layers", format!("Composed a behavior into {orbital}")),
        _ => return None,
    })
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().map(plain_text).collect())
        .unwrap_or_default()
}

fn describe(activity: &Value, facts: RowFacts) -> Option<RowSpec> {
    let text = |key: &str| get_string_or(activity, key, "");
    let opt = |key: &str| get_string(activity, key);
    Some(match kind_of(activity).as_str() {
        "message" => spec("info", "muted", text("content"), vec![]),
        "tool_call" => {
            let args = text("argsText");
            let mut row = spec(
                "wrench",
                "muted",
                format!("Called {}", text("tool")),
                vec![],
            );
            if args != "" && args != "{}" {
                row.raw = Some(args);
            }
            row
        }
        "tool_result" => {
            let success = get_bool_or(activity, "success", false);
            let mut details = Vec::new();
            if let Some(detail) = opt("detail") {
                details.push(detail);
            }
            if let Some(rows) = field(activity, "rows") {
                details.push(format!("{} rows", plain_text(rows)));
            }
            let mut row = spec(
                if success {
                    "corner-down-right"
                } else {
                    "alert-circle"
                },
                if success { "muted" } else { "error" },
                format!("{} returned", text("tool")),
                details,
            );
            row.raw = Some(text("resultText"));
            row
        }
        "file_operation" => spec("file-text", "muted", text("operation"), vec![text("path")]),
        "schema_diff" => spec(
            "file-diff",
            "muted",
            format!("Edited {}", text("filePath")),
            vec![],
        ),
        "error" => spec(
            "alert-circle",
            "error",
            text("message"),
            opt("code")
                .filter(|code| !code.is_empty())
                .into_iter()
                .collect(),
        ),
        "coordinator_decision" => spec(
            "compass",
            "muted",
            format!("Chose {}", text("organism")),
            vec![text("reason")],
        ),
        "plan_committed" => {
            let orbitals = strings(field(activity, "orbitals"));
            spec(
                "layers",
                "muted",
                format!("Planning {} orbitals", orbitals.len()),
                vec![orbitals.join(", ")],
            )
        }
        "pending_question" => spec("help-circle", "warning", text("question"), vec![]),
        "clarification_question" => {
            let candidates = field(activity, "candidates")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .map(|candidate| {
                            let label = get_string_or(candidate, "label", "");
                            match get_string(candidate, "whyThisFits").filter(|why| !why.is_empty())
                            {
                                Some(why) => format!("{label} — {why}"),
                                None => label,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            spec("help-circle", "warning", text("question"), candidates)
        }
        "analysis" => {
            let mut details = Vec::new();
            if let Some(reason) = opt("organismReason").filter(|r| !r.is_empty()) {
                details.push(reason);
            }
            if let Some(reasoning) = field(activity, "complexity")
                .and_then(|complexity| get_string(complexity, "reasoning"))
                .filter(|r| !r.is_empty())
            {
                details.push(reasoning);
            }
            if let Some(renames) = field(activity, "renames").and_then(Value::as_array) {
                for rename in renames {
                    details.push(format!(
                        "Rename {} → {}",
                        get_string_or(rename, "from", ""),
                        get_string_or(rename, "to", "")
                    ));
                }
            }
            for name in strings(field(activity, "deletes")) {
                details.push(format!("Remove {name}"));
            }
            spec(
                "check",
                "success",
                format!("Analyzed your request — {}", text("organism")),
                details,
            )
        }
        "orbital_started" => {
            let name = text("orbitalName");
            if facts.settled {
                spec("layers", "muted", format!("Started {name}"), vec![])
            } else {
                let mut row = spec("loader", "primary", format!("Working on {name}…"), vec![]);
                row.animation = Some("spin");
                row
            }
        }
        "orbital_done" => {
            let traits = field(activity, "traitCount");
            let transitions = field(activity, "transitionCount");
            let details = if traits.is_some() || transitions.is_some() {
                vec![format!(
                    "{} traits, {} transitions",
                    traits.map(plain_text).unwrap_or_else(|| "0".to_string()),
                    transitions
                        .map(plain_text)
                        .unwrap_or_else(|| "0".to_string())
                )]
            } else {
                vec![]
            };
            spec(
                "check",
                "success",
                format!("{} ready", text("orbitalName")),
                details,
            )
        }
        "schema_change" => {
            let (icon, label) = change_kind(&text("changeKind"), &text("orbitalName"))?;
            let details = ["traitName", "transitionEvent"]
                .iter()
                .filter_map(|key| opt(key))
                .collect();
            spec(icon, "primary", label, details)
        }
        "done" => spec(
            "check-circle",
            "success",
            format!("Done — {} orbitals ready", facts.ready_count),
            vec![],
        ),
        "cancelled" => {
            let message = text("message");
            spec(
                "circle-x",
                "muted",
                if message.is_empty() {
                    "Cancelled".to_string()
                } else {
                    message
                },
                vec![],
            )
        }
        _ => return None,
    })
}

pub(crate) struct AccordionEntry {
    pub id: String,
    pub header: Value,
    pub content: Value,
}

pub(crate) fn accordion(
    items: Vec<AccordionEntry>,
    multiple: bool,
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
) -> Element {
    let items: Vec<Value> = items
        .into_iter()
        .map(|entry| {
            object_value([
                ("id", Value::string(entry.id.as_str())),
                ("header", entry.header),
                ("content", entry.content),
            ])
        })
        .collect();
    render_pattern(
        registry,
        view,
        "accordion",
        vec![
            ("items", Value::Array(items.into())),
            ("multiple", Value::Bool(multiple)),
            ("surface", Value::string("none")),
        ],
    )
}

fn llm_summary(activity: &Value) -> String {
    let Some(llm) = field(activity, "llm").filter(|llm| llm.as_object().is_some()) else {
        return "Raw LLM response".to_string();
    };
    let mut parts = vec![
        format!(
            "{} · {}",
            get_string_or(llm, "service", ""),
            get_string_or(llm, "model", "")
        ),
        format!("{:.1}s", get_number_or(llm, "durationMs", 0.0) / 1000.0),
    ];
    let prompt = get_number(llm, "promptTokens");
    let completion = get_number(llm, "completionTokens");
    if prompt.is_some() || completion.is_some() {
        parts.push(format!(
            "{} in / {} out tokens",
            plain_text(&Value::Number(prompt.unwrap_or(0.0))),
            plain_text(&Value::Number(completion.unwrap_or(0.0)))
        ));
    }
    if let Some(cost) = get_number(llm, "costUSD") {
        parts.push(format!("${cost:.4}"));
    }
    parts.join(" · ")
}

fn llm_section(label: &str, text: &str) -> Value {
    pattern_spec(
        "vstack",
        [
            ("gap", Value::string("none")),
            ("className", Value::string("w-full")),
            (
                "children",
                Value::Array(
                    vec![
                        typography_spec("caption", label, Some("muted"), Some("semibold"), None),
                        typography_spec(
                            "caption",
                            text,
                            None,
                            None,
                            Some("whitespace-pre-wrap break-all font-mono"),
                        ),
                    ]
                    .into(),
                ),
            ),
        ],
    )
}

fn llm_row(activity: &Value, registry: &DioxusPatternRegistry, view: &DioxusView<'_>) -> Element {
    let header = pattern_spec(
        "box",
        [
            (
                "className",
                Value::string("flex items-center gap-2 min-w-0"),
            ),
            (
                "children",
                Value::Array(
                    vec![
                        pattern_spec(
                            "icon",
                            [
                                ("name", Value::string("cpu")),
                                ("size", Value::string("sm")),
                                ("color", Value::string("muted")),
                                ("className", Value::string("shrink-0")),
                            ],
                        ),
                        typography_spec(
                            "caption",
                            &llm_summary(activity),
                            Some("muted"),
                            None,
                            Some("truncate"),
                        ),
                    ]
                    .into(),
                ),
            ),
        ],
    );
    let llm = field(activity, "llm").filter(|llm| llm.as_object().is_some());
    let mut sections = vec![llm_section(
        "Response",
        &get_string_or(activity, "content", ""),
    )];
    if let Some(llm) = llm {
        sections.push(llm_section(
            "System prompt",
            &get_string_or(llm, "systemPrompt", ""),
        ));
        sections.push(llm_section(
            "User prompt",
            &get_string_or(llm, "userPrompt", ""),
        ));
        if let Some(calls) = get_string(llm, "toolCallsJson").filter(|calls| !calls.is_empty()) {
            sections.push(llm_section("Tool calls", &calls));
        }
    }
    let content = pattern_spec(
        "vstack",
        [
            ("gap", Value::string("sm")),
            ("className", Value::string("w-full")),
            ("children", Value::Array(sections.into())),
        ],
    );
    let id = format!(
        "llm-{}",
        plain_text(field(activity, "timestamp").unwrap_or(&Value::Null))
    );
    rsx! {
        div { class: "w-full", "data-testid": "agent-activity-llm_response",
            {accordion(vec![AccordionEntry { id, header, content }], false, registry, view)}
        }
    }
}

pub(crate) fn activity_row(
    activity: &Value,
    facts: RowFacts,
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
) -> Element {
    let kind = kind_of(activity);
    if kind == "llm_response" {
        return llm_row(activity, registry, view);
    }
    if kind == "message" && get_string_or(activity, "role", "") != "system" {
        let mine = is_user_message(activity);
        let content = get_string_or(activity, "content", "");
        let class = if mine {
            "max-w-[90%] rounded-[var(--radius-lg)] px-3 py-2 self-end bg-[var(--color-primary)] text-[var(--color-primary-foreground)]"
        } else {
            "max-w-[90%] rounded-[var(--radius-lg)] px-3 py-2 self-start bg-[var(--color-surface)]"
        };
        let testid = if mine {
            "agent-activity-user-message"
        } else {
            "agent-activity-assistant-message"
        };
        let body = if mine {
            typography(
                registry,
                view,
                "body2",
                &content,
                Some("inherit"),
                None,
                Some("whitespace-pre-wrap break-words"),
            )
        } else {
            render_pattern(
                registry,
                view,
                "markdown-content",
                vec![
                    ("content", Value::string(content)),
                    ("className", Value::string("prose-sm break-words")),
                ],
            )
        };
        return rsx! { div { class: "{class}", "data-testid": "{testid}", {body} } };
    }
    let Some(row) = describe(activity, facts) else {
        return rsx! {};
    };
    let mut icon_fields = vec![
        ("name", Value::string(row.icon.as_str())),
        ("size", Value::string("sm")),
        ("color", Value::string(row.color)),
        ("className", Value::string("mt-0.5 shrink-0")),
    ];
    if let Some(animation) = row.animation {
        icon_fields.push(("animation", Value::string(animation)));
    }
    let icon = render_pattern(registry, view, "icon", icon_fields);
    let head = match &row.raw {
        None => typography(
            registry,
            view,
            "body2",
            &row.text,
            None,
            None,
            Some("break-words"),
        ),
        Some(raw) => {
            let id = format!(
                "{kind}-{}",
                plain_text(field(activity, "timestamp").unwrap_or(&Value::Null))
            );
            accordion(
                vec![AccordionEntry {
                    id,
                    header: typography_spec("body2", &row.text, None, None, Some("break-words")),
                    content: typography_spec(
                        "caption",
                        raw,
                        None,
                        None,
                        Some("whitespace-pre-wrap break-all font-mono"),
                    ),
                }],
                false,
                registry,
                view,
            )
        }
    };
    let testid = format!("agent-activity-{kind}");
    rsx! {
        div { class: "flex items-start gap-2 w-full", "data-testid": "{testid}",
            {icon}
            div { class: "flex flex-col gap-0 items-stretch justify-start flex-1 min-w-0",
                {head}
                for detail in row.details.iter() {
                    {typography(registry, view, "caption", detail, Some("muted"), None, Some("break-all whitespace-pre-wrap"))}
                }
            }
        }
    }
}

pub(crate) fn thinking_row(registry: &DioxusPatternRegistry, view: &DioxusView<'_>) -> Element {
    rsx! {
        div { class: "flex items-center gap-2 w-full", "data-testid": "agent-activity-thinking", role: "status", "aria-live": "polite",
            {render_pattern(registry, view, "icon", vec![
                ("name", Value::string("loader")),
                ("size", Value::string("sm")),
                ("color", Value::string("primary")),
                ("animation", Value::string("spin")),
                ("className", Value::string("shrink-0")),
            ])}
            {typography(registry, view, "body2", "Thinking…", Some("muted"), None, None)}
        }
    }
}

impl AgentActivityRowRenderer {
    const NAME: &str = "agent-activity-row";
}

impl DioxusPatternRenderer for AgentActivityRowRenderer {
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
        let Some(activity) = field(props, "activity") else {
            return rsx! {};
        };
        let facts = field(props, "facts")
            .map(|facts| RowFacts {
                settled: get_bool_or(facts, "settled", false),
                ready_count: get_number_or(facts, "readyCount", 0.0) as usize,
            })
            .unwrap_or_default();
        activity_row(activity, facts, registry, view)
    }
}
