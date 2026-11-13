//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    object_value, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `document-panel` pattern.
pub struct DocumentPanelRenderer;

impl DocumentPanelRenderer {
    const NAME: &str = "document-panel";
}

const TITLE_EDIT: &str = "__document_panel_title_edit";
const TITLE_DRAFT: &str = "__document_panel_title_draft";
const TITLE_COMMIT: &str = "__document_panel_title_commit";

#[derive(Props, Clone, PartialEq)]
struct PanelProps {
    props: Value,
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

fn id_payload(id: &str) -> Value {
    object_value([("id", Value::string(id))])
}

fn event_of(action: &Value) -> Option<String> {
    get_string(action, "event").filter(|event| !event.is_empty())
}

#[allow(non_snake_case)]
fn Panel(input: PanelProps) -> Element {
    let props = &input.props;
    let registry = &input.registry;
    let mut title_draft = use_signal(|| None::<String>);

    let record_id = get_string_or(props, "id", "");
    let title = get_string(props, "title").unwrap_or_default();
    let subtitle = get_string(props, "subtitle").filter(|value| !value.is_empty());
    let cover = get_string(props, "coverImage").filter(|value| !value.is_empty());
    let icon = get_string(props, "icon").filter(|value| !value.is_empty());
    let value = get_string_or(props, "value", "");
    let placeholder = get_string(props, "placeholder");
    let editing = get_bool_or(props, "editing", false);
    let title_commit = get_string(props, "titleCommitEvent").filter(|event| !event.is_empty());
    let edit_event = get_string(props, "editEvent").filter(|event| !event.is_empty());
    let done_event = get_string(props, "doneEvent").filter(|event| !event.is_empty());
    let edit_label =
        get_string(props, "editLabel").unwrap_or_else(|| crate::i18n::t("documentPanel.edit"));
    let done_label =
        get_string(props, "doneLabel").unwrap_or_else(|| crate::i18n::t("documentPanel.done"));
    let autosave_hint = get_string(props, "autosaveHint")
        .unwrap_or_else(|| crate::i18n::t("documentPanel.autosaveHint"));
    let actions = get_array(props, "actions");
    let inline_cap = get_number(props, "maxInlineActions")
        .map(|cap| cap.max(0.0) as usize)
        .unwrap_or(1)
        .min(1);
    let inline_indices: Vec<usize> = actions
        .iter()
        .enumerate()
        .filter(|(_, action)| get_string(action, "variant").as_deref() == Some("primary"))
        .map(|(index, _)| index)
        .take(inline_cap)
        .collect();
    let menu_events: Vec<String> = actions
        .iter()
        .enumerate()
        .filter(|(index, _)| !inline_indices.contains(index))
        .filter_map(|(_, action)| event_of(action))
        .collect();

    let inner_sink = input.view.sink;
    let commit_event = title_commit.clone();
    let commit_id = record_id.clone();
    let commit_current = title.clone();
    let edit_seed = title.clone();
    let action_title = title.clone();
    let action_id = record_id.clone();
    let adapter = Callback::new(
        move |(event, payload): (String, Value)| match event.as_str() {
            TITLE_EDIT => title_draft.set(Some(edit_seed.clone())),
            TITLE_DRAFT => {
                if let Some(next) = payload.get("value").and_then(Value::as_str) {
                    title_draft.set(Some(next.to_string()));
                }
            }
            TITLE_COMMIT => {
                let Some(commit) = &commit_event else { return };
                let draft = payload
                    .get("value")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| title_draft.peek().clone())
                    .unwrap_or_default();
                let next = draft.trim().to_string();
                title_draft.set(None);
                if next.is_empty() || next == commit_current {
                    return;
                }
                if let Some(sink) = inner_sink {
                    sink.call((
                        commit.clone(),
                        object_value([
                            ("title", Value::string(next)),
                            ("id", Value::string(commit_id.clone())),
                        ]),
                    ));
                }
            }
            _ => {
                let from_menu = payload.get("itemId").is_some() && menu_events.contains(&event);
                let payload = if from_menu {
                    object_value([
                        ("id", Value::string(action_id.clone())),
                        ("title", Value::string(action_title.clone())),
                    ])
                } else {
                    payload
                };
                if let Some(sink) = inner_sink {
                    sink.call((event, payload));
                }
            }
        },
    );
    let mut owned = input.view.clone();
    owned.sink = Some(adapter);
    let view = owned.as_view();

    let title_shown = if title.is_empty() {
        crate::i18n::t("documentPanel.untitled")
    } else {
        title.clone()
    };
    let title_node = if title_commit.is_some() {
        match title_draft() {
            Some(draft) => pattern_spec(
                "input",
                [
                    ("value", Value::string(draft)),
                    ("aria-label", Value::string(crate::i18n::t("common.title"))),
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
                    (
                        "children",
                        arr(vec![text(
                            &title_shown,
                            "h2",
                            "primary",
                            if title.is_empty() { "text-muted-foreground" } else { "" },
                        )]),
                    ),
                ],
            ),
        }
    } else {
        text(&title_shown, "h2", "primary", "")
    };

    let mut identity = Vec::new();
    if let Some(icon) = &icon {
        identity.push(pattern_spec(
            "box",
            [
                ("className", Value::string("text-muted-foreground")),
                (
                    "children",
                    arr(vec![pattern_spec(
                        "icon",
                        [
                            ("name", Value::string(icon.as_str())),
                            ("size", Value::string("lg")),
                        ],
                    )]),
                ),
            ],
        ));
    }
    identity.push(title_node);
    if let Some(subtitle) = &subtitle {
        identity.push(text(subtitle, "small", "secondary", ""));
    }

    let mut chrome = Vec::new();
    if editing {
        chrome.push(text(&autosave_hint, "caption", "muted", "hidden sm:block"));
        let mut done = vec![
            ("label", Value::string(done_label.as_str())),
            ("variant", Value::string("primary")),
            ("size", Value::string("sm")),
        ];
        if let Some(event) = &done_event {
            done.push(("action", Value::string(event.as_str())));
            done.push(("actionPayload", id_payload(&record_id)));
        }
        chrome.push(pattern_spec("button", done));
    } else {
        if let Some(event) = &edit_event {
            chrome.push(pattern_spec(
                "button",
                [
                    ("label", Value::string(edit_label.as_str())),
                    ("variant", Value::string("ghost")),
                    ("size", Value::string("sm")),
                    ("icon", Value::string("edit")),
                    ("action", Value::string(event.as_str())),
                    ("actionPayload", id_payload(&record_id)),
                ],
            ));
        }
        for index in &inline_indices {
            let action = &actions[*index];
            let mut fields = vec![
                ("label", Value::string(get_string_or(action, "label", ""))),
                ("variant", Value::string("primary")),
                ("size", Value::string("sm")),
            ];
            if let Some(icon) = action.get("icon") {
                fields.push(("icon", icon.clone()));
            }
            if let Some(event) = event_of(action) {
                fields.push(("action", Value::string(event)));
                fields.push((
                    "actionPayload",
                    object_value([
                        ("id", Value::string(record_id.as_str())),
                        ("title", Value::string(title.as_str())),
                    ]),
                ));
            }
            chrome.push(pattern_spec("button", fields));
        }
        let items: Vec<Value> = actions
            .iter()
            .enumerate()
            .filter(|(index, _)| !inline_indices.contains(index))
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
                if let Some(icon) = action.get("icon") {
                    fields.push(("icon", icon.clone()));
                }
                if let Some(event) = event_of(action) {
                    fields.push(("event", Value::string(event)));
                }
                object_value(fields)
            })
            .collect();
        if !items.is_empty() {
            chrome.push(pattern_spec(
                "menu",
                [
                    ("position", Value::string("bottom-end")),
                    ("items", arr(items)),
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
            ));
        }
    }

    let header = pattern_spec(
        "hstack",
        [
            ("gap", Value::string("sm")),
            ("justify", Value::string("between")),
            ("align", Value::string("start")),
            (
                "children",
                arr(vec![
                    pattern_spec(
                        "vstack",
                        [
                            ("gap", Value::string("xs")),
                            ("className", Value::string("flex-1 min-w-0")),
                            ("children", arr(identity)),
                        ],
                    ),
                    pattern_spec(
                        "hstack",
                        [
                            ("gap", Value::string("xs")),
                            ("align", Value::string("center")),
                            ("className", Value::string("flex-shrink-0 pt-1")),
                            ("children", arr(chrome)),
                        ],
                    ),
                ]),
            ),
        ],
    );

    let body = if editing {
        let mut fields = vec![("value", Value::string(value.as_str()))];
        if let Some(event) = props.get("contentChangeEvent") {
            fields.push(("changeEvent", event.clone()));
        }
        if let Some(placeholder) = &placeholder {
            fields.push(("placeholder", Value::string(placeholder.as_str())));
        }
        pattern_spec("rich-text-editor", fields)
    } else {
        let mut fields = vec![
            (
                "className",
                Value::string(if edit_event.is_some() {
                    "rounded-interactive px-1 -mx-1 min-h-[16rem] cursor-text transition-colors hover:bg-muted/30"
                } else {
                    "rounded-interactive px-1 -mx-1 min-h-[16rem]"
                }),
            ),
            (
                "children",
                arr(vec![if value.is_empty() {
                    text(
                        &placeholder
                            .clone()
                            .unwrap_or_else(|| crate::i18n::t("documentPanel.placeholder")),
                        "body",
                        "muted",
                        "",
                    )
                } else {
                    pattern_spec(
                        "rich-text-editor",
                        [
                            ("value", Value::string(value.as_str())),
                            ("readOnly", Value::Bool(true)),
                        ],
                    )
                }]),
            ),
        ];
        if let Some(event) = &edit_event {
            fields.push(("role", Value::string("button")));
            fields.push(("tabIndex", Value::Int(0)));
            fields.push(("aria-label", Value::string(edit_label.as_str())));
            fields.push(("action", Value::string(event.as_str())));
            fields.push(("actionPayload", id_payload(&record_id)));
        }
        pattern_spec("box", fields)
    };

    let mut card_children = Vec::new();
    if let Some(cover) = &cover {
        card_children.push(pattern_spec(
            "box",
            [
                ("className", Value::string("h-44 w-full sm:h-52")),
                (
                    "children",
                    arr(vec![pattern_spec(
                        "image",
                        [
                            ("src", Value::string(cover.as_str())),
                            ("alt", Value::string("")),
                            ("aria-hidden", Value::Bool(true)),
                            ("rounded", Value::string("none")),
                            ("className", Value::string("h-full w-full object-cover")),
                        ],
                    )]),
                ),
            ],
        ));
    }
    card_children.push(pattern_spec(
        "vstack",
        [
            ("gap", Value::string("sm")),
            ("className", Value::string("p-6 sm:p-8")),
            ("children", arr(vec![header, body])),
        ],
    ));

    let card = pattern_spec(
        "card",
        [
            ("variant", Value::string("elevated")),
            (
                "className",
                Value::string(format!(
                    "w-full overflow-hidden {}",
                    get_string_or(props, "className", "")
                )),
            ),
            ("children", arr(card_children)),
        ],
    );
    registry.render_spec(&card, None, &view)
}

impl DioxusPatternRenderer for DocumentPanelRenderer {
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
