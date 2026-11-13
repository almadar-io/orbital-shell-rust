//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    format_field_value, js_string, logged_row_payload, logged_row_route, object_value,
    pattern_spec, relation_display_labels, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `detail-panel` pattern.
pub struct DetailPanelRenderer;

impl DetailPanelRenderer {
    const NAME: &str = "detail-panel";
}

const TITLE_EDIT: &str = "__detail_panel_title_edit";
const TITLE_DRAFT: &str = "__detail_panel_title_draft";
const TITLE_COMMIT: &str = "__detail_panel_title_commit";

const PROSE_VARS: [(&str, &str); 13] = [
    ("color", "var(--color-foreground)"),
    ("--tw-prose-body", "var(--color-foreground)"),
    ("--tw-prose-headings", "var(--color-foreground)"),
    ("--tw-prose-bold", "var(--color-foreground)"),
    ("--tw-prose-links", "var(--color-primary)"),
    ("--tw-prose-code", "var(--color-foreground)"),
    ("--tw-prose-hr", "var(--color-border)"),
    ("--tw-prose-quotes", "var(--color-foreground)"),
    ("--tw-prose-quote-borders", "var(--color-primary)"),
    ("--tw-prose-captions", "var(--color-muted-foreground)"),
    ("--tw-prose-th-borders", "var(--color-border)"),
    ("--tw-prose-td-borders", "var(--color-border)"),
    ("--tw-prose-pre-bg", "var(--color-muted)"),
];

const MONTHS_LONG: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

#[derive(Props, Clone, PartialEq)]
struct PanelProps {
    props: Value,
    entity: Option<Value>,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn arr(items: Vec<Value>) -> Value {
    Value::Array(items.into())
}

fn text(content: &str, variant: &str, color: &str, class: &str) -> Value {
    pattern_spec(
        "typography",
        [
            ("content", Value::string(content)),
            ("variant", Value::string(variant)),
            ("color", Value::string(color)),
            ("className", Value::string(class)),
        ],
    )
}

fn weighted(content: &str, variant: &str, color: &str, weight: &str, class: &str) -> Value {
    pattern_spec(
        "typography",
        [
            ("content", Value::string(content)),
            ("variant", Value::string(variant)),
            ("color", Value::string(color)),
            ("weight", Value::string(weight)),
            ("className", Value::string(class)),
        ],
    )
}

fn stack_class(
    direction: &str,
    gap: &str,
    align: &str,
    justify: &str,
    wrap: bool,
    extra: &str,
) -> String {
    let direction = if direction == "horizontal" {
        "flex-row"
    } else {
        "flex-col"
    };
    let gap = match gap {
        "none" => "gap-0",
        "xs" => "gap-1",
        "sm" => "gap-2",
        "lg" => "gap-6",
        "xl" => "gap-8",
        _ => "gap-4",
    };
    let align = match align {
        "start" => "items-start",
        "center" => "items-center",
        "end" => "items-end",
        _ => "items-stretch",
    };
    let justify = match justify {
        "center" => "justify-center",
        "end" => "justify-end",
        "between" => "justify-between",
        _ => "justify-start",
    };
    let wrap = if wrap { "flex-wrap" } else { "" };
    format!("flex {direction} {gap} {align} {justify} {wrap} {extra}")
}

fn resolve_field(input: &Value) -> Option<Value> {
    if let Some(name) = input.as_str() {
        if name.is_empty() {
            return None;
        }
        return Some(object_value([("name", Value::string(name))]));
    }
    let name = get_string(input, "name").or_else(|| get_string(input, "key"))?;
    if name.is_empty() {
        return None;
    }
    let mut out = input.as_object()?.clone();
    out.insert("name".into(), Value::string(name));
    if !out.contains_key("label") {
        if let Some(header) = get_string(input, "header") {
            out.insert("label".into(), Value::string(header));
        }
    }
    Some(Value::Object(out.into()))
}

fn name_of(field: &Value) -> String {
    get_string_or(field, "name", "")
}

fn slot_of(field: &Value) -> &'static str {
    match get_string(field, "variant").as_deref() {
        Some("badge") => return "badge",
        Some("progress") => return "progress",
        Some("body") => return "body",
        _ => {}
    }
    match get_string(field, "format").as_deref() {
        Some("date") | Some("datetime") => "date",
        Some("currency") | Some("number") | Some("percent") => "metric",
        _ => "other",
    }
}

fn badge_variant(value: &str, field: &Value) -> String {
    let declared = field
        .get("colorMap")
        .and_then(|map| map.get(value))
        .and_then(Value::as_str)
        .unwrap_or("default");
    let variant = if declared == "destructive" {
        "danger"
    } else {
        declared
    };
    variant.to_string()
}

fn value_label(value: &str, field: &Value) -> String {
    field
        .get("labels")
        .and_then(|labels| labels.get(value))
        .and_then(Value::as_str)
        .unwrap_or(value)
        .to_string()
}

fn badge(label: &str, variant: &str) -> Value {
    pattern_spec(
        "badge",
        [
            ("variant", Value::string(variant)),
            ("label", Value::string(label)),
        ],
    )
}

fn prose_box(class: &str, children: Vec<Value>) -> Value {
    pattern_spec(
        "box",
        [
            ("className", Value::string(class)),
            (
                "style",
                object_value(
                    PROSE_VARS
                        .iter()
                        .map(|(name, value)| (*name, Value::string(*value)))
                        .collect::<Vec<_>>(),
                ),
            ),
            ("children", arr(children)),
        ],
    )
}

fn long_date(raw: &str, with_time: bool) -> String {
    let (date, time) = match raw.split_once(['T', ' ']) {
        Some((date, time)) => (date, Some(time)),
        None => (raw, None),
    };
    let mut parts = date.split('-');
    let parsed = (|| {
        let year = parts.next()?.parse::<i64>().ok()?;
        let month = parts.next()?.parse::<usize>().ok()?;
        let day = parts.next()?.parse::<i64>().ok()?;
        (1..=12).contains(&month).then_some((year, month, day))
    })();
    let Some((year, month, day)) = parsed else {
        return raw.to_string();
    };
    let base = format!("{} {day}, {year}", MONTHS_LONG[month - 1]);
    if !with_time {
        return base;
    }
    let (hour, minute) = time
        .and_then(|time| {
            let mut parts = time.split([':', '.', 'Z', '+']);
            Some((
                parts.next()?.parse::<i64>().ok()?,
                parts.next()?.parse::<i64>().ok()?,
            ))
        })
        .unwrap_or((0, 0));
    let suffix = if hour >= 12 { "PM" } else { "AM" };
    let hour12 = if hour % 12 == 0 { 12 } else { hour % 12 };
    format!("{base} at {hour12:02}:{minute:02} {suffix}")
}

fn file_size(bytes: f64) -> String {
    if bytes < 1024.0 {
        format!("{}B", bytes as i64)
    } else if bytes < 1024.0 * 1024.0 {
        format!("{}KB", (bytes / 1024.0).round() as i64)
    } else {
        format!("{}MB", (bytes / (1024.0 * 1024.0)).round() as i64)
    }
}

fn link_text(content: &str) -> Value {
    text(
        content,
        "body",
        "primary",
        "text-primary hover:underline break-all",
    )
}

fn rich_value(field: &Value, value: Option<&Value>, options: Option<&Value>) -> Value {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Value::string("—");
    };
    let name = name_of(field);
    let string = js_string(value);
    if let Some(format) = get_string(field, "format") {
        return Value::string(format_field_value(value, Some(&format)));
    }
    match get_string(field, "type").as_deref() {
        Some("image") => {
            if string.is_empty() {
                return Value::string("—");
            }
            pattern_spec(
                "box",
                [
                    ("className", Value::string("mt-1 max-w-full max-h-64")),
                    (
                        "children",
                        arr(vec![pattern_spec(
                            "image",
                            [
                                ("src", Value::string(string.as_str())),
                                ("alt", Value::string(name.as_str())),
                                ("fit", Value::string("contain")),
                                ("rounded", Value::string("md")),
                            ],
                        )]),
                    ),
                ],
            )
        }
        Some("url") => {
            let lower = string.to_ascii_lowercase();
            if lower.starts_with("http://") || lower.starts_with("https://") {
                link_text(&string)
            } else {
                Value::string(string)
            }
        }
        Some("markdown") | Some("richtext") => pattern_spec(
            "markdown-content",
            [
                ("content", Value::string(string)),
                ("className", Value::string("prose-sm")),
            ],
        ),
        Some("code") => pattern_spec(
            "box",
            [
                (
                    "className",
                    Value::string("mt-1 rounded-container bg-muted p-3 overflow-x-auto"),
                ),
                (
                    "children",
                    arr(vec![text(
                        &string,
                        "body",
                        "primary",
                        "text-sm font-mono whitespace-pre-wrap break-words m-0",
                    )]),
                ),
            ],
        ),
        Some("html") => prose_box(
            "mt-1 prose prose-sm max-w-none break-words",
            vec![text(&string, "body", "primary", "")],
        ),
        Some(kind @ ("date" | "datetime" | "timestamp")) => {
            Value::string(long_date(&string, kind != "date"))
        }
        Some("money") => {
            let number = value
                .as_number()
                .filter(|_| value.is_numeric())
                .or_else(|| string.trim().parse::<f64>().ok());
            match number {
                Some(number) => {
                    Value::string(format_field_value(&Value::Number(number), Some("currency")))
                }
                None => Value::string(string),
            }
        }
        Some("relation") => {
            let labels = relation_display_labels(value, options);
            match labels.len() {
                0 => Value::string("—"),
                1 => Value::string(labels[0].as_str()),
                _ => pattern_spec(
                    "hstack",
                    [
                        ("gap", Value::string("xs")),
                        ("wrap", Value::Bool(true)),
                        (
                            "children",
                            arr(labels.iter().map(|label| badge(label, "default")).collect()),
                        ),
                    ],
                ),
            }
        }
        Some("file") => {
            if let Value::Object(_) = value {
                let label = get_string(value, "name")
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "file".to_string());
                let mut parts = vec![
                    pattern_spec(
                        "icon",
                        [
                            ("name", Value::string("file-text")),
                            ("size", Value::string("sm")),
                            ("className", Value::string("text-muted-foreground")),
                        ],
                    ),
                    text(&label, "body", "primary", "text-sm"),
                ];
                if let Some(bytes) = value.get("sizeBytes").and_then(Value::as_number) {
                    parts.push(text(
                        &format!(" · {}", file_size(bytes)),
                        "caption",
                        "muted",
                        "",
                    ));
                }
                pattern_spec(
                    "box",
                    [
                        (
                            "className",
                            Value::string("inline-flex items-center gap-1.5 rounded-interactive border border-border bg-muted px-2 py-1 text-sm text-foreground no-underline hover:bg-accent hover:text-accent-foreground"),
                        ),
                        ("children", arr(parts)),
                    ],
                )
            } else {
                Value::string(string)
            }
        }
        Some("boolean") => match value {
            Value::Bool(flag) => Value::string(if *flag { "Yes" } else { "No" }),
            _ if string == "true" => Value::string("Yes"),
            _ if string == "false" => Value::string("No"),
            _ => Value::string(string),
        },
        Some("array") => match value {
            Value::Array(items) if !items.is_empty() => pattern_spec(
                "hstack",
                [
                    ("gap", Value::string("xs")),
                    ("wrap", Value::Bool(true)),
                    (
                        "children",
                        arr(items
                            .iter()
                            .map(|item| badge(&js_string(item), "default"))
                            .collect()),
                    ),
                ],
            ),
            Value::Array(_) => Value::string("—"),
            _ => Value::string(string),
        },
        Some("email") | Some("phone") => link_text(&string),
        _ => {
            let in_vocabulary = get_array(field, "values")
                .iter()
                .any(|candidate| candidate.as_str() == Some(string.as_str()));
            if in_vocabulary {
                badge(&value_label(&string, field), &badge_variant(&string, field))
            } else {
                Value::string(format_field_value(value, None))
            }
        }
    }
}

#[allow(non_snake_case)]
fn Panel(input: PanelProps) -> Element {
    let props = &input.props;
    let registry = &input.registry;
    let mut title_draft = use_signal(|| None::<String>);

    let look = get_string_or(props, "look", "panel");
    let slide_over = get_bool_or(props, "slideOver", false);
    let show_actions = get_bool_or(props, "showActions", true);
    let max_inline = get_number(props, "maxInlineActions")
        .map(|n| n.max(0.0) as usize)
        .unwrap_or(2);
    let class_name = get_string_or(props, "className", "");
    let surface_none = get_string(props, "surface").as_deref() == Some("none");
    let surface = if surface_none { "" } else { "surface-content" };

    let entity_value = match &input.entity {
        Some(Value::Array(items)) => items.first().cloned(),
        other => other.clone(),
    };
    let data = entity_value
        .filter(|value| matches!(value, Value::Object(_)))
        .or_else(|| {
            props
                .get("initialData")
                .filter(|value| matches!(value, Value::Object(_)))
                .cloned()
        });

    let field_inputs = get_array(props, "fields");
    let structured = field_inputs.first().is_some_and(|first| {
        first.as_str().is_some()
            || (matches!(first, Value::Object(_))
                && (first.get("key").is_some() || first.get("name").is_some()))
    });
    let resolved: Option<Vec<Value>> = if structured {
        Some(field_inputs.iter().filter_map(resolve_field).collect())
    } else {
        props
            .get("fieldNames")
            .and_then(Value::as_array)
            .map(|names| {
                names
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|name| object_value([("name", Value::string(name))]))
                    .collect()
            })
    };
    let find_field = |name: &str| -> Value {
        resolved
            .as_ref()
            .and_then(|fields| fields.iter().find(|field| name_of(field) == name))
            .cloned()
            .unwrap_or_else(|| object_value([("name", Value::string(name))]))
    };
    let label_for = |name: &str| -> String {
        resolved
            .as_ref()
            .and_then(|fields| fields.iter().find(|field| name_of(field) == name))
            .and_then(|field| get_string(field, "label"))
            .unwrap_or_else(|| name.to_string())
    };
    let relations = props.get("relationsData").cloned();
    let rich = |field: &Value, value: Option<&Value>| -> Value {
        rich_value(
            field,
            value,
            relations
                .as_ref()
                .and_then(|data| data.get(&name_of(field))),
        )
    };

    let title_commit = get_string(props, "onTitleCommit").filter(|event| !event.is_empty());
    let mut title = get_string(props, "title").filter(|title| !title.is_empty());
    let record_id = data
        .as_ref()
        .and_then(|data| data.get("id"))
        .filter(|id| !id.is_null())
        .map(js_string);

    let declared_sections = get_array(props, "sections");
    let has_declared = !declared_sections.is_empty();
    let mut sections: Vec<(String, Vec<Value>)> = Vec::new();
    let mut key_figures: Vec<Value> = Vec::new();
    let mut prose_fields: Vec<Value> = Vec::new();
    let detail_field = |label: String, value: Value| {
        object_value([("label", Value::string(label)), ("value", value)])
    };

    if has_declared {
        for section in &declared_sections {
            let fields: Vec<Value> = get_array(section, "fields")
                .iter()
                .map(|field| match field.as_str() {
                    Some(name) => {
                        let shown = match data.as_ref().and_then(|data| data.get_path(name)) {
                            Some(value) if !value.is_null() => rich(&find_field(name), Some(value)),
                            _ => Value::string("—"),
                        };
                        detail_field(label_for(name), shown)
                    }
                    None => field.clone(),
                })
                .collect();
            sections.push((get_string_or(section, "title", ""), fields));
        }
    }
    if let (Some(data), Some(fields)) = (&data, &resolved) {
        let declared_title = fields
            .iter()
            .find(|field| matches!(get_string(field, "variant").as_deref(), Some("h3" | "h4")));
        let title_value = declared_title
            .and_then(|field| data.get_path(&name_of(field)))
            .filter(|value| match value {
                Value::Null => false,
                Value::String(text) => !text.is_empty(),
                Value::Bool(flag) => *flag,
                Value::Int(number) => *number != 0,
                Value::Number(number) => *number != 0.0,
                _ => true,
            });
        let title_derived = title.is_none() && title_value.is_some();
        if title_derived {
            title = title_value.map(js_string);
        }
        let tab_claimed: Vec<String> = if look == "workspace" {
            get_array(props, "tabs")
                .iter()
                .flat_map(|tab| get_array(tab, "fields"))
                .filter_map(|name| name.as_str().map(str::to_string))
                .collect()
        } else {
            Vec::new()
        };
        let slotted = |slot: &str| -> Vec<&Value> {
            fields
                .iter()
                .filter(|field| {
                    slot_of(field) == slot
                        && !(title_derived && declared_title == Some(*field))
                        && !tab_claimed.contains(&name_of(field))
                })
                .collect()
        };
        let to_detail = |group: Vec<&Value>| -> Vec<Value> {
            group
                .into_iter()
                .filter_map(|field| {
                    let value = data
                        .get_path(&name_of(field))
                        .filter(|value| !value.is_null())?;
                    Some(detail_field(
                        label_for(&name_of(field)),
                        rich(field, Some(value)),
                    ))
                })
                .collect()
        };
        if !has_declared {
            let overview = to_detail(slotted("other"));
            if !overview.is_empty() {
                sections.push(("Overview".to_string(), overview));
            }
            let timeline = to_detail(slotted("date"));
            if !timeline.is_empty() {
                sections.push(("Timeline".to_string(), timeline));
            }
        }
        key_figures = slotted("progress")
            .into_iter()
            .chain(slotted("metric"))
            .filter(|field| {
                data.get_path(&name_of(field))
                    .is_some_and(|value| !value.is_null() && value.as_str() != Some(""))
            })
            .cloned()
            .collect();
        prose_fields = to_detail(slotted("body"));
    }
    let sections: Vec<(String, Vec<Value>)> = sections
        .into_iter()
        .filter(|(_, fields)| !fields.is_empty())
        .collect();
    let show_headings = sections.len() > 1;

    let row = data.clone().unwrap_or_else(|| object_value::<&str>([]));
    let actions: Vec<Value> = get_array(props, "actions")
        .into_iter()
        .filter(|action| row_action_is_shown(action, &row, None))
        .collect();
    let close_event = get_string(props, "closeEvent").filter(|event| !event.is_empty());
    let close_index = close_event.as_ref().and_then(|event| {
        actions
            .iter()
            .position(|action| get_string(action, "event").as_ref() == Some(event))
    });
    let others: Vec<&Value> = actions
        .iter()
        .enumerate()
        .filter(|(index, _)| Some(*index) != close_index)
        .map(|(_, action)| action)
        .collect();

    let overflow_payloads: Vec<(String, Value)> = others
        .iter()
        .skip(max_inline)
        .filter_map(|action| {
            let event = get_string(action, "event")?;
            Some((event, logged_row_payload(action, &row)?))
        })
        .collect();
    let inner_sink = input.view.sink;
    let commit_target = title_commit.clone();
    let commit_id = record_id.clone().unwrap_or_default();
    let current_title = title.clone().unwrap_or_default();
    let seed = title.clone().unwrap_or_default();
    let adapter = Callback::new(
        move |(event, payload): (String, Value)| match event.as_str() {
            TITLE_EDIT => title_draft.set(Some(seed.clone())),
            TITLE_DRAFT => {
                if let Some(next) = payload.get("value").and_then(Value::as_str) {
                    title_draft.set(Some(next.to_string()));
                }
            }
            TITLE_COMMIT => {
                let Some(target) = &commit_target else { return };
                let draft = payload
                    .get("value")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| title_draft.peek().clone())
                    .unwrap_or_default();
                let next = draft.trim().to_string();
                title_draft.set(None);
                if next.is_empty() || next == current_title {
                    return;
                }
                if let Some(sink) = inner_sink {
                    sink.call((
                        target.clone(),
                        object_value([
                            ("title", Value::string(next)),
                            ("id", Value::string(commit_id.clone())),
                        ]),
                    ));
                }
            }
            _ => {
                let overflow = payload.get("itemId").is_some();
                let payload = overflow
                    .then(|| {
                        overflow_payloads
                            .iter()
                            .find(|(name, _)| *name == event)
                            .map(|(_, p)| p.clone())
                    })
                    .flatten()
                    .unwrap_or(payload);
                if let Some(sink) = inner_sink {
                    sink.call((event, payload));
                }
            }
        },
    );
    let mut owned = input.view.clone();
    owned.sink = Some(adapter);
    let view = owned.as_view();
    let r = |spec: &Value| registry.render_spec(spec, None, &view);

    if get_bool_or(props, "isLoading", false) {
        let spec = pattern_spec(
            "skeleton",
            [
                (
                    "spec",
                    props
                        .get("skeleton")
                        .cloned()
                        .unwrap_or_else(|| Value::string("detail")),
                ),
                ("className", Value::string(class_name.as_str())),
            ],
        );
        return r(&spec);
    }
    if let Some(error) = props.get("error").filter(|error| !error.is_null()) {
        let message = get_string(error, "message")
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| "Failed to load".to_string());
        let spec = pattern_spec(
            "error-state",
            [
                ("title", Value::string("Loading data")),
                ("message", Value::string(message)),
                ("retryEvent", Value::string("RETRY")),
                ("className", Value::string(class_name.as_str())),
            ],
        );
        return r(&spec);
    }
    if data.is_none() && resolved.as_ref().is_some_and(|fields| !fields.is_empty()) {
        let spec = pattern_spec(
            "empty-state",
            [
                ("title", Value::string(crate::i18n::t("error.notFound"))),
                (
                    "description",
                    Value::string(crate::i18n::t("display.itemNotFound")),
                ),
                ("className", Value::string(class_name.as_str())),
            ],
        );
        return r(&spec);
    }

    let action_button = |action: &Value, default_variant: &str| -> Value {
        let mut fields = vec![
            ("label", Value::string(get_string_or(action, "label", ""))),
            (
                "variant",
                Value::string(get_string_or(action, "variant", default_variant)),
            ),
            ("size", Value::string("sm")),
        ];
        if let Some(icon) = action.get("icon") {
            fields.push(("icon", icon.clone()));
        }
        if action.get("navigatesTo").is_some() {
            if let Some(route) = logged_row_route(action, &row) {
                fields.push(("href", Value::string(route)));
            }
        } else if let Some(event) = get_string(action, "event") {
            fields.push(("action", Value::string(event)));
        }
        if data.is_some() {
            if let Some(payload) = logged_row_payload(action, &row) {
                fields.push(("actionPayload", payload));
            }
        }
        pattern_spec("button", fields)
    };

    let title_shown = title.clone().unwrap_or_else(|| "Details".to_string());
    let title_node = if title_commit.is_some() {
        match title_draft() {
            Some(draft) => pattern_spec(
                "input",
                [
                    ("value", Value::string(draft)),
                    ("aria-label", Value::string("Title")),
                    ("className", Value::string("h-auto py-1 heading-voice text-3xl")),
                    ("onChange", Value::string(TITLE_DRAFT)),
                    ("action", Value::string(TITLE_COMMIT)),
                ],
            ),
            None => pattern_spec(
                "box",
                [
                    ("role", Value::string("button")),
                    ("tabIndex", Value::Int(0)),
                    (
                        "className",
                        Value::string("cursor-text rounded-interactive px-1 -mx-1 transition-colors hover:bg-muted/40"),
                    ),
                    ("action", Value::string(TITLE_EDIT)),
                    ("children", arr(vec![text(&title_shown, "h2", "primary", "")])),
                ],
            ),
        }
    } else {
        text(&title_shown, "h2", "primary", "")
    };

    let mut badges: Vec<Value> = Vec::new();
    if let (Some(data), Some(fields)) = (&data, &resolved) {
        for field in fields.iter().filter(|field| slot_of(field) == "badge") {
            let Some(value) = data
                .get_path(&name_of(field))
                .filter(|v| !v.is_null() && v.as_str() != Some(""))
            else {
                continue;
            };
            let value = js_string(value);
            badges.push(badge(
                &value_label(&value, field),
                &badge_variant(&value, field),
            ));
        }
    }
    if let Some(status) = props.get("status").filter(|status| !status.is_null()) {
        badges.push(badge(
            &get_string_or(status, "label", ""),
            &get_string_or(status, "variant", "default"),
        ));
    }

    let back = props
        .get("backAction")
        .filter(|back| !back.is_null())
        .map(|back| {
            let mut fields = vec![
                ("label", Value::string(get_string_or(back, "label", ""))),
                (
                    "variant",
                    Value::string(get_string_or(back, "variant", "ghost")),
                ),
                ("size", Value::string("sm")),
                (
                    "icon",
                    back.get("icon")
                        .cloned()
                        .unwrap_or_else(|| Value::string("arrow-left")),
                ),
            ];
            if back.get("navigatesTo").is_some() {
                if let Some(route) = logged_row_route(back, &row) {
                    fields.push(("href", Value::string(route)));
                }
            } else if let Some(event) = get_string(back, "event") {
                fields.push(("action", Value::string(event)));
            }
            if data.is_some() {
                if let Some(payload) = logged_row_payload(back, &row) {
                    fields.push(("actionPayload", payload));
                }
            }
            pattern_spec("button", fields)
        });

    let avatar_field = get_string(props, "avatarField").filter(|field| !field.is_empty());
    let avatar_node: Option<Element> =
        if let Some(avatar) = props.get("avatar").filter(|avatar| !avatar.is_null()) {
            Some(view.render_node(registry, avatar))
        } else if avatar_field.is_some() || look == "profile" {
            let src = avatar_field
                .as_ref()
                .and_then(|field| data.as_ref().and_then(|data| data.get_path(field)))
                .and_then(Value::as_str)
                .filter(|src| !src.is_empty());
            let mut fields = vec![
                ("name", Value::string(title.clone().unwrap_or_default())),
                ("size", Value::string("xl")),
            ];
            if let Some(src) = src {
                fields.push(("src", Value::string(src)));
            }
            Some(r(&pattern_spec("avatar", fields)))
        } else {
            None
        };

    let inline_buttons: Vec<Value> = others
        .iter()
        .take(max_inline)
        .map(|action| action_button(action, "secondary"))
        .collect();
    let overflow_items: Vec<Value> = others
        .iter()
        .enumerate()
        .skip(max_inline)
        .map(|(index, action)| {
            let mut fields = vec![
                ("id", Value::string(index.to_string())),
                ("label", Value::string(get_string_or(action, "label", ""))),
                (
                    "variant",
                    Value::string(
                        if get_string(action, "variant").as_deref() == Some("danger") {
                            "danger"
                        } else {
                            "default"
                        },
                    ),
                ),
            ];
            if action.get("navigatesTo").is_some() {
                match logged_row_route(action, &row) {
                    Some(route) => fields.push(("href", Value::string(route))),
                    None => fields.push(("disabled", Value::Bool(true))),
                }
            } else if let Some(event) = get_string(action, "event") {
                fields.push(("event", Value::string(event)));
            }
            if let Some(icon) = action.get("icon") {
                fields.push(("icon", icon.clone()));
            }
            object_value(fields)
        })
        .collect();
    let overflow_menu = (!overflow_items.is_empty()).then(|| {
        pattern_spec(
            "menu",
            [
                ("position", Value::string("bottom-end")),
                ("items", arr(overflow_items)),
                (
                    "trigger",
                    pattern_spec(
                        "button",
                        [
                            ("label", Value::string("")),
                            ("variant", Value::string("ghost")),
                            ("size", Value::string("sm")),
                            ("icon", Value::string("more-horizontal")),
                            (
                                "aria-label",
                                Value::string(crate::i18n::t("common.actions")),
                            ),
                        ],
                    ),
                ),
            ],
        )
    });
    let close_button = close_index.map(|index| {
        let action = &actions[index];
        let mut fields = vec![
            ("label", Value::string("")),
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("icon", Value::string("x")),
            ("aria-label", Value::string("Close panel")),
        ];
        if let Some(event) = get_string(action, "event") {
            fields.push(("action", Value::string(event)));
        }
        if data.is_some() {
            if let Some(payload) = logged_row_payload(action, &row) {
                fields.push(("actionPayload", payload));
            }
        }
        pattern_spec("button", fields)
    });

    let null = Value::Null;
    let node = |value: &Value, class: &str| -> Element {
        let empty = matches!(value, Value::Null) || value.as_str() == Some("");
        match value {
            Value::Object(_) | Value::Array(_) => {
                let wrapper = format!("text-base font-normal text-foreground {class}");
                rsx! { div { class: "{wrapper}", {view.render_node(registry, value)} } }
            }
            _ => {
                let shown = if empty {
                    "—".to_string()
                } else {
                    js_string(value)
                };
                r(&text(&shown, "body", "primary", class))
            }
        }
    };

    let header_class = stack_class(
        "horizontal",
        "md",
        "start",
        "between",
        true,
        if slide_over {
            "sticky top-0 z-10 -mx-6 -mt-6 bg-card px-6 pt-6 pb-3 border-b border-border"
        } else {
            ""
        },
    );
    let identity_class = stack_class(
        "horizontal",
        "sm",
        "start",
        "stretch",
        false,
        "min-w-0 flex-1",
    );
    let name_class = stack_class("vertical", "xs", "stretch", "start", false, "min-w-0");
    let title_row_class = stack_class("horizontal", "sm", "center", "start", true, "");
    let actions_class = stack_class("horizontal", "xs", "center", "end", false, "shrink-0");
    let subtitle = get_string(props, "subtitle").filter(|subtitle| !subtitle.is_empty());

    let header = rsx! {
        div { class: "{header_class}",
            div { class: "{identity_class}",
                if let Some(back) = &back { {r(back)} }
                if let Some(avatar) = avatar_node { {avatar} }
                div { class: "{name_class}",
                    div { class: "{title_row_class}",
                        {r(&title_node)}
                        for item in badges.iter() { {r(item)} }
                    }
                    if let Some(subtitle) = &subtitle { {r(&text(subtitle, "body", "secondary", ""))} }
                }
            }
            if show_actions {
                div { class: "{actions_class}",
                    for button in inline_buttons.iter() { {r(button)} }
                    if let Some(menu) = &overflow_menu { {r(menu)} }
                    if let Some(close) = &close_button { {r(close)} }
                }
            }
        }
    };

    let figures = (!key_figures.is_empty() && data.is_some()).then(|| {
        let data = data.clone().unwrap_or_else(|| object_value::<&str>([]));
        let list_class = stack_class("horizontal", "md", "stretch", "start", true, "");
        let card_class = stack_class(
            "vertical",
            "xs",
            "stretch",
            "start",
            false,
            "min-w-[10rem] max-w-[18rem] flex-1 rounded-container border border-border bg-muted/30 p-3",
        );
        rsx! {
            div { class: "{list_class}",
                for field in key_figures.iter() {
                    {
                        let value = data.get_path(&name_of(field));
                        let shown = value.map(|value| format_field_value(value, get_string(field, "format").as_deref())).unwrap_or_default();
                        let progress = slot_of(field) == "progress";
                        let number = value.filter(|value| value.is_numeric()).and_then(Value::as_number);
                        rsx! {
                            div { class: "{card_class}",
                                {r(&weighted(&label_for(&name_of(field)), "caption", "muted", "medium", ""))}
                                {r(&text(&shown, "h4", "primary", "tabular-nums break-words"))}
                                if let (true, Some(number)) = (progress, number) {
                                    {r(&pattern_spec("progress-bar", [("value", Value::Number(number))]))}
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    let sections_node = (!sections.is_empty()).then(|| {
        let outer = stack_class("vertical", "lg", "stretch", "start", false, "");
        let inner = stack_class("vertical", "sm", "stretch", "start", false, "");
        let cell = stack_class("horizontal", "sm", "start", "start", false, "min-w-0");
        let body = stack_class("vertical", "none", "stretch", "start", false, "min-w-0 flex-1");
        rsx! {
            div { class: "{outer}",
                for (heading, fields) in sections.iter() {
                    div { class: "{inner}",
                        if show_headings { {r(&text(heading, "h5", "secondary", ""))} }
                        div { class: "grid gap-4", style: "grid-template-columns:repeat(auto-fit, minmax(max(min(192px, 100%), calc((100% - 2 * 1rem) / 3)), 1fr))",
                            for field in fields.iter() {
                                div { class: "{cell}",
                                    if let Some(icon) = field.get("icon").filter(|icon| !icon.is_null()) {
                                        {r(&pattern_spec("icon", [("name", icon.clone()), ("size", Value::string("md")), ("className", Value::string("text-muted-foreground mt-1"))]))}
                                    }
                                    div { class: "{body}",
                                        {r(&weighted(&get_string_or(field, "label", ""), "caption", "muted", "medium", ""))}
                                        {node(field.get("value").unwrap_or(&null), "break-words")}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    let prose_node = (!prose_fields.is_empty()).then(|| {
        let outer = stack_class("vertical", "md", "stretch", "start", false, "");
        let inner = stack_class("vertical", "xs", "stretch", "start", false, "");
        rsx! {
            div { class: "{outer}",
                for field in prose_fields.iter() {
                    div { class: "{inner}",
                        {r(&weighted(&get_string_or(field, "label", ""), "caption", "muted", "medium", ""))}
                        {node(field.get("value").unwrap_or(&null), "whitespace-pre-line break-words")}
                    }
                }
            }
        }
    });

    let footer_node = props
        .get("footer")
        .filter(|footer| !footer.is_null())
        .map(|footer| {
            rsx! {
                {r(&pattern_spec("divider", Vec::<(&str, Value)>::new()))}
                {view.render_node(registry, footer)}
            }
        });

    let totals_node = (|| {
        let totals: Vec<Value> = get_array(props, "totals")
            .iter()
            .filter_map(resolve_field)
            .collect();
        let data = data.as_ref()?;
        if totals.is_empty() {
            return None;
        }
        let outer = stack_class(
            "vertical",
            "xs",
            "end",
            "start",
            false,
            "ms-auto min-w-[16rem] border-t border-border pt-3",
        );
        let line = stack_class("horizontal", "lg", "stretch", "between", false, "w-full");
        let count = totals.len();
        Some(rsx! {
            div { class: "{outer}",
                for (index, field) in totals.iter().enumerate() {
                    {
                        let last = index + 1 == count;
                        let label = get_string(field, "label").unwrap_or_else(|| name_of(field));
                        let shown = match data.get_path(&name_of(field)).filter(|value| !value.is_null()) {
                            Some(value) => format_field_value(value, get_string(field, "format").as_deref()),
                            None => "—".to_string(),
                        };
                        let label_spec = if last {
                            weighted(&label, "body", "primary", "semibold", "")
                        } else {
                            text(&label, "small", "secondary", "")
                        };
                        rsx! {
                            div { class: "{line}",
                                {r(&label_spec)}
                                {r(&text(&shown, if last { "h4" } else { "body" }, "primary", "tabular-nums"))}
                            }
                        }
                    }
                }
            }
        })
    })();

    let look_class = stack_class(
        "vertical",
        "lg",
        "stretch",
        "start",
        false,
        if look == "ledger" {
            "print:shadow-none"
        } else {
            ""
        },
    );
    let cover = (look == "profile")
        .then(|| {
            get_string(props, "coverField")
                .and_then(|field| {
                    data.as_ref()
                        .and_then(|data| data.get_path(&field))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .filter(|cover| !cover.is_empty())
        })
        .flatten();
    let content_pad = if slide_over { "" } else { "p-6" };
    let content = rsx! {
        div { class: "{content_pad}",
            div { class: "{look_class}",
                if let Some(cover) = &cover {
                    div { class: "-mx-6 -mt-6 overflow-hidden",
                        {r(&pattern_spec("image", [("src", Value::string(cover.as_str())), ("alt", Value::string("")), ("aspect", Value::string("21/9")), ("fit", Value::string("cover")), ("rounded", Value::string("none"))]))}
                    }
                }
                {header}
                if look == "ledger" {
                    if let Some(sections) = sections_node { {sections} }
                    if let Some(totals) = totals_node { {totals} }
                    if let Some(figures) = figures { {figures} }
                } else {
                    if let Some(figures) = figures { {figures} }
                    if let Some(sections) = sections_node { {sections} }
                }
                if let Some(prose) = prose_node { {prose} }
                if let Some(footer) = footer_node { {footer} }
            }
        }
    };

    if !slide_over {
        let card_class = format!("{surface} rounded-container");
        return rsx! {
            div { class: "{class_name}", ..crate::registry::shared_root_attrs(props),
                div { class: "{card_class}", {content} }
            }
        };
    }

    let panel_class = format!(
        "fixed inset-y-0 right-0 z-50 w-full max-w-2xl overflow-y-auto bg-card surface-material shadow-elevation-dialog p-6 {class_name}"
    );
    let aria = get_string(props, "aria-label")
        .unwrap_or_else(|| title.clone().unwrap_or_else(|| "Details".to_string()));
    let navigate = view.navigate;
    let dismiss_action = close_index.map(|index| actions[index].clone());
    let dismiss_row = row.clone();
    let dismiss_sink = input.view.sink;
    rsx! {
        div { class: "fixed inset-0 z-40 bg-scrim", aria_hidden: "true",
            onclick: move |_| {
                let Some(action) = &dismiss_action else { return };
                if action.get("navigatesTo").is_some() {
                    if let (Some(navigate), Some(route)) = (navigate, logged_row_route(action, &dismiss_row)) {
                        navigate.call(route);
                    }
                } else if let (Some(sink), Some(event), Some(payload)) =
                    (dismiss_sink, get_string(action, "event"), logged_row_payload(action, &dismiss_row))
                {
                    sink.call((event, payload));
                }
            },
        }
        div { class: "{panel_class}", role: "dialog", aria_modal: "true", aria_label: "{aria}",
            {content}
        }
    }
}

impl DioxusPatternRenderer for DetailPanelRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let entity = props
            .get("entity")
            .filter(|entity| !entity.is_null())
            .cloned()
            .or_else(|| entity.cloned());
        rsx! { Panel { props: props.clone(), entity: entity, view: view.to_owned(), registry: registry.clone() } }
    }
}
