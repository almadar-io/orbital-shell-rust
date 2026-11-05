//! Native Dioxus renderer for the `chat-bar` pattern (web: ChatBar.tsx).
//! Text is the web's default-locale (en) catalog.

use crate::components::core::molecules::agent_activity_row::{field, render_pattern, typography};
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    object_value, pattern_spec, render_with_local_sink, DioxusPatternRenderer, DioxusView,
    OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct ChatBarRenderer;

#[derive(Props, Clone, PartialEq)]
struct BarProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

const TEXTAREA_CLASS: &str = "block w-full interactive-border px-3 py-2 text-sm text-foreground placeholder:text-[var(--color-placeholder)] focus:outline-none focus:ring-offset-0 focus:ring-ring disabled:bg-muted disabled:text-muted-foreground disabled:cursor-not-allowed border-border focus:border-primary flex-1 min-w-0 min-h-0 border-0 rounded-none shadow-none bg-transparent resize-none focus:ring-0";

fn jepa_variant(probability: f64) -> &'static str {
    if probability >= 0.9 {
        "success"
    } else if probability >= 0.5 {
        "warning"
    } else {
        "danger"
    }
}

fn action_button(
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
    variant: &str,
    action: &str,
    icon: &str,
    label: &str,
    class: &str,
) -> Element {
    render_pattern(
        registry,
        view,
        "button",
        vec![
            ("variant", Value::string(variant)),
            ("size", Value::string("sm")),
            ("action", Value::string(action)),
            ("leftIcon", Value::string(icon)),
            ("label", Value::string(label)),
            ("className", Value::string(class)),
        ],
    )
}

#[allow(non_snake_case)]
fn Bar(input: BarProps) -> Element {
    let props = &input.props;
    let view = input.view.as_view();
    let registry = &input.registry;
    let status = get_string_or(props, "status", "idle");
    let send_event = get_string_or(props, "sendEvent", "CHAT_SEND");
    let class_name = get_string_or(props, "className", "");
    let requested = field(props, "focusRequest")
        .and_then(|request| get_string(request, "placeholder"))
        .filter(|text| !text.is_empty());
    let mut value = use_signal(String::new);
    let mut requested_placeholder = use_signal(|| requested);
    let sink = view.sink;
    let current = value();
    let trimmed = current.trim().to_string();
    let placeholder = requested_placeholder()
        .or_else(|| get_string(props, "placeholder"))
        .unwrap_or_else(|| "Ask the agent anything…".to_string());

    let key_event = send_event.clone();
    let on_keydown = move |event: KeyboardEvent| {
        if event.key() != Key::Enter || event.modifiers().shift() {
            return;
        }
        let text = value.peek().trim().to_string();
        if text.is_empty() {
            return;
        }
        event.prevent_default();
        if let Some(sink) = sink {
            sink.call((
                key_event.clone(),
                object_value([("message", Value::string(text))]),
            ));
        }
        value.set(String::new());
        requested_placeholder.set(None);
    };

    let trailing = match status.as_str() {
        "error" => action_button(
            registry,
            &view,
            "secondary",
            "RETRY_GENERATION",
            "refresh-cw",
            "Retry",
            "rounded-none flex-shrink-0 h-auto",
        ),
        "running" => rsx! {
            {action_button(registry, &view, "ghost", "PAUSE", "pause", "", "rounded-none flex-shrink-0 h-auto")}
            {action_button(registry, &view, "ghost", "STOP", "square", "", "rounded-none flex-shrink-0 h-auto border-l border-[var(--color-border)]")}
        },
        "paused" => action_button(
            registry,
            &view,
            "primary",
            "RESUME",
            "play",
            "Resume",
            "rounded-none flex-shrink-0 h-auto",
        ),
        _ => {
            let spec = pattern_spec(
                "button",
                vec![
                    ("variant", Value::string("primary")),
                    ("size", Value::string("sm")),
                    ("action", Value::string(send_event.as_str())),
                    (
                        "actionPayload",
                        object_value([("message", Value::string(trimmed.as_str()))]),
                    ),
                    ("leftIcon", Value::string("send")),
                    ("label", Value::string("")),
                    ("disabled", Value::Bool(trimmed.is_empty())),
                    ("aria-label", Value::string("Send message")),
                    (
                        "className",
                        Value::string("rounded-none flex-shrink-0 h-auto"),
                    ),
                ],
            );
            let sent_event = send_event.clone();
            render_with_local_sink(&view, registry, &spec, move |name, _| {
                if name == sent_event {
                    let mut text = value;
                    let mut placeholder = requested_placeholder;
                    text.set(String::new());
                    placeholder.set(None);
                }
                false
            })
        }
    };

    let focus = field(props, "editFocus").filter(|focus| focus.as_object().is_some());
    let context = get_string(props, "context").filter(|text| !text.is_empty());
    let gate = get_string(props, "activeGate").filter(|text| !text.is_empty());
    let jepa = get_number(props, "jepaValidity");
    let running = status == "running";
    let root_class =
        format!("border-t border-[var(--color-border)] bg-[var(--color-card)] {class_name}");
    let focus_label = focus.map(|focus| format!("Editing: {}", get_string_or(focus, "label", "")));
    let running_indicator = field(props, "runningIndicator").filter(|node| !node.is_null());

    rsx! {
        div { class: "{root_class}", ..shared_root_attrs(props),
            if let Some(label) = focus_label {
                div { class: "flex flex-row gap-1 items-center px-4 pt-2",
                    span { class: "inline-flex items-center gap-1 font-bold rounded-pill bg-secondary text-secondary-foreground px-2 py-0.5 text-xs flex items-center gap-1 max-w-full",
                        {render_pattern(registry, &view, "icon", vec![("name", Value::string("mouse-pointer-2")), ("size", Value::string("xs"))])}
                        {typography(registry, &view, "caption", &label, None, None, Some("text-inherit truncate"))}
                        div {
                            class: "cursor-pointer flex items-center ml-1 hover:opacity-70",
                            title: "Clear selection",
                            onclick: move |_| {
                                if let Some(sink) = sink {
                                    sink.call(("ELEMENT_SELECTED".to_string(), object_value([("focus", Value::Null)])));
                                }
                            },
                            {render_pattern(registry, &view, "icon", vec![("name", Value::string("x")), ("size", Value::string("xs"))])}
                        }
                    }
                }
            }
            div { class: "flex flex-row gap-2 items-center px-4 py-2",
                if let Some(text) = context {
                    {render_pattern(registry, &view, "badge", vec![
                        ("variant", Value::string("primary")),
                        ("size", Value::string("sm")),
                        ("label", Value::string(text)),
                        ("className", Value::string("whitespace-nowrap flex-shrink-0")),
                    ])}
                }
                div { class: "flex-1 min-w-0 flex items-stretch rounded-[var(--radius-md)] border border-[var(--color-border)] bg-[var(--color-background)] focus-within:border-[var(--color-primary)] overflow-hidden",
                    textarea {
                        class: "{TEXTAREA_CLASS}",
                        rows: "1",
                        value: "{current}",
                        placeholder: "{placeholder}",
                        oninput: move |event| value.set(event.value()),
                        onkeydown: on_keydown,
                    }
                    {trailing}
                }
                if gate.is_some() || jepa.is_some() {
                    div { class: "flex flex-row gap-1 items-center flex-shrink-0",
                        if running {
                            div { class: "flex items-center", "data-testid": "chat-bar-running",
                                if let Some(node) = running_indicator {
                                    {view.render_node(registry, node)}
                                } else {
                                    {render_pattern(registry, &view, "spinner", vec![("size", Value::string("sm"))])}
                                }
                            }
                        }
                        if let Some(gate) = gate {
                            {typography(
                                registry,
                                &view,
                                if running { "body" } else { "caption" },
                                &gate,
                                if running { None } else { Some("muted") },
                                if running { Some("medium") } else { None },
                                Some("whitespace-nowrap"),
                            )}
                        }
                        if let Some(probability) = jepa {
                            {render_pattern(registry, &view, "badge", vec![
                                ("variant", Value::string(jepa_variant(probability))),
                                ("size", Value::string("sm")),
                                ("label", Value::string(format!("{:.1}%", probability * 100.0))),
                            ])}
                        }
                    }
                }
            }
        }
    }
}

impl ChatBarRenderer {
    const NAME: &str = "chat-bar";
}

impl DioxusPatternRenderer for ChatBarRenderer {
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
        rsx! { Bar { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
