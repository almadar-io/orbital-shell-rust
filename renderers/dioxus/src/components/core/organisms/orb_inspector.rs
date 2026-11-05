use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    cn, js_string, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

const BOX: &str = "bg-transparent rounded-none shadow-none";

pub struct OrbInspectorRenderer;

impl OrbInspectorRenderer {
    const NAME: &str = "orb-inspector";
}

#[derive(Props, Clone, PartialEq)]
struct InspectorProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn small(class: &str, content: &str) -> Value {
    pattern_spec(
        "typography",
        [
            ("variant", Value::string("small")),
            ("className", Value::string(class)),
            ("content", Value::string(content)),
        ],
    )
}

fn token_contract(pattern_type: &str) -> &'static [&'static str] {
    match pattern_type {
        "button" => &[
            "--color-primary",
            "--color-primary-foreground",
            "--radius-md",
            "--shadow-sm",
        ],
        "badge" => &["--color-primary", "--radius-full"],
        "card" => &[
            "--color-card",
            "--color-border",
            "--radius-lg",
            "--shadow-main",
        ],
        "input" => &["--color-input", "--color-border", "--radius-md"],
        "typography" => &["--color-foreground", "--color-muted-foreground"],
        "divider" => &["--color-border"],
        "avatar" => &["--radius-full", "--color-muted"],
        "alert" => &[
            "--color-warning",
            "--color-success",
            "--color-danger",
            "--radius-md",
        ],
        "modal" => &["--color-card", "--shadow-lg", "--radius-lg"],
        "toast" => &["--color-card", "--shadow-lg", "--radius-md"],
        _ => &[],
    }
}

fn tree_value(value: &Value) -> Element {
    match value {
        Value::String(text) if text.starts_with('@') => {
            rsx! { span { class: "text-primary", "{text}" } }
        }
        Value::String(text) => {
            let quoted = format!("\"{text}\"");
            rsx! { "{quoted}" }
        }
        Value::Array(items)
            if items.first().is_some_and(
                |first| matches!(first, Value::String(text) if text.contains('/')),
            ) =>
        {
            let joined = items.iter().map(js_string).collect::<Vec<_>>().join(" ");
            let shown = format!("({joined})");
            rsx! { span { class: "text-warning", "{shown}" } }
        }
        Value::Array(items) => {
            let joined = items.iter().map(js_string).collect::<Vec<_>>().join(",");
            rsx! { "{joined}" }
        }
        Value::Object(_) => rsx! { "[object Object]" },
        other => {
            let text = js_string(other);
            rsx! { "{text}" }
        }
    }
}

fn pattern_tree(
    config: &Value,
    depth: usize,
    registry: &DioxusPatternRegistry,
    view: &crate::renderer::DioxusView<'_>,
) -> Element {
    let Some(fields) = config.as_object() else {
        return rsx! {};
    };
    let Some(kind) = fields
        .get("type")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return rsx! {};
    };
    let entries: Vec<(String, Value)> = fields
        .iter()
        .filter(|(key, _)| !matches!(key.as_ref(), "type" | "children"))
        .take(5)
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect();
    let children = match fields.get("children") {
        Some(Value::Array(items)) => items.iter().cloned().collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    let wrap = cn(&[BOX]);
    let row = cn(&[BOX, "flex gap-1 text-xs"]);
    let style = format!("padding-left:{}px", depth * 12);
    let title = small("text-primary font-semibold text-xs", &kind);
    rsx! {
        div { class: "{wrap}", style: "{style}",
            {registry.render_spec(&title, None, view)}
            for (key, value) in entries.iter() {
                div { class: "{row}",
                    span { class: "text-muted-foreground", "{key}:" }
                    span { {tree_value(value)} }
                }
            }
            for child in children.iter() { {pattern_tree(child, depth + 1, registry, view)} }
        }
    }
}

#[allow(non_snake_case)]
fn Inspector(input: InspectorProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let node = props.get("node").cloned().unwrap_or(Value::Null);
    let user_type = get_string_or(props, "userType", "builder");
    let initial = get_string_or(props, "defaultTab", "inspector");
    let mut active = use_signal(|| initial.clone());
    let selected_pattern = props
        .get("selectedPattern")
        .filter(|value| value.as_object().is_some())
        .cloned();
    let pattern_type = selected_pattern
        .as_ref()
        .map(|selected| get_string_or(selected, "patternType", ""));
    let orbital = get_string_or(&node, "orbitalName", "");
    let trait_name = get_string_or(&node, "traitName", "");
    let event = get_string_or(&node, "transitionEvent", "");
    let from = get_string_or(&node, "fromState", "");
    let to = get_string_or(&node, "toState", "");
    let entity_name = get_string_or(&node, "entityName", "");
    let effects: Vec<String> = get_array(&node, "effectTypes")
        .iter()
        .map(js_string)
        .collect();
    let patterns = get_array(&node, "patterns");
    let guard_text = node
        .get("guard")
        .filter(|g| !g.is_null())
        .map(|g| js_string(g));
    let is_expanded = !trait_name.is_empty();
    let architect = user_type == "architect";
    let on_close = get_string(props, "onClose");
    let sink = view.sink;

    let inspector_has_content = false;
    let current = active.read().clone();
    let shown = if current == "inspector" && !inspector_has_content {
        "design".to_string()
    } else {
        current
    };
    let header_title = match &pattern_type {
        Some(kind) => kind.clone(),
        None if is_expanded => {
            if event.is_empty() {
                "Transition".to_string()
            } else {
                event.clone()
            }
        }
        None => orbital.clone(),
    };
    let tabs: Vec<(&str, &str)> = [
        ("inspector", "Inspector"),
        ("design", "Design"),
        ("prototype", "Prototype"),
        ("code", "Code"),
    ]
    .into_iter()
    .filter(|(tab, _)| *tab != "code" || architect)
    .filter(|(tab, _)| *tab != "inspector" || inspector_has_content)
    .collect();

    let root = cn(&[
        BOX,
        "flex flex-col bg-card border-l border-border h-full w-full sm:w-[340px]",
    ]);
    let head_wrap = cn(&[BOX, "shrink-0 border-b border-border"]);
    let head_row = cn(&[BOX, "flex items-center justify-between px-4 py-2"]);
    let head_left = cn(&[BOX, "flex items-center gap-2"]);
    let chip = cn(&[BOX, "rounded px-2 py-0.5 text-xs font-mono font-semibold"]);
    let tab_bar = cn(&[BOX, "flex px-4 gap-4"]);
    let scroll = cn(&[BOX, "flex-1 overflow-y-auto"]);
    let section = cn(&[BOX, "px-4 py-3 border-b border-border/40"]);
    let section_tight = cn(&[BOX, "px-4 py-2 border-b border-border/40"]);

    let title_el = if pattern_type.is_some() {
        rsx! { div { class: "{chip}", style: "background-color:var(--color-primary);color:var(--color-primary-foreground)", "{header_title}" } }
    } else {
        registry.render_spec(&small("font-semibold", &header_title), None, &view)
    };

    let c0 = cn(&[BOX, "p-2"]);
    let c1 = cn(&[BOX, "flex items-center gap-2"]);
    let c2 = cn(&[BOX, "flex flex-col gap-1.5"]);
    let c3 = cn(&[BOX, "p-4"]);
    let c4 = cn(&[BOX, "px-4 py-3 flex flex-col gap-4"]);
    let c5 = cn(&[BOX]);
    let c6 = cn(&[BOX, "flex flex-col gap-1"]);
    let c7 = cn(&[BOX, "px-4 py-3"]);
    let body = match shown.as_str() {
        "code" if architect => rsx! { div { class: "{c0}" } },
        "prototype" => {
            let label = small(
                "text-muted-foreground text-xs uppercase tracking-wider mb-2",
                "Transition",
            );
            let trait_line = if trait_name.is_empty() {
                None
            } else {
                let on = if entity_name.is_empty() {
                    String::new()
                } else {
                    format!(" on {entity_name}")
                };
                Some(small(
                    "text-muted-foreground text-xs",
                    &format!("{trait_name}{on}"),
                ))
            };
            rsx! {
                if is_expanded && !from.is_empty() && !to.is_empty() {
                    div { class: "{section}",
                        {registry.render_spec(&label, None, &view)}
                        svg { width: "100%", height: "44", view_box: "0 0 280 44",
                            line { x1: "104", y1: "21", x2: "158", y2: "21", stroke: "var(--color-foreground)", stroke_width: "2", marker_end: "url(#orb-arrow)" }
                            defs { marker { id: "orb-arrow", marker_width: "8", marker_height: "6", ref_x: "8", ref_y: "3", orient: "auto", path { d: "M0,0 L8,3 L0,6 Z", fill: "var(--color-foreground)" } } }
                        }
                        if let Some(line) = trait_line.as_ref() { {registry.render_spec(line, None, &view)} }
                    }
                }
                if is_expanded && !event.is_empty() {
                    div { class: "{section_tight}",
                        div { class: "{c1}",
                            svg { width: "16", height: "16" }
                            {registry.render_spec(&small("font-semibold text-xs", &event), None, &view)}
                        }
                    }
                }
                if architect && is_expanded {
                    if let Some(guard) = guard_text.as_ref() {
                        div { class: "{section_tight}",
                            {registry.render_spec(
                                &pattern_spec("hstack", [
                                    ("gap", Value::string("xs")),
                                    ("className", Value::string("items-center")),
                                    ("children", Value::Array(vec![small("font-mono text-xs text-muted-foreground", guard)].into())),
                                ]),
                                None,
                                &view,
                            )}
                        }
                    }
                }
                if architect && is_expanded && !effects.is_empty() {
                    div { class: "{section}",
                        {registry.render_spec(&small("text-muted-foreground text-xs uppercase tracking-wider mb-2", &format!("Effects ({})", effects.len())), None, &view)}
                        div { class: "{c2}",
                            for (i, kind) in effects.iter().enumerate() {
                                {registry.render_spec(
                                    &pattern_spec("hstack", [
                                        ("gap", Value::string("xs")),
                                        ("className", Value::string("items-center")),
                                        ("children", Value::Array(vec![
                                            small("text-muted-foreground text-xs w-4 text-right shrink-0", &format!("{}.", i + 1)),
                                            small("text-xs flex-1", kind),
                                        ].into())),
                                    ]),
                                    None,
                                    &view,
                                )}
                            }
                        }
                    }
                }
            }
        }
        _ => {
            let styles = match pattern_type.as_deref() {
                None | Some("") => rsx! {
                    div { class: "{c3}",
                        {registry.render_spec(&small("text-muted-foreground text-xs", "Select a pattern to view its style tokens."), None, &view)}
                    }
                },
                Some(kind) => {
                    let tokens = token_contract(kind);
                    let tier = cn(&[
                        BOX,
                        "rounded px-1.5 py-0.5 text-[9px] font-mono uppercase tracking-wider",
                    ]);
                    rsx! {
                        div { class: "{c4}",
                            div { class: "{c1}",
                                {registry.render_spec(&small("font-semibold text-xs", kind), None, &view)}
                                div { class: "{tier}", style: "background-color:var(--color-muted);color:var(--color-muted-foreground)", "Pattern" }
                            }
                            div { class: "{c5}",
                                {registry.render_spec(&small("text-muted-foreground text-xs uppercase tracking-wider mb-2", "Tokens"), None, &view)}
                                if tokens.is_empty() {
                                    {registry.render_spec(&small("text-muted-foreground text-xs italic", "No token contract declared for this pattern."), None, &view)}
                                } else {
                                    div { class: "{c6}",
                                        for token in tokens.iter() {
                                            div { class: "{c1}",
                                                {registry.render_spec(&small("font-mono text-xs", token), None, &view)}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            };
            let source_box = cn(&[
                BOX,
                "bg-muted/20 rounded-md p-3 font-mono text-xs leading-relaxed overflow-x-auto",
            ]);
            rsx! {
                {styles}
                if architect && !patterns.is_empty() && selected_pattern.is_none() {
                    div { class: "{c7}",
                        {registry.render_spec(&small("text-muted-foreground text-xs uppercase tracking-wider mb-2", "render-ui"), None, &view)}
                        div { class: "{source_box}",
                            for entry in patterns.iter() {
                                div { class: "{c5}",
                                    {registry.render_spec(&small("text-muted-foreground text-xs", &format!("slot: {}", get_string_or(entry, "slot", ""))), None, &view)}
                                    {entry.get("pattern").map(|pattern| pattern_tree(pattern, 0, registry, &view)).unwrap_or_else(|| rsx! {})}
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    rsx! {
        div { class: "{root}",
            div { class: "{head_wrap}",
                div { class: "{head_row}",
                    div { class: "{head_left}", {title_el} }
                    button {
                        class: "text-muted-foreground hover:text-foreground text-sm cursor-pointer bg-transparent border-none p-1",
                        "aria-label": "Close",
                        onclick: move |_| { if let (Some(sink), Some(event)) = (sink, on_close.clone()) { sink.call((event, Value::Null)); } },
                        "\u{d7}"
                    }
                }
                div { class: "{tab_bar}",
                    for (tab, label) in tabs.iter() {
                        {
                            let tab = tab.to_string();
                            let class = if shown == tab {
                                "pb-2 text-xs font-medium border-b-2 cursor-pointer bg-transparent border-x-0 border-t-0 px-0 capitalize border-[var(--color-primary)] text-foreground"
                            } else {
                                "pb-2 text-xs font-medium border-b-2 cursor-pointer bg-transparent border-x-0 border-t-0 px-0 capitalize border-transparent text-muted-foreground hover:text-foreground"
                            };
                            rsx! { button { class: "{class}", onclick: move |_| active.set(tab.clone()), "{label}" } }
                        }
                    }
                }
            }
            div { class: "{scroll}", {body} }
        }
    }
}

impl DioxusPatternRenderer for OrbInspectorRenderer {
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
        rsx! { Inspector { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
