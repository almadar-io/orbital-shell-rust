use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, dom_passthrough, layout_box_spec, object_value, pattern_spec,
    render_with_local_sink, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

const BORDER_B: &str = "border-b border-[var(--color-border)]";
const SELECT_PREFIX: &str = "__trace_select:";
const CLOSE_MODAL: &str = "__trace_close_modal";

pub struct SubagentTracePanelRenderer;

impl SubagentTracePanelRenderer {
    const NAME: &str = "subagent-trace-panel";
}

#[derive(Props, Clone, PartialEq)]
struct PanelProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn s(text: &str) -> Value {
    Value::string(text)
}

fn tx(variant: &str, color: Option<&str>, class: &str, content: &str) -> Value {
    let mut fields = vec![("variant", s(variant)), ("content", s(content))];
    if let Some(color) = color {
        fields.push(("color", s(color)));
    }
    if !class.is_empty() {
        fields.push(("className", s(class)));
    }
    pattern_spec("typography", fields)
}

fn tx_span(
    variant: &str,
    color: Option<&str>,
    weight: Option<&str>,
    class: &str,
    content: &str,
) -> Value {
    let mut fields = vec![
        ("as", s("span")),
        ("variant", s(variant)),
        ("content", s(content)),
    ];
    if let Some(color) = color {
        fields.push(("color", s(color)));
    }
    if let Some(weight) = weight {
        fields.push(("weight", s(weight)));
    }
    if !class.is_empty() {
        fields.push(("className", s(class)));
    }
    pattern_spec("typography", fields)
}

fn tx_with(variant: &str, color: Option<&str>, class: &str, children: Vec<Value>) -> Value {
    let mut fields = vec![("variant", s(variant)), ("children", array_value(children))];
    if let Some(color) = color {
        fields.push(("color", s(color)));
    }
    if !class.is_empty() {
        fields.push(("className", s(class)));
    }
    pattern_spec("typography", fields)
}

fn stack(pattern: &str, gap: &str, class: &str, children: Vec<Value>) -> Value {
    let mut fields = vec![("gap", s(gap)), ("children", array_value(children))];
    if !class.is_empty() {
        fields.push(("className", s(class)));
    }
    pattern_spec(pattern, fields)
}

fn hs(gap: &str, class: &str, children: Vec<Value>) -> Value {
    stack("hstack", gap, class, children)
}

fn vs(gap: &str, class: &str, children: Vec<Value>) -> Value {
    stack("vstack", gap, class, children)
}

fn bx(class: &str, children: Vec<Value>) -> Value {
    layout_box_spec(class, None, Vec::new(), children)
}

fn bx_style(class: &str, style: Vec<(&str, String)>, children: Vec<Value>) -> Value {
    let style = object_value(
        style
            .into_iter()
            .map(|(key, value)| (key, Value::string(value))),
    );
    layout_box_spec(class, Some(style), Vec::new(), children)
}

fn icon(name: &str, size: &str, class: &str) -> Value {
    let mut fields = vec![("name", s(name)), ("size", s(size))];
    if !class.is_empty() {
        fields.push(("className", s(class)));
    }
    pattern_spec("icon", fields)
}

fn badge(variant: &str, class: &str, text: &str) -> Value {
    let mut fields = vec![("variant", s(variant)), ("children", s(text))];
    if !class.is_empty() {
        fields.push(("className", s(class)));
    }
    pattern_spec("badge", fields)
}

fn ghost_icon_button(action: Option<&str>, name: &str, class: &str) -> Value {
    let mut fields = vec![
        ("variant", s("ghost")),
        ("size", s("sm")),
        ("children", array_value(vec![icon(name, "xs", "")])),
    ];
    if let Some(action) = action {
        fields.push(("action", s(action)));
    }
    if !class.is_empty() {
        fields.push(("className", s(class)));
    }
    pattern_spec("button", fields)
}

fn two(n: i64) -> String {
    format!("{n:02}")
}

fn hhmmss(timestamp: f64) -> String {
    let seconds = (timestamp / 1000.0).floor() as i64;
    let day = seconds.rem_euclid(86_400);
    format!(
        "{}:{}:{}",
        two(day / 3600),
        two(day % 3600 / 60),
        two(day % 60)
    )
}

fn json_text(value: &Value, pretty: bool) -> String {
    let result = if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    };
    result
        .map_err(|error| tracing::error!(%error, "subagent trace json"))
        .unwrap_or_default()
}

fn pretty_json(text: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(text).ok()?;
    Some(json_text(&parsed, true))
}

fn summarize_args(args: Option<&Value>) -> String {
    let keys: Vec<String> = args
        .and_then(Value::as_object)
        .map(|fields| fields.keys().map(|key| key.to_string()).collect())
        .unwrap_or_default();
    if keys.is_empty() {
        return String::new();
    }
    let shown = keys.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    if keys.len() > 3 {
        format!("{shown}, …")
    } else {
        shown
    }
}

fn status_icon(status: &str) -> &'static str {
    match status {
        "running" => "loader",
        "complete" => "check",
        "error" => "x",
        _ => "circle",
    }
}

fn last_message(sub: &Value) -> Option<String> {
    get_array(sub, "messages")
        .last()
        .map(|message| get_string_or(message, "message", ""))
}

fn activity_type_label(kind: &str) -> String {
    match kind {
        "tool_call" => "tool call",
        "tool_result" => "tool result",
        "message" => "message",
        "error" => "error",
        "file_operation" => "file operation",
        "schema_diff" => "schema diff",
        "milestone" => "milestone",
        "analysis" => "analysis",
        "cancelled" => "cancelled",
        "clarification_question" => "clarification question",
        "coordinator_decision" => "coordinator decision",
        "done" => "done",
        "llm_response" => "model response",
        "orbital_done" => "orbital done",
        "orbital_started" => "orbital started",
        "pending_question" => "pending question",
        "plan_committed" => "plan committed",
        "schema_change" => "schema change",
        other => other,
    }
    .to_string()
}

fn file_operation_label(operation: &str) -> String {
    match operation {
        "ls" => "list",
        "read_file" => "read",
        "write_file" => "write",
        "edit_file" => "edit",
        other => other,
    }
    .to_string()
}

fn activity_item(kind: &str, fields: Vec<(&str, Value)>, timestamp: f64) -> Value {
    let mut all = vec![("type", s(kind))];
    all.extend(fields);
    all.push(("timestamp", Value::Number(timestamp)));
    object_value(all)
}

fn coordinator_to_items(activities: &[Value]) -> Vec<Value> {
    activities
        .iter()
        .filter_map(|a| {
            let kind = get_string_or(a, "type", "");
            let ts = get_number_or(a, "timestamp", 0.0);
            let text = |key: &str| Value::string(get_string_or(a, key, ""));
            match kind.as_str() {
                "tool_call" => Some(activity_item(
                    "tool_call",
                    vec![("tool", text("tool")), ("argsText", text("argsText"))],
                    ts,
                )),
                "tool_result" => {
                    let mut fields =
                        vec![("tool", text("tool")), ("resultText", text("resultText"))];
                    if let Some(success) = a.get("success") {
                        fields.push(("success", success.clone()));
                    }
                    Some(activity_item("tool_result", fields, ts))
                }
                "message" => Some(activity_item(
                    "message",
                    vec![("role", text("role")), ("content", text("content"))],
                    ts,
                )),
                "error" => Some(activity_item(
                    "message",
                    vec![
                        ("role", s("system")),
                        (
                            "content",
                            Value::string(format!("Error: {}", get_string_or(a, "message", ""))),
                        ),
                    ],
                    ts,
                )),
                "llm_response" => Some(activity_item(
                    "message",
                    vec![("role", s("assistant")), ("content", text("content"))],
                    ts,
                )),
                "file_operation" => {
                    let mut fields = vec![("operation", text("operation")), ("path", text("path"))];
                    if let Some(success) = a.get("success") {
                        fields.push(("success", success.clone()));
                    }
                    Some(activity_item("file_operation", fields, ts))
                }
                "schema_diff" => {
                    let mut fields = vec![("filePath", text("filePath"))];
                    if let Some(hunks) = a.get("hunks") {
                        fields.push(("hunks", hunks.clone()));
                    }
                    Some(activity_item("schema_diff", fields, ts))
                }
                _ => None,
            }
        })
        .collect()
}

fn subagent_messages_to_items(messages: &[Value]) -> Vec<Value> {
    messages
        .iter()
        .map(|m| {
            let ts = get_number_or(m, "timestamp", 0.0);
            let message = Value::string(get_string_or(m, "message", ""));
            match get_string(m, "tool").filter(|tool| !tool.is_empty()) {
                Some(tool) => activity_item(
                    "tool_call",
                    vec![("tool", Value::string(tool)), ("argsText", message)],
                    ts,
                ),
                None => activity_item(
                    "message",
                    vec![("role", s("system")), ("content", message)],
                    ts,
                ),
            }
        })
        .collect()
}

fn inline_row(activity: &Value) -> Value {
    let base_row = "items-start px-3 py-1 hover:bg-[var(--color-surface)]";
    let kind = get_string_or(activity, "type", "");
    let time = hhmmss(get_number_or(activity, "timestamp", 0.0));
    let time_el = tx(
        "caption",
        Some("muted"),
        "text-[10px] flex-shrink-0 tabular-nums mt-0.5",
        &time,
    );
    let body_class = "flex-1 min-w-0 text-[11px] font-mono break-all whitespace-pre-wrap";
    match kind.as_str() {
        "tool_call" => {
            let args = get_string_or(activity, "argsText", "");
            let mut inner = vec![tx_span("caption", None, Some("semibold"), "text-[11px] font-mono", &get_string_or(activity, "tool", ""))];
            if args != "" && args != "{}" {
                inner.push(tx_span("caption", Some("muted"), None, "text-[11px] font-mono", &format!(" {args}")));
            }
            hs(
                "sm",
                base_row,
                vec![
                    icon("terminal", "xs", "mt-0.5 flex-shrink-0 text-[var(--color-primary)]"),
                    tx_with("caption", None, body_class, inner),
                    time_el,
                ],
            )
        }
        "tool_result" => {
            let success = activity.get("success").and_then(Value::as_bool).unwrap_or(true);
            let tint = if success { "text-[var(--color-success)]" } else { "text-[var(--color-danger)]" };
            let inner = vec![
                tx_span("caption", Some("muted"), None, "text-[11px] font-mono", &format!("{}: ", get_string_or(activity, "tool", ""))),
                tx_span("caption", None, None, "text-[11px] font-mono", &get_string_or(activity, "resultText", "")),
            ];
            hs(
                "sm",
                base_row,
                vec![
                    icon(if success { "check" } else { "x" }, "xs", &format!("mt-0.5 flex-shrink-0 {tint}")),
                    tx_with("caption", None, body_class, inner),
                    time_el,
                ],
            )
        }
        "message" => {
            let role = get_string_or(activity, "role", "");
            let icon_name = match role.as_str() {
                "user" => "user",
                "system" => "info",
                _ => "sparkles",
            };
            let tint = match role.as_str() {
                "user" => "text-[var(--color-primary)]",
                "system" => "text-[var(--color-muted-foreground)]",
                _ => "text-[var(--color-foreground)]",
            };
            hs(
                "sm",
                base_row,
                vec![
                    icon(icon_name, "xs", &format!("mt-0.5 flex-shrink-0 {tint}")),
                    tx("caption", None, "flex-1 min-w-0 text-[11px] whitespace-pre-wrap break-words", &get_string_or(activity, "content", "")),
                    time_el,
                ],
            )
        }
        "error" => hs(
            "sm",
            base_row,
            vec![
                icon("alert-triangle", "xs", "mt-0.5 flex-shrink-0 text-[var(--color-danger)]"),
                tx("caption", None, "flex-1 min-w-0 text-[11px] text-[var(--color-danger)] whitespace-pre-wrap break-words", &get_string_or(activity, "message", "")),
                time_el,
            ],
        ),
        other => hs(
            "sm",
            base_row,
            vec![
                icon("file", "xs", "mt-0.5 flex-shrink-0 text-[var(--color-muted-foreground)]"),
                tx("caption", Some("muted"), "flex-1 min-w-0 text-[11px] font-mono break-all", &format!("[{other}] {}", json_text(activity, false))),
                time_el,
            ],
        ),
    }
}

fn inline_stream(activities: &[Value], class: &str) -> Value {
    let mut children: Vec<Value> = activities.iter().map(inline_row).collect();
    children.push(bx("", Vec::new()));
    bx(class, children)
}

fn subagent_row(sub: &Value) -> Value {
    let recent = last_message(sub).unwrap_or_else(|| get_string_or(sub, "task", ""));
    hs(
        "sm",
        &format!("items-start px-3 py-2 {BORDER_B}"),
        vec![
            bx(
                "pt-0.5",
                vec![icon(
                    status_icon(&get_string_or(sub, "status", "")),
                    "xs",
                    "",
                )],
            ),
            vs(
                "none",
                "flex-1 min-w-0",
                vec![
                    hs(
                        "sm",
                        "items-center",
                        vec![
                            tx(
                                "caption",
                                None,
                                "font-medium truncate",
                                &get_string_or(sub, "name", ""),
                            ),
                            badge("default", "", &get_string_or(sub, "role", "")),
                        ],
                    ),
                    tx("caption", Some("muted"), "truncate", &recent),
                ],
            ),
        ],
    )
}

fn orbital_group(name: &str, subs: &[Value]) -> Value {
    let running = subs
        .iter()
        .filter(|sub| get_string_or(sub, "status", "") == "running")
        .count();
    let complete = subs
        .iter()
        .filter(|sub| get_string_or(sub, "status", "") == "complete")
        .count();
    let label = if running > 0 {
        format!("{running} running")
    } else {
        format!("{complete} done")
    };
    let mut children = vec![hs(
        "sm",
        &format!("items-center px-3 py-2 bg-[var(--color-surface)] {BORDER_B}"),
        vec![
            icon("circle", "xs", ""),
            tx("caption", None, "font-semibold flex-1 truncate", name),
            badge("default", "", &label),
        ],
    )];
    children.extend(subs.iter().map(subagent_row));
    vs("none", BORDER_B, children)
}

fn rich_card(sub: &Value) -> Value {
    let status = get_string_or(sub, "status", "");
    let role = get_string_or(sub, "role", "");
    let dot_color = match status.as_str() {
        "complete" => "var(--color-success)",
        "error" => "var(--color-error)",
        _ => "var(--color-primary)",
    };
    let mut dot_style = vec![
        ("width", "8px".to_string()),
        ("height", "8px".to_string()),
        ("borderRadius", "50%".to_string()),
        ("flexShrink", "0".to_string()),
        ("backgroundColor", dot_color.to_string()),
    ];
    if status == "running" {
        dot_style.push(("animation", "pulse 1.5s infinite".to_string()));
    }
    let badge_variant = match role.as_str() {
        "deterministic" => "default",
        "llm" => "info",
        _ => "warning",
    };
    let mut header = vec![
        bx_style("", dot_style, Vec::new()),
        tx(
            "caption",
            None,
            "font-semibold flex-1 truncate",
            &get_string_or(sub, "name", ""),
        ),
        badge(badge_variant, "", &role),
    ];
    let duration = get_number_or(sub, "durationMs", 0.0);
    if sub.get("durationMs").is_some() && duration > 0.0 {
        let text = if duration < 1000.0 {
            format!("{}ms", js_number(duration))
        } else {
            format!("{:.1}s", duration / 1000.0)
        };
        header.push(tx("caption", Some("muted"), "", &text));
    }
    let items = subagent_messages_to_items(&get_array(sub, "messages"));
    let tail = if items.is_empty() {
        let text = if status == "complete" {
            "No conversation log"
        } else {
            "Waiting for process data…"
        };
        tx("caption", Some("muted"), "italic text-[10px]", text)
    } else {
        bx(
            &format!("max-h-64 overflow-y-auto border-t border-[var(--color-border)]"),
            vec![inline_stream(&items, "")],
        )
    };
    vs(
        "none",
        &format!("{BORDER_B} p-3"),
        vec![
            hs("sm", "items-center mb-2", header),
            tx(
                "caption",
                Some("muted"),
                "mb-2",
                &get_string_or(sub, "task", ""),
            ),
            tail,
        ],
    )
}

fn js_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e21 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

fn role_badge_variant(role: &str) -> &'static str {
    match role {
        "system" => "neutral",
        "user" => "info",
        "tool" => "warning",
        _ => "default",
    }
}

fn code_block(code: &str, max_height: &str) -> Value {
    pattern_spec(
        "code-block",
        [
            ("code", s(code)),
            ("language", s("json")),
            ("maxHeight", s(max_height)),
        ],
    )
}

fn markdown(content: &str) -> Value {
    pattern_spec("markdown-content", [("content", s(content))])
}

fn tool_call_items(tool_calls: &[Value], prefix: &str) -> Value {
    let items: Vec<Value> = tool_calls
        .iter()
        .enumerate()
        .map(|(i, tc)| {
            let args = tc.get("args");
            let title = format!(
                "{}({})",
                get_string_or(tc, "name", ""),
                summarize_args(args)
            );
            let code = args.map(|args| json_text(args, true)).unwrap_or_default();
            object_value([
                ("id", Value::string(format!("{prefix}-{i}"))),
                ("title", Value::string(title)),
                ("content", code_block(&code, "24rem")),
            ])
        })
        .collect();
    pattern_spec(
        "accordion",
        [
            ("multiple", Value::Bool(true)),
            ("items", array_value(items)),
        ],
    )
}

fn tool_body(content: &str) -> Value {
    match pretty_json(content) {
        Some(pretty) => code_block(&pretty, "24rem"),
        None => tx(
            "body",
            None,
            "whitespace-pre-wrap font-mono text-[11px]",
            content,
        ),
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn chat_row(message: &Value, index: usize) -> Value {
    let role = get_string_or(message, "role", "");
    let content = get_string_or(message, "content", "");
    if role == "tool" {
        let tool_name = get_string(message, "toolName").unwrap_or_else(|| "tool".to_string());
        let suffix = get_string(message, "toolCallId")
            .filter(|id| !id.is_empty())
            .map(|id| format!(" {}", short_id(&id)))
            .unwrap_or_default();
        let items = vec![object_value([
            ("id", Value::string(format!("tool-{index}"))),
            ("title", Value::string(format!("{tool_name} →{suffix}"))),
            ("content", tool_body(&content)),
        ])];
        return bx(
            &format!("px-3 py-2 {BORDER_B}"),
            vec![pattern_spec("accordion", [("items", array_value(items))])],
        );
    }
    let reasoning = get_string(message, "reasoningContent").filter(|text| !text.is_empty());
    let tool_calls = get_array(message, "toolCalls");
    let mut head = vec![badge(
        role_badge_variant(&role),
        "text-[10px] uppercase",
        &role,
    )];
    if reasoning.is_some() {
        head.push(tx("caption", Some("muted"), "text-[10px]", "· thinking"));
    }
    if !tool_calls.is_empty() {
        let label = if tool_calls.len() == 1 {
            "tool call"
        } else {
            "tool calls"
        };
        head.push(tx(
            "caption",
            Some("muted"),
            "text-[10px]",
            &format!("· {} {label}", tool_calls.len()),
        ));
    }
    let mut children = vec![hs("xs", "items-center", head)];
    if let Some(reasoning) = reasoning {
        let items = vec![object_value([
            ("id", Value::string(format!("reasoning-{index}"))),
            ("title", s("Reasoning")),
            ("content", markdown(&reasoning)),
        ])];
        children.push(pattern_spec("accordion", [("items", array_value(items))]));
    }
    if !content.is_empty() {
        children.push(markdown(&content));
    }
    if !tool_calls.is_empty() {
        children.push(tool_call_items(&tool_calls, &format!("tc-{index}")));
    }
    vs("xs", &format!("px-3 py-2 {BORDER_B}"), children)
}

fn conversation(messages: &[Value]) -> Value {
    let mut children: Vec<Value> = messages
        .iter()
        .enumerate()
        .map(|(i, m)| chat_row(m, i))
        .collect();
    children.push(bx("", Vec::new()));
    bx("", children)
}

fn coordinator_card(activities: &[Value]) -> Option<Value> {
    let mut decision = None;
    let mut plan = None;
    let mut questions = Vec::new();
    for a in activities {
        match get_string_or(a, "type", "").as_str() {
            "coordinator_decision" => decision = Some(a),
            "plan_committed" => plan = Some(a),
            "pending_question" => questions.push(a),
            _ => {}
        }
    }
    if decision.is_none() && plan.is_none() && questions.is_empty() {
        return None;
    }
    let label_class = "text-[10px] uppercase tracking-wide";
    let mut children = vec![hs(
        "xs",
        "items-center mb-2",
        vec![
            icon("compass", "xs", ""),
            tx("caption", None, "font-semibold text-[11px]", "Coordinator"),
        ],
    )];
    if let Some(decision) = decision {
        let organism = get_string_or(decision, "organism", "");
        let mut row = vec![
            tx("caption", Some("muted"), label_class, "Organism"),
            badge("default", "text-[10px]", &organism),
        ];
        if let Some(prior) = get_string(decision, "priorOrganism")
            .filter(|prior| !prior.is_empty() && *prior != organism)
        {
            row.push(tx(
                "caption",
                Some("muted"),
                "text-[10px]",
                &format!("was {prior}"),
            ));
        }
        let mut block = vec![hs("xs", "items-center", row)];
        if let Some(reason) = get_string(decision, "reason").filter(|reason| !reason.is_empty()) {
            block.push(tx("caption", Some("muted"), "text-[11px] mt-0.5", &reason));
        }
        children.push(vs("none", "mb-2", block));
    }
    if let Some(plan) = plan {
        let orbitals = get_array(plan, "orbitals");
        if !orbitals.is_empty() {
            let head = if orbitals.len() == 1 {
                format!("Plan · {} orbital", orbitals.len())
            } else {
                format!("Plan · {} orbitals", orbitals.len())
            };
            let badges = orbitals
                .iter()
                .map(|o| badge("neutral", "text-[10px]", &js_string_of(o)))
                .collect();
            children.push(vs(
                "none",
                "mb-2",
                vec![
                    tx(
                        "caption",
                        Some("muted"),
                        &format!("{label_class} mb-0.5"),
                        &head,
                    ),
                    hs("xs", "flex-wrap", badges),
                ],
            ));
        }
    }
    if !questions.is_empty() {
        let head = if questions.len() == 1 {
            format!("{} pending question", questions.len())
        } else {
            format!("{} pending questions", questions.len())
        };
        let mut block = vec![tx(
            "caption",
            Some("muted"),
            &format!("{label_class} mb-0.5"),
            &head,
        )];
        for q in questions {
            let mut inner = vec![tx_span(
                "caption",
                None,
                None,
                "",
                &get_string_or(q, "question", ""),
            )];
            if let Some(orbital) = get_string(q, "orbitalName").filter(|name| !name.is_empty()) {
                inner.push(tx_span(
                    "caption",
                    Some("muted"),
                    None,
                    "text-[10px] ml-1",
                    &format!("({orbital})"),
                ));
            }
            block.push(hs(
                "xs",
                "items-start",
                vec![
                    icon(
                        "help-circle",
                        "xs",
                        "mt-0.5 flex-shrink-0 text-[var(--color-warning)]",
                    ),
                    tx_with("caption", None, "text-[11px] flex-1 min-w-0", inner),
                ],
            ));
        }
        children.push(vs("none", "", block));
    }
    Some(bx(
        "px-3 py-2 border-b border-[var(--color-border)] bg-[var(--color-muted)]/30",
        children,
    ))
}

fn js_string_of(value: &Value) -> String {
    crate::renderer::js_string(value)
}

#[derive(Clone)]
enum Timeline {
    Message(Value, usize),
    Activity(Value),
    Subagent(Value),
}

fn timeline_items(messages: &[Value], activities: &[Value], subagents: &[Value]) -> Vec<Timeline> {
    let mut items = Vec::new();
    items.extend(
        messages
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, m)| Timeline::Message(m, i)),
    );
    items.extend(activities.iter().cloned().map(Timeline::Activity));
    items.extend(subagents.iter().cloned().map(Timeline::Subagent));
    items
}

fn timeline_icon(item: &Timeline) -> (&'static str, &'static str) {
    const PRIMARY: &str = "text-[var(--color-primary)]";
    const MUTED: &str = "text-[var(--color-muted-foreground)]";
    match item {
        Timeline::Message(m, _) => {
            let role = get_string_or(m, "role", "");
            if role == "tool" {
                ("terminal", PRIMARY)
            } else if get_string(m, "reasoningContent").is_some_and(|text| !text.is_empty()) {
                ("sparkles", PRIMARY)
            } else if role == "user" {
                ("user", PRIMARY)
            } else {
                ("message-square", MUTED)
            }
        }
        Timeline::Activity(a) => match get_string_or(a, "type", "").as_str() {
            "tool_call" => ("terminal", PRIMARY),
            "tool_result" => {
                if a.get("success").and_then(Value::as_bool).unwrap_or(true) {
                    ("check", "text-[var(--color-success)]")
                } else {
                    ("x", "text-[var(--color-danger)]")
                }
            }
            "error" => ("alert-triangle", "text-[var(--color-danger)]"),
            "file_operation" => ("file", MUTED),
            "schema_diff" => ("git-commit", MUTED),
            _ => ("info", MUTED),
        },
        Timeline::Subagent(_) => ("bot", PRIMARY),
    }
}

fn timeline_label(item: &Timeline) -> String {
    match item {
        Timeline::Message(m, _) => {
            let role = get_string_or(m, "role", "");
            if role == "tool" {
                "tool".to_string()
            } else if get_string(m, "reasoningContent").is_some_and(|text| !text.is_empty()) {
                "thinking".to_string()
            } else {
                role
            }
        }
        Timeline::Activity(a) => activity_type_label(&get_string_or(a, "type", "")),
        Timeline::Subagent(_) => "subagent".to_string(),
    }
}

fn timeline_preview(item: &Timeline) -> String {
    match item {
        Timeline::Message(m, _) => {
            let content = get_string_or(m, "content", "");
            if get_string_or(m, "role", "") == "tool" {
                let name = get_string(m, "toolName").unwrap_or_else(|| "tool".to_string());
                let id = get_string(m, "toolCallId")
                    .map(|id| short_id(&id))
                    .unwrap_or_default();
                return format!("{name} → {id}");
            }
            get_string(m, "reasoningContent")
                .filter(|text| !text.is_empty())
                .unwrap_or(content)
        }
        Timeline::Activity(a) => {
            let kind = get_string_or(a, "type", "");
            match kind.as_str() {
                "tool_call" => format!(
                    "{}({})",
                    get_string_or(a, "tool", ""),
                    get_string_or(a, "argsText", "")
                ),
                "tool_result" => format!(
                    "{}: {}",
                    get_string_or(a, "tool", ""),
                    get_string_or(a, "resultText", "")
                ),
                "message" => get_string_or(a, "content", ""),
                "error" => get_string_or(a, "message", ""),
                "file_operation" => format!(
                    "{} {}",
                    file_operation_label(&get_string_or(a, "operation", "")),
                    get_string_or(a, "path", "")
                ),
                "schema_diff" => format!("diff {}", get_string_or(a, "filePath", "")),
                other => other.to_string(),
            }
        }
        Timeline::Subagent(sub) => format!(
            "{} · {}",
            get_string_or(sub, "name", ""),
            get_string_or(sub, "task", "")
        ),
    }
}

fn timeline_time(item: &Timeline) -> Option<String> {
    match item {
        Timeline::Activity(a) => Some(hhmmss(get_number_or(a, "timestamp", 0.0))),
        Timeline::Subagent(sub) => get_array(sub, "messages")
            .first()
            .map(|m| hhmmss(get_number_or(m, "timestamp", 0.0))),
        Timeline::Message(..) => None,
    }
}

fn detail_head(badge_spec: Value, time: Option<String>) -> Value {
    let mut row = vec![badge_spec];
    if let Some(time) = time {
        row.push(tx("caption", Some("muted"), "", &time));
    }
    hs("xs", "items-center", row)
}

fn detail(item: &Timeline) -> Value {
    match item {
        Timeline::Message(m, _) => {
            let role = get_string_or(m, "role", "");
            let content = get_string_or(m, "content", "");
            let mut children = vec![hs(
                "xs",
                "items-center",
                vec![badge(
                    role_badge_variant(&role),
                    "text-[10px] uppercase",
                    &role,
                )],
            )];
            if let Some(reasoning) =
                get_string(m, "reasoningContent").filter(|text| !text.is_empty())
            {
                children.push(bx(
                    "",
                    vec![
                        tx("caption", Some("muted"), "mb-1", "Reasoning"),
                        markdown(&reasoning),
                    ],
                ));
            }
            if !content.is_empty() {
                let body = if role == "tool" {
                    tool_body(&content)
                } else {
                    markdown(&content)
                };
                children.push(bx(
                    "",
                    vec![tx("caption", Some("muted"), "mb-1", "Content"), body],
                ));
            }
            let calls = get_array(m, "toolCalls");
            if !calls.is_empty() {
                children.push(bx(
                    "",
                    vec![
                        tx("caption", Some("muted"), "mb-1", "Tool calls"),
                        tool_call_items(&calls, "tc-detail"),
                    ],
                ));
            }
            vs("md", "", children)
        }
        Timeline::Activity(a) => {
            let kind = get_string_or(a, "type", "");
            let time = Some(hhmmss(get_number_or(a, "timestamp", 0.0)));
            match kind.as_str() {
                "tool_call" => vs(
                    "md",
                    "",
                    vec![
                        detail_head(
                            badge("default", "text-[10px]", &activity_type_label("tool_call")),
                            time,
                        ),
                        tx("body2", None, "", &get_string_or(a, "tool", "")),
                        code_block(&get_string_or(a, "argsText", ""), "60vh"),
                    ],
                ),
                "tool_result" => {
                    let ok = a.get("success").and_then(Value::as_bool).unwrap_or(true);
                    vs(
                        "md",
                        "",
                        vec![
                            detail_head(
                                badge(
                                    if ok { "default" } else { "warning" },
                                    "text-[10px]",
                                    if ok { "success" } else { "fail" },
                                ),
                                time,
                            ),
                            tx("body2", None, "", &get_string_or(a, "tool", "")),
                            code_block(&get_string_or(a, "resultText", ""), "60vh"),
                        ],
                    )
                }
                "message" => vs(
                    "md",
                    "",
                    vec![
                        detail_head(
                            badge("neutral", "text-[10px]", &get_string_or(a, "role", "")),
                            time,
                        ),
                        tx(
                            "body2",
                            None,
                            "whitespace-pre-wrap",
                            &get_string_or(a, "content", ""),
                        ),
                    ],
                ),
                "error" => vs(
                    "md",
                    "",
                    vec![
                        detail_head(badge("warning", "text-[10px]", "error"), time),
                        tx(
                            "body2",
                            None,
                            "whitespace-pre-wrap text-[var(--color-danger)]",
                            &get_string_or(a, "message", ""),
                        ),
                    ],
                ),
                "file_operation" => vs(
                    "md",
                    "",
                    vec![
                        detail_head(badge("neutral", "text-[10px]", "file"), time),
                        tx(
                            "body2",
                            None,
                            "",
                            &format!(
                                "{} {}",
                                file_operation_label(&get_string_or(a, "operation", "")),
                                get_string_or(a, "path", "")
                            ),
                        ),
                    ],
                ),
                "schema_diff" => vs(
                    "md",
                    "",
                    vec![
                        detail_head(badge("neutral", "text-[10px]", "diff"), time),
                        tx("body2", None, "", &get_string_or(a, "filePath", "")),
                    ],
                ),
                other => vs(
                    "md",
                    "",
                    vec![
                        tx("caption", Some("muted"), "", &time.unwrap_or_default()),
                        tx("body2", None, "", other),
                    ],
                ),
            }
        }
        Timeline::Subagent(sub) => {
            let messages = get_array(sub, "messages");
            let ts = messages
                .first()
                .map(|m| hhmmss(get_number_or(m, "timestamp", 0.0)));
            let items = subagent_messages_to_items(&messages);
            let mut children = vec![
                detail_head(
                    badge("default", "text-[10px]", &get_string_or(sub, "role", "")),
                    ts,
                ),
                pattern_spec(
                    "typography",
                    [
                        ("variant", s("body2")),
                        ("weight", s("semibold")),
                        ("content", s(&get_string_or(sub, "name", ""))),
                    ],
                ),
                tx("body2", Some("muted"), "", &get_string_or(sub, "task", "")),
            ];
            if !items.is_empty() {
                children.push(bx(
                    "border-t border-[var(--color-border)] pt-2",
                    vec![inline_stream(&items, "")],
                ));
            }
            vs("md", "", children)
        }
    }
}

fn panel_header(icon_size: &str, title: Value, close: Option<&str>) -> Value {
    let mut row = vec![icon("activity", icon_size, ""), title];
    if let Some(action) = close {
        row.push(ghost_icon_button(Some(action), "x", ""));
    }
    hs("sm", &format!("items-center px-3 py-2 {BORDER_B}"), row)
}

#[allow(non_snake_case)]
fn Panel(input: PanelProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let mut selected = use_signal(|| Option::<usize>::None);

    let mode = get_string_or(props, "mode", "overlay");
    let is_tab = mode == "tab";
    let open = get_bool_or(props, "open", false);
    if !is_tab && !open {
        return rsx! {};
    }
    let rich = is_tab || get_number_or(props, "disclosureLevel", 0.0) >= 3.0;
    let class_name = get_string_or(props, "className", "");
    let on_close = get_string(props, "onClose").filter(|name| !name.is_empty());
    let close = on_close.as_deref();
    let subagents = get_array(props, "subagents");
    let activities = get_array(props, "coordinatorActivities");
    let messages = get_array(props, "coordinatorMessages");
    let a11y = dom_passthrough(props);
    let root = |class: &str, style: Option<Value>, children: Vec<Value>| {
        layout_box_spec(class, style, a11y.clone(), children)
    };

    let (spec, modal_items) = if is_tab {
        let items = timeline_items(&messages, &activities, &subagents);
        let mut children = vec![panel_header(
            "sm",
            tx("caption", None, "font-semibold flex-1", "Trace"),
            close,
        )];
        if items.is_empty() {
            children.push(bx(
                "flex-1 flex items-center justify-center px-4",
                vec![tx(
                    "body2",
                    Some("muted"),
                    "text-center",
                    "No agent activity yet",
                )],
            ));
        } else {
            let count = items.len();
            let rows: Vec<Value> = items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let (icon_name, icon_color) = timeline_icon(item);
                    let action = format!("{SELECT_PREFIX}{i}");
                    let mut gutter = vec![bx_style(
                        "rounded-full flex-shrink-0",
                        vec![("width", "8px".to_string()), ("height", "8px".to_string()), ("backgroundColor", "var(--color-primary)".to_string())],
                        Vec::new(),
                    )];
                    if i + 1 != count {
                        gutter.push(bx_style(
                            "flex-1",
                            vec![
                                ("width", "2px".to_string()),
                                ("backgroundColor", "var(--color-border)".to_string()),
                                ("marginTop", "4px".to_string()),
                                ("minHeight", "24px".to_string()),
                            ],
                            Vec::new(),
                        ));
                    }
                    let mut head = vec![icon(icon_name, "xs", &format!("flex-shrink-0 {icon_color}"))];
                    if let Some(time) = timeline_time(item) {
                        head.push(tx("caption", Some("muted"), "", &time));
                    }
                    head.push(badge("neutral", "text-[10px]", &timeline_label(item)));
                    let content = vs(
                        "xs",
                        "flex-1 min-w-0",
                        vec![
                            hs("xs", "items-center", head),
                            hs(
                                "sm",
                                "items-start",
                                vec![
                                    tx("body2", Some("muted"), "flex-1 min-w-0 line-clamp-2", &timeline_preview(item)),
                                    pattern_spec(
                                        "button",
                                        [
                                            ("variant", s("ghost")),
                                            ("size", s("sm")),
                                            ("className", s("flex-shrink-0 mt-0")),
                                            ("action", Value::string(action.clone())),
                                            ("children", array_value(vec![icon("maximize-2", "xs", "")])),
                                        ],
                                    ),
                                ],
                            ),
                        ],
                    );
                    layout_box_spec(
                        &format!("flex items-start px-3 py-2 {BORDER_B} hover:bg-[var(--color-muted)]/30 transition-colors duration-fast cursor-pointer"),
                        None,
                        vec![("action".to_string(), Value::string(action))],
                        vec![
                            bx_style("flex flex-col items-center mr-3 mt-0.5", vec![("width", "20px".to_string())], gutter),
                            content,
                        ],
                    )
                })
                .collect();
            children.push(bx("flex-1 overflow-y-auto", vec![vs("none", "", rows)]));
        }
        let current = *selected.read();
        let chosen = current.and_then(|index| items.get(index).cloned());
        let title = chosen
            .as_ref()
            .map(|item| match timeline_time(item) {
                Some(time) => format!("{} · {time}", timeline_label(item)),
                None => timeline_label(item),
            })
            .unwrap_or_default();
        children.push(pattern_spec(
            "modal",
            [
                ("isOpen", Value::Bool(chosen.is_some())),
                ("closeEvent", s(CLOSE_MODAL)),
                ("title", Value::string(title)),
                ("size", s("lg")),
                ("children", array_value(chosen.iter().map(detail).collect())),
            ],
        ));
        (
            root(&format!("h-full w-full bg-[var(--color-card)] overflow-hidden flex flex-col {class_name}"), None, children),
            true,
        )
    } else {
        let focused = get_string(props, "focusedOrbital").filter(|name| !name.is_empty());
        let filtered: Vec<Value> = match &focused {
            Some(name) => subagents
                .iter()
                .filter(|sub| get_string(sub, "orbitalName").as_deref() == Some(name.as_str()))
                .cloned()
                .collect(),
            None => subagents.clone(),
        };
        let wrapper = if rich {
            format!("absolute inset-x-3 bottom-3 sm:inset-x-auto sm:top-3 sm:right-3 sm:bottom-3 w-full sm:w-[28rem] max-w-[calc(100vw-1.5rem)] max-h-[60vh] sm:max-h-none bg-[var(--color-card)] border border-[var(--color-border)] rounded-container shadow-elevation-popover surface-material overflow-hidden flex flex-col {class_name}")
        } else {
            format!("absolute inset-x-3 bottom-3 sm:inset-x-auto sm:top-3 sm:right-3 sm:bottom-3 w-full sm:w-80 max-w-[calc(100vw-1.5rem)] max-h-[60vh] sm:max-h-none bg-[var(--color-card)] border border-[var(--color-border)] rounded-container shadow-elevation-popover surface-material overflow-hidden flex flex-col {class_name}")
        };
        let style = Some(object_value([("zIndex", s("20"))]));
        let density = if rich { "Detailed" } else { "Compact" };
        let has_coordinator = !messages.is_empty() || !activities.is_empty();
        let children = if filtered.is_empty() && !has_coordinator {
            let empty = match &focused {
                Some(name) => format!("No subagents for {name}"),
                None => "No subagents active".to_string(),
            };
            vec![
                panel_header(
                    "sm",
                    tx("caption", None, "font-semibold flex-1", "Subagents"),
                    close,
                ),
                bx(
                    "flex-1 flex items-center justify-center px-4",
                    vec![tx("caption", Some("muted"), "text-center", &empty)],
                ),
            ]
        } else if let Some(name) = &focused {
            let count = if filtered.len() == 1 {
                format!("{} subagent", filtered.len())
            } else {
                format!("{} subagents", filtered.len())
            };
            let title = vs(
                "none",
                "flex-1 min-w-0",
                vec![
                    tx("caption", None, "font-semibold truncate", name),
                    tx(
                        "caption",
                        Some("muted"),
                        "",
                        &format!("{count} · {density}"),
                    ),
                ],
            );
            let rows = filtered
                .iter()
                .map(|sub| {
                    if rich {
                        rich_card(sub)
                    } else {
                        subagent_row(sub)
                    }
                })
                .collect();
            vec![
                panel_header("sm", title, close),
                bx("flex-1 overflow-y-auto", rows),
            ]
        } else {
            let title = tx(
                "caption",
                None,
                "font-semibold flex-1",
                &format!("Subagents ({}) · {density}", filtered.len()),
            );
            let mut body = Vec::new();
            if rich {
                body.extend(coordinator_card(&activities));
            }
            let dot = |class: &str| {
                bx_style(
                    class,
                    vec![
                        ("width", "6px".to_string()),
                        ("height", "6px".to_string()),
                        ("borderRadius", "50%".to_string()),
                        ("backgroundColor", "var(--color-primary)".to_string()),
                        ("flexShrink", "0".to_string()),
                    ],
                    Vec::new(),
                )
            };
            let section_head = |count: String| {
                hs(
                    "xs",
                    &format!("items-center px-3 py-1.5 bg-[var(--color-card)] {BORDER_B}"),
                    vec![
                        dot(""),
                        tx("caption", None, "font-semibold text-[11px]", "Coordinator"),
                        tx("caption", Some("muted"), "text-[10px]", &count),
                    ],
                )
            };
            if !messages.is_empty() {
                body.push(bx(
                    BORDER_B,
                    vec![
                        section_head(format!("{} messages", messages.len())),
                        bx(
                            "max-h-[36rem] overflow-y-auto",
                            vec![conversation(&messages)],
                        ),
                    ],
                ));
            } else if !activities.is_empty() {
                body.push(bx(
                    BORDER_B,
                    vec![
                        section_head(format!("{} events", activities.len())),
                        bx(
                            "max-h-60 overflow-y-auto",
                            vec![inline_stream(&coordinator_to_items(&activities), "")],
                        ),
                    ],
                ));
            }
            if rich {
                body.extend(filtered.iter().map(rich_card));
            } else {
                let mut order: Vec<String> = Vec::new();
                let mut groups: Vec<Vec<Value>> = Vec::new();
                for sub in &filtered {
                    let key = get_string(sub, "orbitalName")
                        .unwrap_or_else(|| "(unattached)".to_string());
                    match order.iter().position(|existing| *existing == key) {
                        Some(index) => groups[index].push(sub.clone()),
                        None => {
                            order.push(key);
                            groups.push(vec![sub.clone()]);
                        }
                    }
                }
                body.extend(
                    order
                        .iter()
                        .zip(groups.iter())
                        .map(|(name, subs)| orbital_group(name, subs)),
                );
            }
            vec![
                panel_header("sm", title, close),
                bx("flex-1 overflow-y-auto", body),
            ]
        };
        (root(&wrapper, style, children), false)
    };

    let items_for_sink = if modal_items { Some(selected) } else { None };
    render_with_local_sink(&view, registry, &spec, move |event, _| {
        if let (Some(mut selected), Some(index)) =
            (items_for_sink, event.strip_prefix(SELECT_PREFIX))
        {
            if let Ok(index) = index.parse::<usize>() {
                selected.set(Some(index));
                return true;
            }
        }
        if event == CLOSE_MODAL {
            if let Some(mut selected) = items_for_sink {
                selected.set(None);
            }
            return true;
        }
        false
    })
}

impl DioxusPatternRenderer for SubagentTracePanelRenderer {
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
        rsx! { Panel { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
