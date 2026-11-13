//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    format_field_value, js_string, object_value, pattern_spec, relation_label,
    DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `document-details` pattern.
pub struct DocumentDetailsRenderer;

impl DocumentDetailsRenderer {
    const NAME: &str = "document-details";
}

const EDIT: &str = "__document_details_edit:";
const DRAFT: &str = "__document_details_draft:";
const COMMIT: &str = "__document_details_commit:";

#[derive(Props, Clone, PartialEq)]
struct DetailsProps {
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

fn field_name(field: &Value) -> String {
    get_string(field, "name")
        .or_else(|| get_string(field, "key"))
        .unwrap_or_default()
}

fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Int(number)) => *number != 0,
        Some(Value::Number(number)) => *number != 0.0,
        Some(Value::String(text)) => !text.is_empty(),
        Some(_) => true,
    }
}

fn is_empty(raw: Option<&Value>) -> bool {
    match raw {
        None | Some(Value::Null) => true,
        Some(Value::String(text)) => text.is_empty(),
        _ => false,
    }
}

fn kind_of(field: &Value, raw: Option<&Value>) -> String {
    if let Some(kind) = get_string(field, "kind") {
        return kind;
    }
    if get_string(field, "type").as_deref() == Some("image") {
        return "image".into();
    }
    if !get_array(field, "options").is_empty() {
        return "select".into();
    }
    if matches!(raw, Some(Value::Bool(_)))
        || get_string(field, "format").as_deref() == Some("boolean")
    {
        return "boolean".into();
    }
    if matches!(raw, Some(Value::Array(_)))
        || raw.is_some_and(|raw| relation_label(raw, None).is_some())
    {
        return "readonly".into();
    }
    "text".into()
}

fn label_for(labels: Option<&Value>, value: &str) -> String {
    labels
        .and_then(|labels| labels.get(value))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

#[allow(non_snake_case)]
fn Details(input: DetailsProps) -> Element {
    let props = &input.props;
    let registry = &input.registry;
    let mut draft = use_signal(|| None::<(String, String)>);

    let fields: Vec<Value> = get_array(props, "fields")
        .into_iter()
        .filter(|field| !field_name(field).is_empty())
        .collect();
    if fields.is_empty() {
        return rsx! {};
    }
    let entity = input
        .entity
        .clone()
        .unwrap_or_else(|| object_value::<&str>([]));
    let record_id = entity
        .get("id")
        .filter(|id| !id.is_null())
        .map(js_string)
        .unwrap_or_default();
    let commit_event = get_string(props, "metaCommitEvent").filter(|event| !event.is_empty());
    let relation_event = get_string(props, "relationEvent").filter(|event| !event.is_empty());
    let relations = props.get("relationsData").cloned();
    let inner_sink = input.view.sink;

    let seed_entity = entity.clone();
    let commit_target = commit_event.clone();
    let commit_id = record_id.clone();
    let adapter = Callback::new(move |(event, payload): (String, Value)| {
        if let Some(name) = event.strip_prefix(EDIT) {
            let current = seed_entity
                .get_path(name)
                .filter(|value| !value.is_null())
                .map(js_string)
                .unwrap_or_default();
            draft.set(Some((name.to_string(), current)));
        } else if let Some(name) = event.strip_prefix(DRAFT) {
            if let Some(next) = payload.get("value").and_then(Value::as_str) {
                draft.set(Some((name.to_string(), next.to_string())));
            }
        } else if let Some(name) = event.strip_prefix(COMMIT) {
            draft.set(None);
            let current = seed_entity.get_path(name);
            let next = if let Some(checked) = payload.get("checked").and_then(Value::as_bool) {
                if checked == truthy(current) {
                    return;
                }
                Value::Bool(checked)
            } else if let Some(value) = payload.get("value").and_then(Value::as_str) {
                let next = value.trim().to_string();
                if current.and_then(Value::as_str) == Some(next.as_str()) {
                    return;
                }
                Value::string(next)
            } else {
                return;
            };
            if let (Some(sink), Some(target)) = (inner_sink, &commit_target) {
                sink.call((
                    target.clone(),
                    object_value([
                        ("id", Value::string(commit_id.clone())),
                        (
                            "patch",
                            object_value([("id", Value::string(commit_id.clone())), (name, next)]),
                        ),
                    ]),
                ));
            }
        } else if let Some(sink) = inner_sink {
            sink.call((event, payload));
        }
    });
    let mut owned = input.view.clone();
    owned.sink = Some(adapter);
    let view = owned.as_view();

    let chip = |item: &Value, name: &str, labels: Option<&Value>| -> Value {
        let options = relations.as_ref().and_then(|data| data.get(name));
        let label =
            relation_label(item, options).unwrap_or_else(|| label_for(labels, &js_string(item)));
        match &relation_event {
            None => pattern_spec(
                "badge",
                [
                    ("variant", Value::string("default")),
                    ("label", Value::string(label)),
                ],
            ),
            Some(event) => {
                let id = item
                    .get("id")
                    .filter(|id| !id.is_null())
                    .map(js_string)
                    .unwrap_or_else(|| js_string(item));
                pattern_spec(
                    "box",
                    [
                        ("display", Value::string("inline-flex")),
                        ("role", Value::string("button")),
                        ("tabIndex", Value::Int(0)),
                        ("action", Value::string(event.as_str())),
                        (
                            "actionPayload",
                            object_value([
                                ("field", Value::string(name)),
                                ("id", Value::string(id)),
                                ("name", Value::string(label.as_str())),
                            ]),
                        ),
                        (
                            "children",
                            arr(vec![pattern_spec(
                                "badge",
                                [
                                    ("variant", Value::string("default")),
                                    ("label", Value::string(label.as_str())),
                                    (
                                        "className",
                                        Value::string("cursor-pointer transition-colors hover:bg-accent hover:text-accent-foreground"),
                                    ),
                                ],
                            )]),
                        ),
                    ],
                )
            }
        }
    };

    let render_value = |field: &Value| -> Value {
        let name = field_name(field);
        let raw = entity.get_path(&name);
        let kind = kind_of(field, raw);
        let label = get_string(field, "label")
            .or_else(|| get_string(field, "header"))
            .unwrap_or_else(|| name.clone());
        let labels = field.get("labels");
        let format = get_string(field, "format");
        let options = relations.as_ref().and_then(|data| data.get(&name));
        let dash = || text("—", "small", "secondary", "");

        match kind.as_str() {
            "image" => {
                let url = raw.and_then(Value::as_str).unwrap_or("");
                if url.is_empty() {
                    return dash();
                }
                return pattern_spec(
                    "image",
                    [
                        ("src", Value::string(url)),
                        ("alt", Value::string(label.as_str())),
                        ("loading", Value::string("lazy")),
                        ("rounded", Value::string("none")),
                        (
                            "className",
                            Value::string("max-h-32 w-full rounded-container border border-border object-cover"),
                        ),
                    ],
                );
            }
            "boolean" => {
                return pattern_spec(
                    "switch",
                    [
                        ("checked", Value::Bool(truthy(raw))),
                        ("disabled", Value::Bool(commit_event.is_none())),
                        ("aria-label", Value::string(label.as_str())),
                        ("action", Value::string(format!("{COMMIT}{name}"))),
                    ],
                );
            }
            "select" => {
                let choices: Vec<Value> = get_array(field, "options")
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|choice| {
                        object_value([
                            ("value", Value::string(choice)),
                            ("label", Value::string(label_for(labels, choice))),
                        ])
                    })
                    .collect();
                return pattern_spec(
                    "select",
                    [
                        (
                            "value",
                            Value::string(
                                raw.filter(|raw| !raw.is_null())
                                    .map(js_string)
                                    .unwrap_or_default(),
                            ),
                        ),
                        ("disabled", Value::Bool(commit_event.is_none())),
                        ("aria-label", Value::string(label.as_str())),
                        ("className", Value::string("h-8 w-full")),
                        ("options", arr(choices)),
                        ("onChange", Value::string(format!("{COMMIT}{name}"))),
                    ],
                );
            }
            _ => {}
        }

        if kind == "readonly" || commit_event.is_none() {
            if let Some(Value::Array(items)) = raw {
                if items.is_empty() {
                    return dash();
                }
                return pattern_spec(
                    "hstack",
                    [
                        ("gap", Value::string("xs")),
                        ("wrap", Value::Bool(true)),
                        (
                            "children",
                            arr(items.iter().map(|item| chip(item, &name, labels)).collect()),
                        ),
                    ],
                );
            }
            if let Some(raw) =
                raw.filter(|raw| !is_empty(Some(raw)) && relation_label(raw, None).is_some())
            {
                return chip(raw, &name, labels);
            }
            let shown = if is_empty(raw) {
                "—".to_string()
            } else {
                raw.map(|raw| format_field_value(raw, format.as_deref()))
                    .unwrap_or_default()
            };
            return text(&shown, "small", "primary", "break-words");
        }

        if let Some((editing, value)) = draft().filter(|(editing, _)| *editing == name) {
            let _ = editing;
            return pattern_spec(
                "input",
                [
                    ("value", Value::string(value)),
                    ("aria-label", Value::string(label.as_str())),
                    ("className", Value::string("h-8 w-full")),
                    ("onChange", Value::string(format!("{DRAFT}{name}"))),
                    ("action", Value::string(format!("{COMMIT}{name}"))),
                ],
            );
        }
        let empty = is_empty(raw);
        let shown = if empty {
            "—".to_string()
        } else {
            raw.and_then(|raw| relation_label(raw, options))
                .or_else(|| raw.map(|raw| format_field_value(raw, format.as_deref())))
                .unwrap_or_default()
        };
        pattern_spec(
            "box",
            [
                ("role", Value::string("button")),
                ("tabIndex", Value::Int(0)),
                (
                    "className",
                    Value::string(if empty {
                        "cursor-text rounded-interactive px-1 -mx-1 transition-colors hover:bg-muted/40 text-muted-foreground"
                    } else {
                        "cursor-text rounded-interactive px-1 -mx-1 transition-colors hover:bg-muted/40"
                    }),
                ),
                ("action", Value::string(format!("{EDIT}{name}"))),
                (
                    "children",
                    arr(vec![text(&shown, "small", "primary", "break-words")]),
                ),
            ],
        )
    };

    let rows: Vec<Value> = fields
        .iter()
        .map(|field| {
            let name = field_name(field);
            let label = get_string(field, "label")
                .or_else(|| get_string(field, "header"))
                .unwrap_or_else(|| name.clone());
            let mut head = Vec::new();
            if let Some(icon) = field.get("icon").filter(|icon| !icon.is_null()) {
                head.push(pattern_spec(
                    "icon",
                    [
                        ("name", icon.clone()),
                        ("size", Value::string("xs")),
                        ("className", Value::string("text-muted-foreground")),
                    ],
                ));
            }
            head.push(text(&label, "caption", "muted", ""));
            pattern_spec(
                "vstack",
                [
                    ("gap", Value::string("xs")),
                    (
                        "children",
                        arr(vec![
                            pattern_spec(
                                "hstack",
                                [
                                    ("gap", Value::string("xs")),
                                    ("align", Value::string("center")),
                                    ("children", arr(head)),
                                ],
                            ),
                            pattern_spec(
                                "box",
                                [
                                    ("className", Value::string("min-w-0")),
                                    ("children", arr(vec![render_value(field)])),
                                ],
                            ),
                        ]),
                    ),
                ],
            )
        })
        .collect();

    let heading =
        get_string(props, "title").unwrap_or_else(|| crate::i18n::t("documentDetails.title"));
    let card = pattern_spec(
        "card",
        [
            ("variant", Value::string("bordered")),
            (
                "className",
                Value::string(format!("w-full {}", get_string_or(props, "className", ""))),
            ),
            (
                "children",
                arr(vec![pattern_spec(
                    "vstack",
                    [
                        ("gap", Value::string("sm")),
                        ("className", Value::string("p-4")),
                        (
                            "children",
                            arr(vec![
                                text(
                                    &heading,
                                    "caption",
                                    "secondary",
                                    "font-medium uppercase tracking-wide",
                                ),
                                pattern_spec(
                                    "vstack",
                                    [("gap", Value::string("sm")), ("children", arr(rows))],
                                ),
                            ]),
                        ),
                    ],
                )]),
            ),
        ],
    );
    registry.render_spec(&card, None, &view)
}

impl DioxusPatternRenderer for DocumentDetailsRenderer {
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
        rsx! { Details { props: props.clone(), entity: entity, view: view.to_owned(), registry: registry.clone() } }
    }
}
