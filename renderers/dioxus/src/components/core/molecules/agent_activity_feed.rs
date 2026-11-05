//! Native Dioxus renderer for the `agent-activity-feed` pattern (web: AgentActivityFeed.tsx).
//! Text is the web's default-locale (en) catalog; the host has no translation catalog.

use super::agent_activity_row::{
    accordion, activity_row, activity_row_facts, field, is_user_message, plain_text_of,
    render_pattern, thinking_row, typography, AccordionEntry,
};
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct AgentActivityFeedRenderer;

struct Conversation {
    prompt: Option<String>,
    started_at: String,
    activities: Vec<Value>,
}

fn group_into_conversations(activities: &[Value]) -> Vec<Conversation> {
    let mut groups: Vec<Conversation> = Vec::new();
    for activity in activities {
        let prompt = is_user_message(activity);
        if prompt || groups.is_empty() {
            groups.push(Conversation {
                prompt: prompt.then(|| get_string_or(activity, "content", "")),
                started_at: plain_text_of(field(activity, "timestamp")),
                activities: vec![activity.clone()],
            });
        } else if let Some(current) = groups.last_mut() {
            current.activities.push(activity.clone());
        }
    }
    groups
}

fn conversation_rows(
    activities: &[Value],
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
) -> Element {
    let facts = activity_row_facts(activities);
    rsx! {
        for (activity, fact) in activities.iter().zip(facts.iter()) {
            {activity_row(activity, *fact, registry, view)}
        }
    }
}

impl AgentActivityFeedRenderer {
    const NAME: &str = "agent-activity-feed";
}

impl DioxusPatternRenderer for AgentActivityFeedRenderer {
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
        let activities = get_array(props, "activities");
        let working = get_bool_or(props, "working", false);
        if activities.is_empty() && !working {
            return render_pattern(
                registry,
                view,
                "empty-state",
                vec![
                    ("icon", Value::string("sparkles")),
                    (
                        "title",
                        Value::string(get_string_or(props, "emptyTitle", "No activity yet")),
                    ),
                    (
                        "description",
                        Value::string(get_string_or(
                            props,
                            "emptyDescription",
                            "Send a message and the agent's work shows up here.",
                        )),
                    ),
                ],
            );
        }
        let conversations = group_into_conversations(&activities);
        let (earlier, current) = match conversations.split_last() {
            Some((current, earlier)) => (earlier, Some(current)),
            None => (&conversations[..0], None),
        };
        let class = format!(
            "flex flex-col gap-2 {}",
            get_string_or(props, "className", "")
        );
        let earlier_block = if earlier.is_empty() {
            None
        } else {
            let entries = earlier
                .iter()
                .enumerate()
                .map(|(index, conversation)| {
                    let label = conversation
                        .prompt
                        .clone()
                        .unwrap_or_else(|| "Earlier activity".to_string());
                    let header = rsx! {
                        div { class: "flex items-center gap-2 min-w-0", "data-testid": "agent-earlier-conversation",
                            {render_pattern(registry, view, "icon", vec![
                                ("name", Value::string("message-square")),
                                ("size", Value::string("sm")),
                                ("color", Value::string("muted")),
                                ("className", Value::string("shrink-0")),
                            ])}
                            {typography(registry, view, "body2", &label, None, None, Some("truncate"))}
                        }
                    };
                    let rows = if conversation.prompt.is_none() {
                        &conversation.activities[..]
                    } else {
                        &conversation.activities[1..]
                    };
                    let content = rsx! {
                        div { class: "flex flex-col gap-2 w-full", {conversation_rows(rows, registry, view)} }
                    };
                    AccordionEntry {
                        id: format!("conversation-{}-{index}", conversation.started_at),
                        header,
                        content,
                    }
                })
                .collect();
            let title = format!("Earlier conversations ({})", earlier.len());
            Some(rsx! {
                div { class: "flex flex-col gap-1 w-full", "data-testid": "agent-earlier-conversations",
                    {typography(registry, view, "caption", &title, Some("muted"), Some("semibold"), None)}
                    {accordion(entries, true, registry, view)}
                }
            })
        };
        let empty: Vec<Value> = Vec::new();
        let current_rows = current.map(|c| &c.activities).unwrap_or(&empty);
        rsx! {
            div { class: "{class}", "data-testid": "agent-activity-feed",
                if let Some(block) = earlier_block { {block} }
                {conversation_rows(current_rows, registry, view)}
                if working { {thinking_row(registry, view)} }
                div {}
            }
        }
    }
}
