//! Native Dioxus body for the `timeline` pattern, ported from web `Timeline.tsx`.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{cn, object_value, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct TimelineRenderer;

impl TimelineRenderer {
    const NAME: &str = "timeline";
}

struct Item {
    id: String,
    title: String,
    description: Option<String>,
    date: Option<String>,
    status: Option<String>,
    tags: Vec<String>,
}

fn node(name: &str, mut fields: Vec<(&str, Value)>, children: Vec<Value>) -> Value {
    if !children.is_empty() {
        fields.push(("children", Value::Array(children.into())));
    }
    pattern_spec(name, fields)
}

fn typo(
    content: String,
    variant: &str,
    color: Option<&str>,
    weight: Option<&str>,
    class: Option<&str>,
) -> Value {
    let mut fields = vec![
        ("content", Value::string(content)),
        ("variant", Value::string(variant)),
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
    node("typography", fields, vec![])
}

fn stack(name: &str, fields: Vec<(&str, Value)>, children: Vec<Value>) -> Value {
    node(name, fields, children)
}

fn text_of(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.to_string()),
        Some(Value::Int(n)) => Some(n.to_string()),
        Some(Value::Number(n)) => Some(format!("{n}")),
        Some(Value::Bool(b)) => Some(b.to_string()),
        _ => None,
    }
}

fn truthy_text(value: Option<&Value>) -> Option<String> {
    text_of(value).filter(|t| !t.is_empty() && t != "0" && t != "false")
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn format_date(raw: &str) -> String {
    let parsed = raw.get(..10).and_then(|date| {
        if raw.len() > 10 && !raw[10..].starts_with('T') {
            return None;
        }
        let mut parts = date.split('-');
        let y: i64 = parts.next()?.parse().ok()?;
        let m: usize = parts.next()?.parse().ok()?;
        let d: u32 = parts.next()?.parse().ok()?;
        (1..=12)
            .contains(&m)
            .then(|| format!("{} {d}, {y}", MONTHS[m - 1]))
    });
    parsed.unwrap_or_else(|| raw.to_string())
}

fn look_class(look: &str) -> &'static str {
    match look {
        "vertical-compact" => "gap-1 [&>*]:py-1",
        "horizontal" => "flex-row [&>*]:flex-row [&>*]:items-center",
        "swimlane" => "flex-row gap-6 [&>*]:flex-row [&>*]:items-center",
        _ => "",
    }
}

fn status_style(status: &str) -> (&'static str, &'static str, &'static str) {
    match status {
        "complete" => ("success", "bg-success", "circle-check"),
        "active" => ("primary", "bg-primary", "clock"),
        "error" => ("error", "bg-error", "circle-alert"),
        _ => ("muted", "bg-border", "circle"),
    }
}

fn parse_items(props: &Value, view: &DioxusView<'_>) -> Vec<Item> {
    let provided = get_array(props, "items");
    if !provided.is_empty() {
        return provided
            .iter()
            .map(|row| Item {
                id: text_of(row.get("id")).unwrap_or_default(),
                title: text_of(row.get("title")).unwrap_or_default(),
                description: text_of(row.get("description")).filter(|t| !t.is_empty()),
                date: text_of(row.get("date")).filter(|t| !t.is_empty()),
                status: text_of(row.get("status")).filter(|t| !t.is_empty()),
                tags: get_array(row, "tags")
                    .iter()
                    .filter_map(|t| text_of(Some(t)))
                    .collect(),
            })
            .collect();
    }
    let title_field = get_string(props, "titleField");
    let description_field = get_string(props, "descriptionField");
    let date_field = get_string(props, "dateField");
    let status_field = get_string(props, "statusField");
    get_entity_rows_from(props, view.entities)
        .iter()
        .enumerate()
        .map(|(idx, record)| Item {
            id: text_of(record.get("id")).unwrap_or_else(|| idx.to_string()),
            title: title_field
                .as_deref()
                .map(|f| text_of(record.get(f)).unwrap_or_default())
                .unwrap_or_default(),
            description: description_field
                .as_deref()
                .and_then(|f| truthy_text(record.get(f))),
            date: date_field
                .as_deref()
                .and_then(|f| truthy_text(record.get(f))),
            status: Some(
                status_field
                    .as_deref()
                    .and_then(|f| truthy_text(record.get(f)))
                    .unwrap_or_else(|| "pending".to_string()),
            ),
            tags: vec![],
        })
        .collect()
}

fn item_row(item: &Item) -> Value {
    let mut fields = vec![
        ("id", Value::string(item.id.clone())),
        ("title", Value::string(item.title.clone())),
    ];
    if let Some(description) = &item.description {
        fields.push(("description", Value::string(description.clone())));
    }
    if let Some(date) = &item.date {
        fields.push(("date", Value::string(date.clone())));
    }
    if let Some(status) = &item.status {
        fields.push(("status", Value::string(status.clone())));
    }
    if !item.tags.is_empty() {
        fields.push((
            "tags",
            Value::Array(
                item.tags
                    .iter()
                    .map(|t| Value::string(t.clone()))
                    .collect::<Vec<_>>()
                    .into(),
            ),
        ));
    }
    object_value(fields)
}

impl DioxusPatternRenderer for TimelineRenderer {
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
        let class_name = get_string_or(props, "className", "");
        let wrap = |inner: Value| node("box", vec![], vec![inner]);

        if get_bool_or(props, "isLoading", false) {
            let skeleton = props
                .get("skeleton")
                .cloned()
                .unwrap_or_else(|| Value::string("list"));
            let spec = wrap(node(
                "skeleton",
                vec![("spec", skeleton), ("className", Value::string(class_name))],
                vec![],
            ));
            return registry.render_spec(&spec, None, view);
        }
        if let Some(message) = props
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
        {
            let spec = wrap(node(
                "error-state",
                vec![
                    (
                        "title",
                        Value::string(crate::i18n::t("display.timelineError")),
                    ),
                    ("message", Value::string(message)),
                    ("className", Value::string(class_name)),
                ],
                vec![],
            ));
            return registry.render_spec(&spec, None, view);
        }

        let items = parse_items(props, view);
        if items.is_empty() {
            let spec = wrap(node(
                "empty-state",
                vec![
                    ("title", Value::string(crate::i18n::t("display.noEvents"))),
                    (
                        "description",
                        Value::string(crate::i18n::t("display.noTimelineEvents")),
                    ),
                    ("className", Value::string(class_name)),
                ],
                vec![],
            ));
            return registry.render_spec(&spec, None, view);
        }

        let look = get_string_or(props, "look", "vertical-spacious");
        let actions = get_array(props, "itemActions");
        let last_index = items.len() - 1;

        let rows: Vec<Value> = items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let is_last = idx == last_index;
                let (color, line_bg, icon) =
                    status_style(item.status.as_deref().unwrap_or("pending"));
                let mut track = vec![node(
                    "icon",
                    vec![
                        ("name", Value::string(icon)),
                        ("size", Value::string("sm")),
                        ("color", Value::string(color)),
                        ("className", Value::string("z-10 bg-card")),
                    ],
                    vec![],
                )];
                if !is_last {
                    track.push(node(
                        "box",
                        vec![(
                            "className",
                            Value::string(format!(
                                "w-0.5 flex-1 min-h-[24px] {line_bg} opacity-40"
                            )),
                        )],
                        vec![],
                    ));
                }

                let mut heading = vec![typo(
                    item.title.clone(),
                    "body",
                    None,
                    Some("semibold"),
                    None,
                )];
                if let Some(date) = &item.date {
                    heading.push(typo(
                        format_date(date),
                        "caption",
                        Some("secondary"),
                        None,
                        Some("flex-shrink-0"),
                    ));
                }
                let mut content = vec![stack(
                    "hstack",
                    vec![
                        ("justify", Value::string("between")),
                        ("align", Value::string("start")),
                        ("wrap", Value::Bool(true)),
                    ],
                    heading,
                )];
                if let Some(description) = &item.description {
                    content.push(typo(
                        description.clone(),
                        "small",
                        Some("secondary"),
                        None,
                        None,
                    ));
                }
                if !item.tags.is_empty() {
                    let tags = item
                        .tags
                        .iter()
                        .map(|tag| {
                            node(
                                "badge",
                                vec![
                                    ("variant", Value::string("default")),
                                    ("label", Value::string(tag.clone())),
                                ],
                                vec![],
                            )
                        })
                        .collect();
                    content.push(stack(
                        "hstack",
                        vec![("gap", Value::string("xs")), ("wrap", Value::Bool(true))],
                        tags,
                    ));
                }
                if !actions.is_empty() {
                    let buttons = actions
                        .iter()
                        .map(|action| {
                            let mut fields = vec![
                                ("variant", Value::string("ghost")),
                                ("size", Value::string("sm")),
                                ("label", Value::string(get_string_or(action, "label", ""))),
                                ("actionPayload", object_value([("row", item_row(item))])),
                            ];
                            if let Some(event) = get_string(action, "event") {
                                fields.push(("action", Value::string(event)));
                            }
                            node("button", fields, vec![])
                        })
                        .collect();
                    content.push(stack(
                        "hstack",
                        vec![
                            ("gap", Value::string("xs")),
                            ("className", Value::string("mt-1")),
                        ],
                        buttons,
                    ));
                }

                let content_class = if is_last {
                    "flex-1 min-w-0"
                } else {
                    "flex-1 min-w-0 pb-6"
                };
                stack(
                    "hstack",
                    vec![
                        ("gap", Value::string("md")),
                        ("align", Value::string("start")),
                        ("className", Value::string("relative")),
                        ("data-entity-id", Value::string(item.id.clone())),
                    ],
                    vec![
                        stack(
                            "vstack",
                            vec![
                                ("align", Value::string("center")),
                                ("className", Value::string("flex-shrink-0 relative")),
                                ("style", object_value([("width", Value::string("24px"))])),
                            ],
                            track,
                        ),
                        stack(
                            "vstack",
                            vec![
                                ("gap", Value::string("xs")),
                                ("className", Value::string(content_class)),
                            ],
                            content,
                        ),
                    ],
                )
            })
            .collect();

        let mut body = Vec::new();
        if let Some(title) = get_string(props, "title").filter(|t| !t.is_empty()) {
            body.push(typo(title, "h5", None, None, None));
        }
        body.push(stack(
            "vstack",
            vec![
                ("gap", Value::string("none")),
                (
                    "className",
                    Value::string(format!("relative {}", look_class(&look))),
                ),
            ],
            rows,
        ));
        let surface_class = if get_string_or(props, "surface", "auto") == "auto" {
            "surface-content"
        } else {
            ""
        };
        let spec = node(
            "box",
            vec![(
                "className",
                Value::string(cn(&[surface_class, "p-6 rounded-container", &class_name])),
            )],
            vec![stack("vstack", vec![("gap", Value::string("md"))], body)],
        );
        registry.render_spec(&spec, None, view)
    }
}
