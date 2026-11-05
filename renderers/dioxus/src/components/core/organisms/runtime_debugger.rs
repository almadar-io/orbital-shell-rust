use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, dom_passthrough, layout_box_spec, object_value, pattern_spec,
    render_with_local_sink, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

const TOGGLE: &str = "__debugger_toggle";

pub struct RuntimeDebuggerRenderer;

impl RuntimeDebuggerRenderer {
    const NAME: &str = "runtime-debugger";
}

#[derive(Props, Clone, PartialEq)]
struct DebuggerProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn s(text: &str) -> Value {
    Value::string(text)
}

fn badge(variant: &str, text: &str) -> Value {
    pattern_spec(
        "badge",
        [
            ("variant", s(variant)),
            ("size", s("sm")),
            ("children", s(text)),
        ],
    )
}

fn tab_items(default_tab: Option<&str>) -> Value {
    let _ = default_tab;
    let tab = |id: &str, label: &str, badge: Option<i64>| {
        let mut fields = vec![("id", s(id)), ("label", s(label))];
        if let Some(count) = badge {
            fields.push(("badge", Value::Int(count)));
        }
        object_value(fields)
    };
    array_value(vec![
        tab("dispatch", "Dispatch", None),
        tab("verify", "Verify", None),
        tab("timeline", "Timeline", None),
        tab("bridge", "Bridge", Some(1)),
        tab("traits", "Traits", None),
        tab("ticks", "Ticks", None),
        tab("entities", "Entities", None),
        tab("events", "Events", None),
        tab("guards", "Guards", None),
    ])
}

fn tabs(props: &Value) -> Value {
    let mut fields = vec![
        ("items", tab_items(None)),
        ("variant", s("pills")),
        ("className", s("runtime-debugger__tabs")),
    ];
    if let Some(tab) = get_string(props, "defaultTab") {
        fields.push(("defaultActiveTab", Value::string(tab)));
    }
    pattern_spec("tabs", fields)
}

#[allow(non_snake_case)]
fn Debugger(input: DebuggerProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let mode = get_string_or(props, "mode", "floating");
    let class_name = get_string_or(props, "className", "");
    let default_collapsed = get_bool_or(props, "defaultCollapsed", true);
    let mut collapsed = use_signal(|| mode == "verify" || default_collapsed);
    let mut expanded = use_signal(|| true);

    match mode.as_str() {
        "inline" => {
            let is_collapsed = *collapsed.read();
            let root = cn(&["runtime-debugger", "runtime-debugger--inline", &class_name]);
            let panel_class = if is_collapsed {
                "runtime-debugger__panel runtime-debugger__panel--inline-collapsed"
            } else {
                "runtime-debugger__panel runtime-debugger__panel--inline"
            };
            let heading = pattern_spec(
                "typography",
                [
                    ("variant", s("h6")),
                    ("style", object_value([("fontSize", s("0.75rem"))])),
                    (
                        "content",
                        Value::string(format!(
                            "{} Debugger",
                            if is_collapsed { "\u{25B6}" } else { "\u{25BC}" }
                        )),
                    ),
                ],
            );
            let header = layout_box_spec(
                "runtime-debugger__header",
                Some(object_value([
                    ("cursor", s("pointer")),
                    ("userSelect", s("none")),
                ])),
                vec![
                    ("action".to_string(), s(TOGGLE)),
                    ("aria-expanded".to_string(), Value::Bool(!is_collapsed)),
                ],
                vec![pattern_spec(
                    "hstack",
                    [
                        ("gap", s("sm")),
                        (
                            "children",
                            array_value(vec![heading, badge("info", "Idle")]),
                        ),
                    ],
                )],
            );
            let mut children = vec![header];
            if !is_collapsed {
                children.push(layout_box_spec(
                    "runtime-debugger__content",
                    None,
                    Vec::new(),
                    vec![tabs(props)],
                ));
            }
            let card = pattern_spec(
                "card",
                [
                    ("className", s(panel_class)),
                    ("children", array_value(children)),
                ],
            );
            let content = render_with_local_sink(&view, registry, &card, move |event, _| {
                let mut collapsed = collapsed;
                if event == TOGGLE {
                    let next = !*collapsed.peek();
                    collapsed.set(next);
                    return true;
                }
                false
            });
            let attrs = dom_passthrough(props);
            let id = attrs
                .iter()
                .find(|(key, _)| key == "id")
                .and_then(|(_, value)| value.as_str().map(str::to_string));
            let label = attrs
                .iter()
                .find(|(key, _)| key == "aria-label")
                .and_then(|(_, value)| value.as_str().map(str::to_string));
            rsx! { div { class: "{root}", id: id, "aria-label": label, "data-testid": "debugger-inline", {content} } }
        }
        "verify" => {
            let is_expanded = *expanded.read();
            let root = cn(&[
                "runtime-debugger runtime-debugger--verify",
                "flex flex-col bg-[var(--color-card)] text-[var(--color-foreground)] border-t-heavy border-accent",
                "fixed bottom-0 left-0 right-0",
                &class_name,
            ]);
            let style = format!(
                "height:{};z-index:40",
                if is_expanded { "40vh" } else { "auto" }
            );
            let aria = if is_expanded {
                "Collapse the debugger"
            } else {
                "Expand the debugger"
            };
            let marker = if is_expanded { "\u{25BE}" } else { "\u{25B8}" };
            let status = badge("success", "OK");
            let status_el = registry.render_spec(&status, None, &view);
            rsx! {
                div { class: "{root}", "data-testid": "debugger-verify", "data-expanded": if is_expanded { "true" } else { "false" }, style: "{style}",
                    button {
                        r#type: "button",
                        class: "px-3 py-1.5 flex items-center gap-3 text-xs font-mono border-b border-border flex-shrink-0 w-full text-left hover:bg-[var(--color-card-hover,transparent)] cursor-pointer",
                        "aria-expanded": if is_expanded { "true" } else { "false" },
                        "aria-label": "{aria}",
                        "data-testid": "debugger-verify-toggle",
                        onclick: move |_| { let next = !*expanded.peek(); expanded.set(next); },
                        span { class: "text-muted-foreground w-3", "aria-hidden": "true", "{marker}" }
                        {status_el}
                        span { class: "text-muted-foreground", "0 local" }
                        span { class: "text-primary", "0 server" }
                    }
                    if is_expanded {
                        div { class: "flex-1 min-h-0 overflow-hidden",
                            div { class: "h-full overflow-y-auto px-2 py-1" }
                        }
                    }
                }
            }
        }
        _ => rsx! {},
    }
}

impl DioxusPatternRenderer for RuntimeDebuggerRenderer {
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
        rsx! { Debugger { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
