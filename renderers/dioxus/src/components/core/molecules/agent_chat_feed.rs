//! Native Dioxus renderer for the `agent-chat-feed` pattern (web: AgentChatFeed.tsx).
//! Text is the web's default-locale (en) catalog; times and days are read in UTC (the host has no local zone).

use super::agent_activity_row::{
    activity_row, activity_row_facts, is_user_message, kind_of, render_pattern, thinking_row,
    typography, RowFacts,
};
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView, OwnedDioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct AgentChatFeedRenderer;

struct ChatTurn {
    request: Option<Value>,
    steps: Vec<Value>,
    replies: Vec<Value>,
    started_at: i64,
}

fn is_reply(activity: &Value) -> bool {
    let kind = kind_of(activity);
    (kind == "message" && !is_user_message(activity)) || kind == "error"
}

fn timestamp(activity: &Value) -> i64 {
    get_number_or(activity, "timestamp", 0.0) as i64
}

fn group_turns(activities: &[Value]) -> Vec<ChatTurn> {
    let mut turns: Vec<ChatTurn> = Vec::new();
    for activity in activities {
        let user = is_user_message(activity);
        let reply = is_reply(activity);
        if user || turns.is_empty() {
            turns.push(ChatTurn {
                request: user.then(|| activity.clone()),
                steps: if user || reply {
                    vec![]
                } else {
                    vec![activity.clone()]
                },
                replies: if reply {
                    vec![activity.clone()]
                } else {
                    vec![]
                },
                started_at: timestamp(activity),
            });
        } else if let Some(current) = turns.last_mut() {
            if reply {
                current.replies.push(activity.clone());
            } else {
                current.steps.push(activity.clone());
            }
        }
    }
    turns
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn day_number(ms: i64) -> i64 {
    ms.div_euclid(86_400_000)
}

fn day_label(ms: i64, now_ms: i64) -> String {
    let day = day_number(ms);
    if day == day_number(now_ms) {
        return "Today".to_string();
    }
    if day == day_number(now_ms) - 1 {
        return "Yesterday".to_string();
    }
    let (year, month, date) = civil_from_days(day);
    let name = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][month as usize - 1];
    format!("{name} {date}, {year}")
}

fn clock_label(ms: i64) -> String {
    let seconds = ms.div_euclid(1000).rem_euclid(86_400);
    let hour = seconds / 3600;
    let minute = (seconds % 3600) / 60;
    let hour12 = match hour % 12 {
        0 => 12,
        other => other,
    };
    format!(
        "{hour12:02}:{minute:02} {}",
        if hour < 12 { "AM" } else { "PM" }
    )
}

fn now_ms() -> i64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

fn time_el(
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
    ms: i64,
    align: &str,
) -> Element {
    typography(
        registry,
        view,
        "caption",
        &clock_label(ms),
        Some("muted"),
        None,
        Some(if align == "end" {
            "self-end"
        } else {
            "self-start"
        }),
    )
}

fn step_rows(steps: &[Value], registry: &DioxusPatternRegistry, view: &DioxusView<'_>) -> Element {
    let facts = activity_row_facts(steps);
    rsx! {
        div { class: "flex flex-col gap-1 items-stretch justify-start w-full",
            for (step, fact) in steps.iter().zip(facts.iter()) {
                {activity_row(step, *fact, registry, view)}
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct TurnStepsProps {
    show_raw: Signal<bool>,
    tool_steps: usize,
    has_raw: bool,
    switch_id: String,
    toggle_closed: Element,
    toggle_open: Element,
    switch_label: Element,
    body: Element,
}

#[allow(non_snake_case)]
fn TurnSteps(input: TurnStepsProps) -> Element {
    let mut open = use_signal(|| false);
    let mut show_raw = input.show_raw;
    let is_open = open();
    let checked = show_raw();
    rsx! {
        div { class: "flex flex-col gap-1 items-stretch justify-start self-start w-full max-w-[90%]", "data-testid": "agent-chat-steps",
            div {
                class: "contents",
                "data-testid": "agent-chat-steps-toggle",
                "aria-expanded": "{is_open}",
                onclick: move |_| open.set(!open()),
                if is_open { {input.toggle_open.clone()} } else { {input.toggle_closed.clone()} }
            }
            if is_open {
                div { class: "flex flex-col gap-1 items-stretch justify-start w-full border-s-[length:var(--border-width)] border-[var(--color-border)] ps-3",
                    if input.has_raw {
                        div { class: "inline-flex items-center gap-2",
                            dioxus_primitives::switch::Switch {
                                class: "group relative inline-flex h-[1.5rem] w-[2.75rem] shrink-0 cursor-pointer items-center rounded-full border-heavy border-transparent transition-colors duration-fast bg-muted data-[state=checked]:bg-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:cursor-not-allowed disabled:opacity-50",
                                checked: Some(checked),
                                id: "{input.switch_id}",
                                on_checked_change: move |next: bool| show_raw.set(next),
                                dioxus_primitives::switch::SwitchThumb { class: "pointer-events-none block h-[1.25rem] w-[1.25rem] rounded-full bg-background shadow-elevation-interactive ring-0 transition-transform duration-fast translate-x-0 group-data-[state=checked]:translate-x-[1.25rem]" }
                            }
                            label { class: "text-sm font-medium leading-none cursor-pointer", r#for: "{input.switch_id}", {input.switch_label.clone()} }
                        }
                    }
                    {input.body.clone()}
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct ChatFeedProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn ChatFeed(input: ChatFeedProps) -> Element {
    let props = &input.props;
    let view = input.view.as_view();
    let registry = &input.registry;
    let show_raw = use_signal(|| false);
    let activities = get_array(props, "activities");
    let working = get_bool_or(props, "working", false);
    let class_name = get_string_or(props, "className", "");
    let id_prefix = get_string_or(props, "idPrefix", "");
    if activities.is_empty() && !working {
        let class = format!("flex flex-col gap-0 items-center justify-center min-h-0 {class_name}");
        return rsx! {
            div { class: "{class}",
                {render_pattern(registry, &view, "empty-state", vec![
                    ("icon", Value::string("sparkles")),
                    ("title", Value::string(get_string_or(props, "emptyTitle", ""))),
                    ("description", Value::string("Ask in your own words — the assistant can look things up and make changes for you.")),
                ])}
            }
        };
    }
    let turns = group_turns(&activities);
    let raw_on = show_raw();
    let now = now_ms();
    let outer = format!("relative flex flex-col min-h-0 {class_name}");
    let scroller_id = format!("{id_prefix}-scroller");
    let last = turns.len().saturating_sub(1);
    rsx! {
        div { class: "{outer}",
            div { class: "flex-1 min-h-0 overflow-y-auto p-3", "data-testid": "{scroller_id}", role: "log", "aria-live": "polite",
                div { class: "flex flex-col gap-2 items-stretch justify-start w-full",
                    for (index, turn) in turns.iter().enumerate() {
                        {
                            let new_day = index == 0
                                || day_number(turns[index - 1].started_at) != day_number(turn.started_at);
                            let live = index == last && working && turn.replies.is_empty();
                            let visible: Vec<Value> = if raw_on {
                                turn.steps.clone()
                            } else {
                                turn.steps.iter().filter(|s| kind_of(s) != "llm_response").cloned().collect()
                            };
                            let tool_steps = turn.steps.iter().filter(|s| kind_of(s) != "llm_response").count();
                            let has_raw = turn.steps.iter().any(|s| kind_of(s) == "llm_response");
                            let steps_block = if live {
                                step_rows(&visible, registry, &view)
                            } else if tool_steps == 0 && !has_raw {
                                rsx! {}
                            } else {
                                let label = format!("Worked through {tool_steps} steps");
                                let toggle = |icon: &str| render_pattern(registry, &view, "button", vec![
                                    ("variant", Value::string("ghost")),
                                    ("size", Value::string("sm")),
                                    ("leftIcon", Value::string(icon)),
                                    ("label", Value::string(label.as_str())),
                                    ("className", Value::string("self-start")),
                                ]);
                                rsx! {
                                    TurnSteps {
                                        show_raw: show_raw,
                                        tool_steps: tool_steps,
                                        has_raw: has_raw,
                                        switch_id: format!("{id_prefix}-show-raw-llm-{index}"),
                                        toggle_closed: toggle("chevron-right"),
                                        toggle_open: toggle("chevron-down"),
                                        switch_label: rsx! { "Raw LLM" },
                                        body: step_rows(&visible, registry, &view),
                                    }
                                }
                            };
                            rsx! {
                                div { key: "turn-{turn.started_at}-{index}", class: "flex flex-col gap-1 items-stretch justify-start w-full",
                                    if new_day {
                                        div { class: "flex items-center gap-2 w-full py-1", "data-testid": "agent-chat-day", role: "separator",
                                            div { class: "h-px flex-1 bg-[var(--color-border)]" }
                                            {typography(registry, &view, "caption", &day_label(turn.started_at, now), Some("muted"), None, None)}
                                            div { class: "h-px flex-1 bg-[var(--color-border)]" }
                                        }
                                    }
                                    if let Some(request) = turn.request.as_ref() {
                                        div { class: "flex flex-col gap-0 items-stretch justify-start w-full",
                                            {activity_row(request, RowFacts::default(), registry, &view)}
                                            {time_el(registry, &view, timestamp(request), "end")}
                                        }
                                    }
                                    {steps_block}
                                    for (r, reply) in turn.replies.iter().enumerate() {
                                        div { key: "reply-{timestamp(reply)}-{r}", class: "flex flex-col gap-0 items-stretch justify-start w-full",
                                            {activity_row(reply, RowFacts::default(), registry, &view)}
                                            div { class: "flex items-center gap-1 self-start",
                                                {time_el(registry, &view, timestamp(reply), "start")}
                                                if kind_of(reply) == "message" {
                                                    {render_pattern(registry, &view, "button", vec![
                                                        ("variant", Value::string("ghost")),
                                                        ("size", Value::string("sm")),
                                                        ("icon", Value::string("copy")),
                                                        ("label", Value::string("")),
                                                        ("aria-label", Value::string("Copy reply")),
                                                    ])}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if working { {thinking_row(registry, &view)} }
                    div {}
                }
            }
        }
    }
}

impl AgentChatFeedRenderer {
    const NAME: &str = "agent-chat-feed";
}

impl DioxusPatternRenderer for AgentChatFeedRenderer {
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
        rsx! { ChatFeed { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
