//! Native Dioxus body for the `sidebar` pattern, ported from `Sidebar.tsx`.

use crate::components::core::templates::dashboard_layout::{box_class, ghost_button_class, icon_spec, slot, BUTTON_CONTENT};
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    emit_empty_event, object_value, pattern_spec, DioxusPatternRenderer, DioxusView,
    OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `sidebar` pattern.
pub struct SidebarRenderer;

impl SidebarRenderer {
    const NAME: &str = "sidebar";
}

#[derive(Props, Clone, PartialEq)]
struct SidebarProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn badge_text(item: &Value) -> Option<String> {
    match item.get("badge") {
        Some(Value::String(text)) => Some(text.to_string()),
        Some(Value::Int(number)) => Some(number.to_string()),
        Some(Value::Number(number)) => Some(number.to_string()),
        _ => None,
    }
}

#[allow(non_snake_case)]
fn Sidebar(props: SidebarProps) -> Element {
    let view = props.view.as_view();
    let registry = &props.registry;
    let p = &props.props;
    let controlled = p.get("collapsed").and_then(Value::as_bool);
    let mut internal = use_signal(|| get_bool_or(p, "defaultCollapsed", false));
    let collapsed = controlled.unwrap_or_else(|| internal());
    let brand_name = get_string_or(p, "brandName", "KFlow");
    let hide_collapse = get_bool_or(p, "hideCollapseButton", false);
    let show_close = get_bool_or(p, "showCloseButton", false);
    let collapse_event = get_string(p, "collapseChangeEvent").filter(|event| !event.is_empty());
    let close_event = get_string(p, "closeEvent");
    let logo_event = get_string(p, "logoClickEvent").filter(|event| !event.is_empty());
    let sink = view.sink;
    let navigate = view.navigate;
    let is_controlled = controlled.is_some();

    let class = box_class(&crate::renderer::cn(&[
        "flex flex-col h-full",
        "bg-card border-e border-border",
        "transition-all duration-normal ease-standard",
        if collapsed { "w-20" } else { "w-64" },
        &get_string_or(p, "className", ""),
    ]));

    let logo_el = match slot(p, "logo") {
        Some(Value::String(src)) => {
            let src = src.to_string();
            rsx! { img { src: "{src}", alt: "{brand_name}", class: "h-8 w-8" } }
        }
        Some(node) => view.render_node(registry, node),
        None => match get_string(p, "logoSrc").filter(|src| !src.is_empty()) {
            Some(src) => rsx! { img { src: "{src}", alt: "{brand_name}", class: "h-8 w-8" } },
            None => {
                let mark = registry.render_spec(
                    &pattern_spec(
                        "typography",
                        [
                            ("content", Value::string("K")),
                            ("variant", Value::string("small")),
                            (
                                "className",
                                Value::string("text-primary-foreground font-bold text-sm"),
                            ),
                        ],
                    ),
                    None,
                    &view,
                );
                rsx! { div { class: box_class("h-8 w-8 bg-primary flex items-center justify-center rounded-interactive"), {mark} } }
            }
        },
    };
    let name_el = (!collapsed).then(|| {
        registry.render_spec(
            &pattern_spec(
                "typography",
                [
                    ("content", Value::string(brand_name.as_str())),
                    ("variant", Value::string("body")),
                    (
                        "className",
                        Value::string("heading-voice text-xl text-foreground"),
                    ),
                ],
            ),
            None,
            &view,
        )
    });
    let logo_class = box_class(&crate::renderer::cn(&[
        "flex items-center gap-3 cursor-pointer",
        if collapsed {
            "justify-center w-full"
        } else {
            ""
        },
    ]));
    let pressable = logo_event.is_some();
    let logo_label = pressable.then(|| brand_name.clone());
    let logo_click = logo_event.clone();

    let chevron = registry.render_spec(
        &icon_spec(
            if collapsed {
                "chevron-right"
            } else {
                "chevron-left"
            },
            "h-[18px] w-[18px]",
        ),
        None,
        &view,
    );
    let collapse_class = ghost_button_class(&format!(
        "p-1.5 hover:bg-muted text-foreground hidden lg:block rounded-interactive{}",
        if collapsed { " mx-auto" } else { "" }
    ));
    let collapse_title = if collapsed {
        "Expand sidebar"
    } else {
        "Collapse sidebar"
    };
    let toggle_event = collapse_event.clone();
    let toggle_event_footer = collapse_event.clone();
    let close_icon = registry.render_spec(&icon_spec("x", "h-[18px] w-[18px]"), None, &view);
    let close_class =
        ghost_button_class("p-1.5 hover:bg-muted text-foreground lg:hidden rounded-interactive");
    let close_click = close_event.clone();

    let items = get_array(p, "items");
    let entries: Vec<Element> = items
        .iter()
        .map(|item| {
            let label = get_string_or(item, "label", "");
            let href = get_string(item, "href").filter(|href| !href.is_empty());
            let active = item
                .get("active")
                .and_then(Value::as_bool)
                .or_else(|| item.get("isActive").and_then(Value::as_bool))
                .unwrap_or(false);
            let item_class = ghost_button_class(&format!(
                "w-full flex items-center gap-3 px-3 py-2.5 transition-all duration-fast group relative rounded-interactive border-[length:var(--border-width-thin)] border-transparent {}",
                if active {
                    "bg-primary text-primary-foreground font-medium shadow-elevation-interactive border-primary translate-x-1 rtl:-translate-x-1 -translate-y-0.5"
                } else {
                    "text-foreground hover:bg-muted hover:border-border active:bg-primary active:text-primary-foreground"
                }
            ));
            let icon = get_string(item, "icon").map(|name| {
                registry.render_spec(
                    &pattern_spec(
                        "icon",
                        [
                            ("name", Value::string(name)),
                            ("size", Value::string("sm")),
                            (
                                "className",
                                Value::string(if active {
                                    "min-w-[20px] flex-shrink-0 text-primary-foreground"
                                } else {
                                    "min-w-[20px] flex-shrink-0"
                                }),
                            ),
                        ],
                    ),
                    None,
                    &view,
                )
            });
            let text = (!collapsed).then(|| {
                registry.render_spec(
                    &pattern_spec(
                        "typography",
                        [
                            ("content", Value::string(label.as_str())),
                            ("variant", Value::string("body")),
                            ("color", Value::string(if active { "inherit" } else { "primary" })),
                            ("className", Value::string("font-medium truncate flex-1 text-start")),
                        ],
                    ),
                    None,
                    &view,
                )
            });
            let badge = (!collapsed).then(|| badge_text(item)).flatten().map(|text| {
                registry.render_spec(
                    &pattern_spec(
                        "badge",
                        [
                            ("label", Value::string(text)),
                            ("variant", Value::string("danger")),
                            ("size", Value::string("sm")),
                        ],
                    ),
                    None,
                    &view,
                )
            });
            let tooltip = collapsed.then(|| {
                box_class("absolute start-full ms-2 px-2 py-1 text-xs opacity-0 group-hover:opacity-100 pointer-events-none whitespace-nowrap z-50 transition-opacity bg-primary text-primary-foreground border-[length:var(--border-width-thin)] border-border rounded-interactive")
            });
            let title = collapsed.then(|| label.clone());
            let on_click = get_string(item, "onClick");
            let body = rsx! {
                span { class: "{BUTTON_CONTENT}",
                    if let Some(icon) = icon { span { class: "flex-shrink-0", {icon} } }
                    if let Some(text) = text { {text} }
                    if let Some(badge) = badge { {badge} }
                    if let Some(tooltip) = tooltip { div { class: "{tooltip}", "{label}" } }
                }
            };
            match href {
                Some(href) => {
                    let target = href.clone();
                    rsx! {
                        a { class: "{item_class}", href: "{href}", title: title.as_deref(),
                            "aria-current": active.then_some("page"),
                            onclick: move |event| {
                                emit_empty_event(sink, on_click.as_deref());
                                if let Some(navigate) = navigate {
                                    if target.starts_with('/') || target.starts_with('#') {
                                        event.prevent_default();
                                        navigate.call(target.clone());
                                    }
                                }
                            },
                            {body}
                        }
                    }
                }
                None => rsx! {
                    button { class: "{item_class}", r#type: "button", title: title.as_deref(),
                        "aria-current": active.then_some("page"),
                        onclick: move |_| emit_empty_event(sink, on_click.as_deref()),
                        {body}
                    }
                },
            }
        })
        .collect();

    let footer_content = slot(p, "footerContent").map(|node| view.render_node(registry, node));
    let user_section = slot(p, "userSection").map(|node| view.render_node(registry, node));
    let has_footer = footer_content.is_some() || user_section.is_some();
    let footer_row = box_class(&crate::renderer::cn(&[
        "flex items-center",
        if collapsed {
            "justify-center flex-col gap-4"
        } else {
            "justify-between px-2"
        },
    ]));
    let footer_chevron = (collapsed && !hide_collapse)
        .then(|| registry.render_spec(&icon_spec("chevron-right", "h-5 w-5"), None, &view));
    let footer_toggle_class = ghost_button_class(
        "w-full flex justify-center p-2 mt-2 text-muted-foreground hover:text-foreground lg:hidden",
    );

    rsx! {
        aside { class: "{class}", ..shared_root_attrs(p),
            div { class: box_class("h-16 flex items-center justify-between px-4 border-b border-border flex-shrink-0"),
                div { class: "{logo_class}",
                    role: pressable.then_some("button"),
                    tabindex: pressable.then_some("0"),
                    "aria-label": logo_label.as_deref(),
                    onclick: move |_| emit_empty_event(sink, logo_click.as_deref()),
                    {logo_el}
                    if let Some(name_el) = name_el { {name_el} }
                }
                if !hide_collapse {
                    button { class: "{collapse_class}", r#type: "button", title: "{collapse_title}",
                        onclick: move |_| {
                            let next = !collapsed;
                            if !is_controlled {
                                internal.set(next);
                            }
                            if let (Some(sink), Some(event)) = (sink, &toggle_event) {
                                sink.call((event.clone(), object_value([("collapsed", Value::Bool(next))])));
                            }
                        },
                        span { class: "{BUTTON_CONTENT}", {chevron} }
                    }
                }
                if show_close {
                    button { class: "{close_class}", r#type: "button", "aria-label": "Close sidebar",
                        onclick: move |_| emit_empty_event(sink, close_click.as_deref()),
                        span { class: "{BUTTON_CONTENT}", {close_icon} }
                    }
                }
            }
            nav { class: box_class("flex-1 py-6 px-3 space-y-1 overflow-y-auto"),
                for entry in entries { {entry} }
            }
            if has_footer {
                div { class: box_class("p-3 border-t border-border space-y-1 flex-shrink-0"),
                    div { class: "{footer_row}",
                        if let Some(content) = footer_content { div { class: box_class("flex items-center"), {content} } }
                        if let Some(section) = user_section { div { class: box_class("flex items-center"), {section} } }
                    }
                    if let Some(chevron) = footer_chevron {
                        button { class: "{footer_toggle_class}", r#type: "button",
                            onclick: move |_| {
                                let next = !collapsed;
                                if !is_controlled {
                                    internal.set(next);
                                }
                                if let (Some(sink), Some(event)) = (sink, &toggle_event_footer) {
                                    sink.call((event.clone(), object_value([("collapsed", Value::Bool(next))])));
                                }
                            },
                            span { class: "{BUTTON_CONTENT}", {chevron} }
                        }
                    }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for SidebarRenderer {
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
        rsx! { Sidebar { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
