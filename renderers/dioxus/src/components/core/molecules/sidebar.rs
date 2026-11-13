//! Native Dioxus body for the `sidebar` pattern, ported from `Sidebar.tsx`.

use crate::components::core::templates::dashboard_layout::{
    box_class, ghost_button_spec, icon_spec, slot,
};
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    array_value, emit_empty_event, object_value, pattern_spec, DioxusPatternRenderer, DioxusView,
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
    let internal = use_signal(|| get_bool_or(p, "defaultCollapsed", false));
    let collapsed = controlled.unwrap_or_else(|| internal());
    let brand_name = get_string_or(p, "brandName", "KFlow");
    let hide_collapse = get_bool_or(p, "hideCollapseButton", false);
    let show_close = get_bool_or(p, "showCloseButton", false);
    let collapse_event = get_string(p, "collapseChangeEvent").filter(|event| !event.is_empty());
    let close_event = get_string(p, "closeEvent");
    let logo_event = get_string(p, "logoClickEvent").filter(|event| !event.is_empty());
    let sink = view.sink;
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

    let collapse_class = format!(
        "p-1.5 hover:bg-muted text-foreground hidden lg:block rounded-interactive{}",
        if collapsed { " mx-auto" } else { "" }
    );
    let collapse_title = if collapsed {
        crate::i18n::t("sidebar.expand")
    } else {
        crate::i18n::t("sidebar.collapse")
    };
    let toggle_button = |class: &str, children: Vec<Value>, mut extra: Vec<(&str, Value)>| {
        extra.push(("action", Value::string("__local_toggle_collapse")));
        let spec = ghost_button_spec(class, children, extra);
        let event = collapse_event.clone();
        crate::renderer::render_with_local_sink(&view, registry, &spec, move |name, _| {
            if name != "__local_toggle_collapse" {
                return false;
            }
            let next = !collapsed;
            if !is_controlled {
                let mut state = internal;
                state.set(next);
            }
            if let (Some(sink), Some(event)) = (sink, &event) {
                sink.call((
                    event.clone(),
                    object_value([("collapsed", Value::Bool(next))]),
                ));
            }
            true
        })
    };
    let collapse_button = toggle_button(
        &collapse_class,
        vec![icon_spec(
            if collapsed {
                "chevron-right"
            } else {
                "chevron-left"
            },
            "h-[18px] w-[18px]",
        )],
        vec![("title", Value::string(collapse_title))],
    );
    let close_button = {
        let mut extra = vec![("aria-label", Value::string(crate::i18n::t("sidebar.close")))];
        if let Some(event) = close_event.as_ref() {
            extra.push(("action", Value::string(event.as_str())));
        }
        let spec = ghost_button_spec(
            "p-1.5 hover:bg-muted text-foreground lg:hidden rounded-interactive",
            vec![icon_spec("x", "h-[18px] w-[18px]")],
            extra,
        );
        registry.render_spec(&spec, None, &view)
    };

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
            let item_class = format!(
                "w-full flex items-center gap-3 px-3 py-2.5 transition-all duration-fast group relative rounded-interactive border-[length:var(--border-width-thin)] border-transparent {}",
                if active {
                    "bg-primary text-primary-foreground font-medium shadow-elevation-interactive border-primary translate-x-1 rtl:-translate-x-1 -translate-y-0.5"
                } else {
                    "text-foreground hover:bg-muted hover:border-border active:bg-primary active:text-primary-foreground"
                }
            );
            let mut children: Vec<Value> = Vec::new();
            if let Some(name) = get_string(item, "icon") {
                children.push(pattern_spec(
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
                ));
            }
            if !collapsed {
                children.push(pattern_spec(
                    "typography",
                    [
                        ("content", Value::string(label.as_str())),
                        ("variant", Value::string("body")),
                        ("color", Value::string(if active { "inherit" } else { "primary" })),
                        ("className", Value::string("font-medium truncate flex-1 text-start")),
                    ],
                ));
                if let Some(text) = badge_text(item) {
                    children.push(pattern_spec(
                        "badge",
                        [
                            ("label", Value::string(text)),
                            ("variant", Value::string("danger")),
                            ("size", Value::string("sm")),
                        ],
                    ));
                }
            } else {
                children.push(pattern_spec(
                    "box",
                    [
                        (
                            "className",
                            Value::string("absolute start-full ms-2 px-2 py-1 text-xs opacity-0 group-hover:opacity-100 pointer-events-none whitespace-nowrap z-50 transition-opacity bg-primary text-primary-foreground border-[length:var(--border-width-thin)] border-border rounded-interactive"),
                        ),
                        (
                            "children",
                            array_value(vec![pattern_spec(
                                "typography",
                                [
                                    ("content", Value::string(label.as_str())),
                                    ("variant", Value::string("body")),
                                    ("color", Value::string("inherit")),
                                    ("as", Value::string("span")),
                                    ("className", Value::string("text-xs")),
                                ],
                            )]),
                        ),
                    ],
                ));
            }
            let mut extra: Vec<(&str, Value)> = Vec::new();
            if collapsed {
                extra.push(("title", Value::string(label.as_str())));
            }
            if active {
                extra.push(("aria-current", Value::string("page")));
            }
            if let Some(href) = href {
                extra.push(("href", Value::string(href.as_str())));
            }
            if let Some(event) = get_string(item, "onClick") {
                extra.push(("action", Value::string(event.as_str())));
            }
            let spec = ghost_button_spec(&item_class, children, extra);
            registry.render_spec(&spec, None, &view)
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
    let footer_toggle = (collapsed && !hide_collapse).then(|| {
        toggle_button(
            "w-full flex justify-center p-2 mt-2 text-muted-foreground hover:text-foreground lg:hidden",
            vec![icon_spec("chevron-right", "h-5 w-5")],
            Vec::new(),
        )
    });

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
                if !hide_collapse { {collapse_button} }
                if show_close { {close_button} }
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
                    if let Some(footer_toggle) = footer_toggle { {footer_toggle} }
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
