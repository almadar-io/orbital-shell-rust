//! Native Dioxus renderer for the `reply-tree` pattern (web: ReplyTree.tsx).
//! Text is the web's default-locale (en) catalog.

use super::agent_activity_row::{field, plain_text_of, render_pattern, typography};
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    object_value, pattern_spec, render_with_local_sink, DioxusPatternRenderer, DioxusView,
    OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct ReplyTreeRenderer;

#[derive(Clone, PartialEq)]
struct TreeConfig {
    vote_event: Option<String>,
    reply_event: Option<String>,
    flag_event: Option<String>,
    continue_event: Option<String>,
    show_actions: bool,
    max_depth: usize,
}

#[derive(Props, Clone, PartialEq)]
struct NodeProps {
    node: Value,
    depth: usize,
    config: TreeConfig,
    collapsed: Signal<Vec<String>>,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[derive(Props, Clone, PartialEq)]
struct TreeProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn node_list(props: &Value) -> Vec<Value> {
    match field(props, "nodes") {
        Some(Value::Array(items)) => items.iter().cloned().collect(),
        Some(node @ Value::Object(_)) => vec![node.clone()],
        _ => vec![],
    }
}

fn collect_collapsed(nodes: &[Value], out: &mut Vec<String>) {
    for node in nodes {
        let replies = get_array(node, "replies");
        if get_bool_or(node, "collapsed", false) {
            out.push(plain_text_of(field(node, "id")));
        }
        collect_collapsed(&replies, out);
    }
}

fn event_name(props: &Value, key: &str) -> Option<String> {
    get_string(props, key).filter(|name| !name.is_empty())
}

const TEXTAREA_CLASS: &str = "block w-full rounded-interactive border-[length:var(--border-width)] shadow-elevation-interactive interactive-border px-3 py-2 text-sm text-foreground bg-card placeholder:text-[var(--color-placeholder)] focus:outline-none focus:ring-2 focus:ring-offset-0 focus:ring-ring disabled:bg-muted disabled:text-muted-foreground disabled:cursor-not-allowed resize-y min-h-20 border-border focus:border-primary";

#[allow(non_snake_case)]
fn ReplyNode(input: NodeProps) -> Element {
    let view = input.view.as_view();
    let registry = &input.registry;
    let node = &input.node;
    let config = &input.config;
    let collapsed = input.collapsed;
    let reply_open = use_signal(|| false);
    let mut draft = use_signal(String::new);
    let node_id = plain_text_of(field(node, "id"));
    let author = get_string_or(node, "authorName", "");
    let avatar = get_string(node, "authorAvatarUrl");
    let content = get_string_or(node, "content", "");
    let posted_at = get_string_or(node, "postedAt", "");
    let user_vote = get_string(node, "userVote");
    let replies = get_array(node, "replies");
    let has_replies = !replies.is_empty();
    let is_collapsed = collapsed.read().contains(&node_id);
    let at_max_depth = input.depth >= config.max_depth;
    let sink = view.sink;
    let draft_text = draft();
    let trimmed = draft_text.trim().to_string();
    let is_open = reply_open();

    let toggle_id = node_id.clone();
    let toggle_spec = pattern_spec(
        "button",
        vec![
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("leftIcon", Value::string(if is_collapsed { "chevron-right" } else { "chevron-down" })),
            ("label", Value::string("")),
            ("aria-label", Value::string(if is_collapsed { crate::i18n::t("replyTree.expandReplies") } else { crate::i18n::t("replyTree.collapseReplies") })),
            ("aria-expanded", Value::Bool(!is_collapsed)),
            ("className", Value::string("w-6 h-6 p-0 min-w-0 rounded-interactive text-muted-foreground hover:bg-muted hover:text-foreground")),
            ("action", Value::string("__local_collapse")),
        ],
    );
    let toggle_button = render_with_local_sink(&view, registry, &toggle_spec, move |name, _| {
        if name != "__local_collapse" {
            return false;
        }
        let mut list = collapsed;
        let mut current = list.write();
        if let Some(position) = current.iter().position(|entry| *entry == toggle_id) {
            current.remove(position);
        } else {
            current.push(toggle_id.clone());
        }
        true
    });
    let avatar_el = {
        let mut fields = vec![
            ("name", Value::string(author.as_str())),
            ("size", Value::string("sm")),
        ];
        if let Some(src) = avatar {
            fields.push(("src", Value::string(src)));
        }
        render_pattern(registry, &view, "avatar", fields)
    };

    let vote_node = {
        let mut fields = vec![
            (
                "count",
                Value::Number(get_number_or(node, "voteCount", 0.0)),
            ),
            (
                "userVote",
                user_vote
                    .as_deref()
                    .map(Value::string)
                    .unwrap_or(Value::Null),
            ),
            ("size", Value::string("sm")),
            ("variant", Value::string("horizontal")),
            ("label", Value::string(format!("Vote on reply by {author}"))),
        ];
        if config.vote_event.is_some() {
            fields.push(("voteEvent", Value::string("__local_vote")));
        }
        let spec = pattern_spec("vote-stack", fields);
        let event = config.vote_event.clone();
        let id = node_id.clone();
        render_with_local_sink(&view, registry, &spec, move |name, payload| {
            if name != "__local_vote" {
                return false;
            }
            if let (Some(sink), Some(event)) = (sink, &event) {
                let vote = payload.get("next").cloned().unwrap_or(Value::Null);
                sink.call((
                    event.clone(),
                    object_value([("nodeId", Value::string(id.as_str())), ("vote", vote)]),
                ));
            }
            true
        })
    };

    let flag_button = {
        let mut fields = vec![
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("leftIcon", Value::string("flag")),
            ("label", Value::string(crate::i18n::t("replyTree.flag"))),
            (
                "aria-label",
                Value::string(format!("Flag reply by {author}")),
            ),
        ];
        if let Some(event) = &config.flag_event {
            fields.push(("action", Value::string(event.as_str())));
            fields.push((
                "actionPayload",
                object_value([("nodeId", Value::string(node_id.as_str()))]),
            ));
        }
        render_pattern(registry, &view, "button", fields)
    };
    let reply_spec = pattern_spec(
        "button",
        vec![
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("leftIcon", Value::string("message-square")),
            ("label", Value::string(crate::i18n::t("replyTree.reply"))),
            ("aria-label", Value::string(format!("Reply to {author}"))),
            ("action", Value::string("__local_reply_toggle")),
        ],
    );
    let reply_button = render_with_local_sink(&view, registry, &reply_spec, move |name, _| {
        if name != "__local_reply_toggle" {
            return false;
        }
        let mut open = reply_open;
        let next = !open();
        open.set(next);
        true
    });
    let send_button = {
        let send_action = config
            .reply_event
            .clone()
            .unwrap_or_else(|| "__local_send".to_string());
        let fields = vec![
            ("variant", Value::string("primary")),
            ("size", Value::string("sm")),
            ("leftIcon", Value::string("send")),
            ("label", Value::string(crate::i18n::t("replyTree.send"))),
            ("disabled", Value::Bool(trimmed.is_empty())),
            ("action", Value::string(send_action.as_str())),
            (
                "actionPayload",
                object_value([
                    ("parentNodeId", Value::string(node_id.as_str())),
                    ("content", Value::string(trimmed.as_str())),
                ]),
            ),
        ];
        let spec = pattern_spec("button", fields);
        let local_only = config.reply_event.is_none();
        render_with_local_sink(&view, registry, &spec, move |name, _| {
            if name != send_action {
                return false;
            }
            let mut text = draft;
            let mut open = reply_open;
            text.set(String::new());
            open.set(false);
            local_only
        })
    };
    let cancel_spec = pattern_spec(
        "button",
        vec![
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("label", Value::string(crate::i18n::t("common.cancel"))),
            ("action", Value::string("__local_cancel")),
        ],
    );
    let cancel_button = render_with_local_sink(&view, registry, &cancel_spec, move |name, _| {
        if name != "__local_cancel" {
            return false;
        }
        let mut text = draft;
        let mut open = reply_open;
        text.set(String::new());
        open.set(false);
        true
    });
    let continue_button = {
        let mut fields = vec![
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("rightIcon", Value::string("chevron-right")),
            ("label", Value::string(crate::i18n::t("replyTree.continueThread"))),
            ("className", Value::string("self-start gap-1 px-0 h-auto text-sm text-primary hover:underline hover:bg-transparent")),
        ];
        if let Some(event) = &config.continue_event {
            fields.push(("action", Value::string(event.as_str())));
            fields.push((
                "actionPayload",
                object_value([("nodeId", Value::string(node_id.as_str()))]),
            ));
        }
        render_pattern(registry, &view, "button", fields)
    };
    let placeholder = format!("Reply to {author}…");
    let aria_reply = format!("Reply to {author}");
    let children: Vec<(String, Value)> = replies
        .iter()
        .map(|child| (plain_text_of(field(child, "id")), child.clone()))
        .collect();

    rsx! {
        div { class: "flex flex-row gap-2 items-stretch min-w-0",
            div { class: "flex flex-col items-center flex-shrink-0 w-6",
                if has_replies {
                    {toggle_button}
                } else {
                    div { class: "w-6 h-6", "aria-hidden": "true" }
                }
                if has_replies && !is_collapsed {
                    div { class: "flex-1 w-px bg-border mt-1", "aria-hidden": "true" }
                }
            }
            div { class: "flex flex-col gap-2 flex-1 min-w-0 pb-3",
                div { class: "flex flex-row gap-2 items-center min-w-0",
                    {avatar_el}
                    {typography(registry, &view, "body", &author, None, Some("semibold"), None)}
                    {typography(registry, &view, "caption", &posted_at, Some("secondary"), None, None)}
                }
                {typography(registry, &view, "body", &content, None, None, Some("whitespace-pre-wrap break-words"))}
                if config.show_actions {
                    div { class: "flex flex-row gap-2 items-center",
                        {vote_node}
                        {reply_button}
                        {flag_button}
                    }
                }
                if is_open {
                    div { class: "flex flex-col gap-2 mt-1",
                        textarea {
                            class: "{TEXTAREA_CLASS}",
                            rows: "2",
                            value: "{draft_text}",
                            placeholder: "{placeholder}",
                            "aria-label": "{aria_reply}",
                            oninput: move |event| draft.set(event.value()),
                        }
                        div { class: "flex flex-row gap-2 items-center",
                            {send_button}
                            {cancel_button}
                        }
                    }
                }
                if has_replies && !is_collapsed {
                    if at_max_depth {
                        {continue_button}
                    } else {
                        div { class: "flex flex-col gap-2 mt-1",
                            for (child_id, child) in children.iter() {
                                ReplyNode {
                                    key: "{child_id}",
                                    node: child.clone(),
                                    depth: input.depth + 1,
                                    config: config.clone(),
                                    collapsed: collapsed,
                                    view: input.view.clone(),
                                    registry: input.registry.clone(),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[allow(non_snake_case)]
fn Tree(input: TreeProps) -> Element {
    let props = &input.props;
    let nodes = node_list(props);
    let collapsed = use_signal(|| {
        let mut initial = Vec::new();
        collect_collapsed(&nodes, &mut initial);
        initial
    });
    let class_name = get_string_or(props, "className", "");
    if nodes.is_empty() {
        let class = format!("text-sm text-muted-foreground {class_name}");
        return rsx! { div { class: "{class}", {crate::i18n::t("replyTree.noRepliesYet")} } };
    }
    let config = TreeConfig {
        vote_event: event_name(props, "voteEvent"),
        reply_event: event_name(props, "replyEvent"),
        flag_event: event_name(props, "flagEvent"),
        continue_event: event_name(props, "continueThreadEvent"),
        show_actions: get_bool_or(props, "showActions", true),
        max_depth: get_number_or(props, "maxDepth", 6.0) as usize,
    };
    let class = format!("flex flex-col gap-2 min-w-0 {class_name}");
    let keyed: Vec<(String, Value)> = nodes
        .iter()
        .map(|node| (plain_text_of(field(node, "id")), node.clone()))
        .collect();
    rsx! {
        div { class: "{class}",
            for (node_id, node) in keyed.iter() {
                ReplyNode {
                    key: "{node_id}",
                    node: node.clone(),
                    depth: 0usize,
                    config: config.clone(),
                    collapsed: collapsed,
                    view: input.view.clone(),
                    registry: input.registry.clone(),
                }
            }
        }
    }
}

impl ReplyTreeRenderer {
    const NAME: &str = "reply-tree";
}

impl DioxusPatternRenderer for ReplyTreeRenderer {
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
        rsx! { Tree { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
