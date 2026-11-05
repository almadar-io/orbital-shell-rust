//! Native Dioxus renderer for the `reply-tree` pattern (web: ReplyTree.tsx).
//! Text is the web's default-locale (en) catalog. Votes are drawn inline because the `vote-stack` body
//! cannot carry the per-node `{ nodeId, vote }` payload.

use super::agent_activity_row::{field, plain_text_of, render_pattern, typography};
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    object_value, plain_text, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
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
    let mut collapsed = input.collapsed;
    let mut reply_open = use_signal(|| false);
    let mut draft = use_signal(String::new);
    let node_id = plain_text_of(field(node, "id"));
    let author = get_string_or(node, "authorName", "");
    let avatar = get_string(node, "authorAvatarUrl");
    let content = get_string_or(node, "content", "");
    let posted_at = get_string_or(node, "postedAt", "");
    let vote_text = plain_text(&Value::Number(get_number_or(node, "voteCount", 0.0)));
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
    let toggle_button = render_pattern(
        registry,
        &view,
        "button",
        vec![
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("leftIcon", Value::string(if is_collapsed { "chevron-right" } else { "chevron-down" })),
            ("label", Value::string("")),
            ("aria-label", Value::string(if is_collapsed { "Expand replies" } else { "Collapse replies" })),
            ("aria-expanded", Value::Bool(!is_collapsed)),
            ("className", Value::string("w-6 h-6 p-0 min-w-0 rounded-interactive text-muted-foreground hover:bg-muted hover:text-foreground")),
        ],
    );
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

    let vote_button = |direction: &'static str, icon: &'static str| {
        let selected = user_vote.as_deref() == Some(direction);
        let next = if selected {
            Value::Null
        } else {
            Value::string(direction)
        };
        let event = config.vote_event.clone();
        let id = node_id.clone();
        let rounded = if direction == "up" {
            "rounded-l-interactive"
        } else {
            "rounded-r-interactive"
        };
        let class = format!(
            "inline-flex items-center justify-center {rounded} {} hover:bg-muted active:bg-muted transition-colors duration-instant disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent focus:outline-none focus-visible:ring-[length:var(--focus-ring-width)] focus-visible:ring-ring focus-visible:ring-inset w-7 h-7",
            if selected { "text-primary" } else { "text-muted-foreground" }
        );
        let label = if direction == "up" {
            "Upvote"
        } else {
            "Downvote"
        };
        rsx! {
            button {
                r#type: "button",
                class: "{class}",
                "aria-label": "{label}",
                "aria-pressed": "{selected}",
                onclick: move |_| {
                    if let (Some(sink), Some(event)) = (sink, &event) {
                        sink.call((event.clone(), object_value([("nodeId", Value::string(id.as_str())), ("vote", next.clone())])));
                    }
                },
                {render_pattern(registry, &view, "icon", vec![("name", Value::string(icon)), ("className", Value::string("w-3 h-3"))])}
            }
        }
    };

    let flag_button = {
        let mut fields = vec![
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("leftIcon", Value::string("flag")),
            ("label", Value::string("Flag")),
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
    let reply_button = render_pattern(
        registry,
        &view,
        "button",
        vec![
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("leftIcon", Value::string("message-square")),
            ("label", Value::string("Reply")),
            ("aria-label", Value::string(format!("Reply to {author}"))),
        ],
    );
    let send_button = {
        let mut fields = vec![
            ("variant", Value::string("primary")),
            ("size", Value::string("sm")),
            ("leftIcon", Value::string("send")),
            ("label", Value::string("Send")),
            ("disabled", Value::Bool(trimmed.is_empty())),
        ];
        if let Some(event) = &config.reply_event {
            fields.push(("action", Value::string(event.as_str())));
            fields.push((
                "actionPayload",
                object_value([
                    ("parentNodeId", Value::string(node_id.as_str())),
                    ("content", Value::string(trimmed.as_str())),
                ]),
            ));
        }
        render_pattern(registry, &view, "button", fields)
    };
    let cancel_button = render_pattern(
        registry,
        &view,
        "button",
        vec![
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("label", Value::string("Cancel")),
        ],
    );
    let continue_button = {
        let mut fields = vec![
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("rightIcon", Value::string("chevron-right")),
            ("label", Value::string("Continue thread")),
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
    let group_label = format!("Vote on reply by {author}");
    let up = vote_button("up", "chevron-up");
    let down = vote_button("down", "chevron-down");
    let children: Vec<(String, Value)> = replies
        .iter()
        .map(|child| (plain_text_of(field(child, "id")), child.clone()))
        .collect();

    rsx! {
        div { class: "flex flex-row gap-2 items-stretch min-w-0",
            div { class: "flex flex-col items-center flex-shrink-0 w-6",
                if has_replies {
                    div {
                        class: "contents",
                        onclick: move |_| {
                            let mut current = collapsed.write();
                            if let Some(position) = current.iter().position(|entry| *entry == toggle_id) {
                                current.remove(position);
                            } else {
                                current.push(toggle_id.clone());
                            }
                        },
                        {toggle_button}
                    }
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
                        div {
                            class: "inline-flex items-center justify-center w-fit flex-row rounded-interactive border-[length:var(--border-width)] border-border bg-surface",
                            role: "group",
                            "aria-label": "{group_label}",
                            {up}
                            span {
                                class: "text-center font-bold tabular-nums select-none text-foreground border-x-[length:var(--border-width)] border-border h-full px-1 py-0.5 text-sm min-w-[2rem]",
                                "aria-live": "polite",
                                "aria-atomic": "true",
                                "{vote_text}"
                            }
                            {down}
                        }
                        div { class: "contents", onclick: move |_| reply_open.set(!reply_open()), {reply_button} }
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
                            div { class: "contents", onclick: move |_| { draft.set(String::new()); reply_open.set(false); }, {send_button} }
                            div { class: "contents", onclick: move |_| { draft.set(String::new()); reply_open.set(false); }, {cancel_button} }
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
        return rsx! { div { class: "{class}", "No replies yet." } };
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
