//! Native Dioxus body for the `dashboard-layout` pattern, ported from `DashboardLayout.tsx`.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    pattern_spec, DioxusPatternRenderer, DioxusView, NavigationSink, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::{Value, ValueMap};

pub(crate) fn box_class(extra: &str) -> String {
    crate::renderer::cn(&["bg-transparent rounded-none shadow-none", extra])
}

/// The web `HStack`/`VStack` classes for the given align/justify/gap tokens and `className`.
pub(crate) fn stack_class(
    horizontal: bool,
    gap: &str,
    align: &str,
    justify: &str,
    extra: &str,
) -> String {
    let direction = if horizontal {
        "flex flex-row"
    } else {
        "flex flex-col"
    };
    crate::renderer::cn(&[direction, gap, align, justify, extra])
}

const BUTTON_BASE: &str = "relative inline-flex items-center justify-center gap-2 font-medium rounded-interactive interactive-border cursor-pointer chrome-button transition-all duration-normal focus:outline-none focus-visible:ring-[length:var(--focus-ring-width)] focus-visible:ring-ring focus-visible:ring-offset-[length:var(--focus-ring-offset)] disabled:opacity-50 disabled:cursor-not-allowed";
const BUTTON_GHOST: &str = "bg-transparent text-muted-foreground border border-transparent hover:text-primary-foreground hover:bg-primary hover:border-primary active:scale-[var(--active-scale)]";
const BUTTON_MD: &str = "h-button-md px-4 text-sm";
pub(crate) const BUTTON_CONTENT: &str =
    "inline-flex flex-1 min-w-0 items-center [justify-content:inherit] gap-2";

/// The ghost `Button` classes for a `className`, for a button that carries children (the `button` body takes none).
pub(crate) fn ghost_button_class(extra: &str) -> String {
    crate::renderer::cn(&[BUTTON_BASE, BUTTON_GHOST, BUTTON_MD, extra])
}

pub(crate) fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::String(text) => !text.is_empty(),
        Value::Int(number) => *number != 0,
        Value::Number(number) => *number != 0.0,
        _ => true,
    }
}

pub(crate) fn slot<'a>(props: &'a Value, key: &str) -> Option<&'a Value> {
    props.get(key).filter(|value| truthy(value))
}

pub(crate) fn icon_spec(name: &str, class: &str) -> Value {
    pattern_spec(
        "icon",
        [
            ("name", Value::string(name)),
            ("className", Value::string(class)),
        ],
    )
}

fn text_spec(content: &str, variant: &str, class: &str, extra: &[(&str, &str)]) -> Value {
    let mut fields = vec![
        ("content", Value::string(content)),
        ("variant", Value::string(variant)),
        ("className", Value::string(class)),
    ];
    for (key, value) in extra {
        fields.push((*key, Value::string(*value)));
    }
    pattern_spec("typography", fields)
}

fn badge_text(item: &Value) -> Option<String> {
    match item.get("badge") {
        Some(Value::String(text)) if !text.is_empty() => Some(text.to_string()),
        Some(Value::Int(number)) if *number != 0 => Some(number.to_string()),
        Some(Value::Number(number)) if *number != 0.0 => Some(number.to_string()),
        _ => None,
    }
}

fn badge_spec(text: &str, active: bool, class: &str) -> Value {
    let mut fields = vec![
        ("label", Value::string(text)),
        (
            "variant",
            Value::string(if active { "primary" } else { "default" }),
        ),
        ("size", Value::string("sm")),
    ];
    if !class.is_empty() {
        fields.push(("className", Value::string(class)));
    }
    pattern_spec("badge", fields)
}

fn active_href(items: &[Value], current: &str) -> Option<String> {
    if current.is_empty() {
        return None;
    }
    let mut best: Option<String> = None;
    for item in items {
        let mut hrefs = vec![get_string_or(item, "href", "")];
        for child in get_array(item, "children") {
            hrefs.push(get_string_or(&child, "href", ""));
        }
        for href in hrefs {
            if current == href {
                return Some(href);
            }
            if current.starts_with(&format!("{href}/"))
                && best.as_ref().is_none_or(|known| href.len() > known.len())
            {
                best = Some(href);
            }
        }
    }
    best
}

fn empty_payload() -> Value {
    Value::Object(ValueMap::new().into())
}

fn is_local_route(href: &str) -> bool {
    href.starts_with('/') || href.starts_with('#')
}

fn nav_anchor(
    class: String,
    href: &str,
    current: bool,
    title: Option<String>,
    aria_label: Option<String>,
    navigate: Option<NavigationSink>,
    close: Option<Signal<bool>>,
    body: Element,
) -> Element {
    let target = href.to_string();
    let shown = href.to_string();
    rsx! {
        a { class: "{class}", href: "{shown}",
            "aria-current": current.then_some("page"),
            title: title.as_deref(),
            "aria-label": aria_label.as_deref(),
            onclick: move |event| {
                if let Some(navigate) = navigate {
                    if is_local_route(&target) {
                        event.prevent_default();
                        navigate.call(target.clone());
                    }
                }
                if let Some(mut close) = close {
                    close.set(false);
                }
            },
            {body}
        }
    }
}

/// Dioxus renderer for the `dashboard-layout` pattern.
pub struct DashboardLayoutRenderer;

impl DashboardLayoutRenderer {
    const NAME: &str = "dashboard-layout";
}

#[derive(Props, Clone, PartialEq)]
struct LayoutProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[derive(Props, Clone, PartialEq)]
struct NavLinkProps {
    item: Value,
    active_href: Option<String>,
    compact: bool,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn NavLink(props: NavLinkProps) -> Element {
    let view = props.view.as_view();
    let registry = &props.registry;
    let item = &props.item;
    let label = get_string_or(item, "label", "");
    let href = get_string_or(item, "href", "");
    let children = get_array(item, "children");
    let has_children = !children.is_empty();
    let active = props.active_href.as_deref();
    let child_active = has_children
        && children
            .iter()
            .any(|child| Some(get_string_or(child, "href", "").as_str()) == active);
    let is_active = Some(href.as_str()) == active || child_active;
    let mut open = use_signal(|| child_active);
    let navigate = view.navigate;
    let icon_name = get_string(item, "icon");
    let badge = badge_text(item);
    let icon_class = if is_active {
        "h-5 w-5 text-primary-foreground"
    } else {
        "h-5 w-5 text-muted-foreground"
    };
    let initial = label
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default();
    let state_class = if is_active {
        "bg-primary text-primary-foreground shadow-elevation-interactive"
    } else {
        "text-muted-foreground hover:bg-muted hover:text-foreground"
    };

    if props.compact {
        let glyph = match &icon_name {
            Some(name) => registry.render_spec(&icon_spec(name, icon_class), None, &view),
            None => registry.render_spec(
                &text_spec(&initial, "small", "font-semibold", &[]),
                None,
                &view,
            ),
        };
        let corner = badge.as_ref().map(|text| {
            registry.render_spec(
                &badge_spec(
                    text,
                    is_active,
                    "absolute -top-2 -right-2 px-1 py-0 text-[10px] leading-4",
                ),
                None,
                &view,
            )
        });
        let mark = rsx! {
            span { class: box_class("relative inline-flex"),
                {glyph}
                if let Some(corner) = corner { {corner} }
            }
        };
        if !has_children {
            let class = crate::renderer::cn(&[
                "flex items-center justify-center px-2 py-2 rounded-interactive transition-colors",
                state_class,
            ]);
            return nav_anchor(
                class,
                &href,
                Some(href.as_str()) == active,
                Some(label.clone()),
                Some(label.clone()),
                navigate,
                None,
                mark,
            );
        }
        let button_class = ghost_button_class(&format!(
            "flex items-center justify-center w-full px-2 py-2 rounded-interactive transition-colors {state_class}"
        ));
        let flyout_items = children.iter().map(|child| {
            let child_href = get_string_or(child, "href", "");
            let child_label = get_string_or(child, "label", "");
            let child_is_active = Some(child_href.as_str()) == active;
            let child_icon = get_string(child, "icon")
                .map(|name| registry.render_spec(&icon_spec(&name, "h-4 w-4"), None, &view));
            let child_badge = badge_text(child).map(|text| {
                registry.render_spec(&badge_spec(&text, child_is_active, ""), None, &view)
            });
            let text = registry.render_spec(
                &text_spec(&child_label, "small", "flex-1", &[("color", "primary")]),
                None,
                &view,
            );
            let class = crate::renderer::cn(&[
                "flex items-center gap-2 px-3 py-2 text-sm transition-colors",
                if child_is_active {
                    "bg-muted text-foreground font-medium"
                } else {
                    "text-muted-foreground hover:bg-muted hover:text-foreground"
                },
            ]);
            let body = rsx! {
                if let Some(child_icon) = child_icon { {child_icon} }
                {text}
                if let Some(child_badge) = child_badge { {child_badge} }
            };
            nav_anchor(
                class,
                &child_href,
                child_is_active,
                None,
                None,
                navigate,
                Some(open),
                body,
            )
        });
        let flyout: Vec<Element> = flyout_items.collect();
        return rsx! {
            div { class: box_class("relative"),
                button { class: "{button_class}", r#type: "button", title: "{label}", "aria-label": "{label}",
                    "aria-expanded": if open() { "true" } else { "false" },
                    onclick: move |_| { let next = !open(); open.set(next); },
                    span { class: "{BUTTON_CONTENT}", {mark} }
                }
                if open() {
                    div { class: box_class("fixed inset-0 z-20"), onclick: move |_| open.set(false) }
                    div { class: box_class("absolute left-full top-0 ml-2 w-48 bg-card surface-material rounded-container shadow-elevation-popover border border-border py-1 z-30"),
                        for entry in flyout { {entry} }
                    }
                }
            }
        };
    }

    let icon = icon_name
        .as_ref()
        .map(|name| registry.render_spec(&icon_spec(name, icon_class), None, &view));
    let text_color = if is_active { "inherit" } else { "primary" };
    let text = registry.render_spec(
        &text_spec(&label, "small", "flex-1", &[("color", text_color)]),
        None,
        &view,
    );
    let badge_el = badge
        .as_ref()
        .map(|text| registry.render_spec(&badge_spec(text, is_active, ""), None, &view));

    if !has_children {
        let class = crate::renderer::cn(&[
            "flex items-center gap-3 px-3 py-2 rounded-interactive text-sm font-medium transition-colors",
            state_class,
        ]);
        let body = rsx! {
            if let Some(icon) = icon { {icon} }
            {text}
            if let Some(badge_el) = badge_el { {badge_el} }
        };
        return nav_anchor(
            class,
            &href,
            Some(href.as_str()) == active,
            None,
            None,
            navigate,
            None,
            body,
        );
    }

    let chevron = registry.render_spec(
        &icon_spec(
            "chevron-down",
            if open() {
                "h-4 w-4 shrink-0 transition-transform rotate-180"
            } else {
                "h-4 w-4 shrink-0 transition-transform"
            },
        ),
        None,
        &view,
    );
    let button_class = ghost_button_class(&format!(
        "flex items-center gap-3 w-full px-3 py-2 rounded-interactive text-sm font-medium transition-colors {state_class}"
    ));
    let sub_items: Vec<Element> = children
        .iter()
        .map(|child| {
            let child_href = get_string_or(child, "href", "");
            let child_label = get_string_or(child, "label", "");
            let child_is_active = Some(child_href.as_str()) == active;
            let child_icon = get_string(child, "icon")
                .map(|name| registry.render_spec(&icon_spec(&name, "h-4 w-4"), None, &view));
            let child_badge = badge_text(child).map(|text| {
                registry.render_spec(&badge_spec(&text, child_is_active, ""), None, &view)
            });
            let child_text = registry.render_spec(
                &text_spec(
                    &child_label,
                    "small",
                    "flex-1",
                    &[(
                        "color",
                        if child_is_active {
                            "inherit"
                        } else {
                            "primary"
                        },
                    )],
                ),
                None,
                &view,
            );
            let class = crate::renderer::cn(&[
                "flex items-center gap-2 px-3 py-1.5 rounded-interactive text-sm transition-colors",
                if child_is_active {
                    "bg-primary text-primary-foreground shadow-elevation-interactive"
                } else {
                    "text-muted-foreground hover:bg-muted hover:text-foreground"
                },
            ]);
            let body = rsx! {
                if let Some(child_icon) = child_icon { {child_icon} }
                {child_text}
                if let Some(child_badge) = child_badge { {child_badge} }
            };
            nav_anchor(
                class,
                &child_href,
                child_is_active,
                None,
                None,
                navigate,
                None,
                body,
            )
        })
        .collect();
    let group_class = stack_class(
        false,
        "gap-0",
        "items-stretch",
        "justify-start",
        "mt-1 ml-4 pl-4 border-s-[length:var(--border-width)] border-border space-y-1",
    );
    rsx! {
        div { class: box_class(""),
            button { class: "{button_class}", r#type: "button",
                "aria-expanded": if open() { "true" } else { "false" },
                onclick: move |_| { let next = !open(); open.set(next); },
                span { class: "{BUTTON_CONTENT}",
                    if let Some(icon) = icon { span { class: "flex-shrink-0", {icon} } }
                    {text}
                    if let Some(badge_el) = badge_el { {badge_el} }
                    span { class: "flex-shrink-0", {chevron} }
                }
            }
            if open() {
                div { class: "{group_class}",
                    for entry in sub_items { {entry} }
                }
            }
        }
    }
}

fn bottom_link(
    item: &Value,
    active: Option<&str>,
    navigate: Option<NavigationSink>,
    view: &DioxusView<'_>,
    registry: &DioxusPatternRegistry,
) -> Element {
    let href = get_string_or(item, "href", "");
    let label = get_string_or(item, "label", "");
    let is_active = Some(href.as_str()) == active;
    let icon = get_string(item, "icon").map(|name| {
        let class = if is_active {
            "h-5 w-5 text-primary"
        } else {
            "h-5 w-5 text-muted-foreground"
        };
        registry.render_spec(&icon_spec(&name, class), None, view)
    });
    let text_class = crate::renderer::cn(&[
        "text-xs leading-tight truncate max-w-full",
        if is_active {
            "text-primary font-medium"
        } else {
            "text-muted-foreground"
        },
    ]);
    let text = registry.render_spec(&text_spec(&label, "caption", &text_class, &[]), None, view);
    let badge = badge_text(item).map(|text| {
        registry.render_spec(
            &badge_spec(&text, is_active, "text-xs px-1 py-0 min-w-0"),
            None,
            view,
        )
    });
    let body = rsx! {
        if let Some(icon) = icon { {icon} }
        {text}
        if let Some(badge) = badge { {badge} }
    };
    nav_anchor(
        "flex flex-col items-center justify-center gap-0.5 px-2 py-1 rounded-interactive transition-colors flex-1 min-w-0".to_string(),
        &href,
        is_active,
        None,
        None,
        navigate,
        None,
        body,
    )
}

fn initials_of(name: &str) -> String {
    name.split(' ')
        .filter_map(|part| part.chars().next())
        .collect::<String>()
        .chars()
        .take(2)
        .collect()
}

fn action_item(action: &Value, index: usize) -> Value {
    let event = get_string(action, "event");
    let route = get_string(action, "navigatesTo");
    let key = event
        .clone()
        .or_else(|| route.clone())
        .unwrap_or_else(|| "entry".to_string());
    let mut fields = vec![
        ("id", Value::string(format!("{key}-{index}"))),
        ("label", Value::string(get_string_or(action, "label", ""))),
    ];
    if let Some(icon) = get_string(action, "icon") {
        fields.push(("icon", Value::string(icon)));
    }
    if get_string(action, "variant").as_deref() == Some("danger") {
        fields.push(("variant", Value::string("danger")));
    }
    if let Some(event) = event {
        fields.push(("event", Value::string(event)));
    }
    if let Some(route) = route {
        fields.push(("href", Value::string(route)));
    }
    let mut map = ValueMap::new();
    for (name, value) in fields {
        map.insert(name.into(), value);
    }
    Value::Object(map.into())
}

#[allow(non_snake_case)]
fn DashboardLayout(props: LayoutProps) -> Element {
    let view = props.view.as_view();
    let registry = &props.registry;
    let p = &props.props;
    let mut sidebar_open = use_signal(|| false);
    let mut search_open = use_signal(|| false);

    let app_name = get_string_or(p, "appName", "App");
    let initial = app_name
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default();
    let nav_items = get_array(p, "navItems");
    let layout_mode = get_string_or(p, "layoutMode", "sidebar");
    let full_bleed = get_bool_or(p, "fullBleed", false);
    let show_theme_toggle = get_bool_or(p, "showThemeToggle", true);
    let search_event = get_string(p, "searchEvent").filter(|event| !event.is_empty());
    let search_enabled = get_bool_or(p, "showSearch", false) || search_event.is_some();
    let notifications = p.get("notifications").and_then(Value::as_array);
    let unread = notifications.map(|list| {
        list.iter()
            .filter(|entry| entry.get("read").and_then(Value::as_bool) != Some(true))
            .count()
    });
    let top_bar_actions = get_array(p, "topBarActions");
    let user_menu_items = get_array(p, "userMenuItems");
    let current_path = get_string_or(p, "currentPath", "");
    let active = active_href(&nav_items, &current_path);
    let sink = view.sink;
    let navigate = view.navigate;
    let logo = slot(p, "logo");

    let width = view.viewport_width;
    let is_mobile = width.is_some_and(|w| w < 768.0);
    let is_drawer = layout_mode == "drawer";
    let off_canvas = is_mobile || is_drawer;
    let is_rail = !is_drawer && width.is_some_and(|w| (768.0..1024.0).contains(&w));
    let open_now = sidebar_open() && off_canvas;
    let show_sidebar = layout_mode == "sidebar" || is_drawer;
    let show_header = layout_mode != "minimal";
    let show_bottom_nav = layout_mode == "bottomnav";
    let is_top_nav = layout_mode == "topnav";

    let root_class = box_class(&format!(
        "@container/dashboard w-full bg-background surface-page flex flex-row items-stretch {}",
        if full_bleed {
            "h-screen overflow-hidden"
        } else {
            "min-h-screen"
        }
    ));

    let skip = registry.render_spec(
        &pattern_spec(
            "button",
            [
                ("variant", Value::string("link")),
                ("href", Value::string("#main-content")),
                ("label", Value::string("Skip to content")),
                (
                    "className",
                    Value::string("sr-only focus:not-sr-only focus:fixed focus:top-2 focus:start-2 focus:z-50 focus:px-3 focus:py-2 focus:bg-card focus:shadow-elevation-popover"),
                ),
            ],
        ),
        None,
        &view,
    );

    let brand_mark = |size_class: &str, text_class: &str| -> Element {
        match logo {
            Some(node) => view.render_node(registry, node),
            None => {
                let mark = registry.render_spec(
                    &text_spec(&initial, "small", text_class, &[]),
                    None,
                    &view,
                );
                rsx! { div { class: "{box_class(size_class)}", {mark} } }
            }
        }
    };

    let sidebar = if show_sidebar {
        let aside_class = box_class(&format!(
            "z-30 flex-shrink-0 bg-card surface-material border-e-[length:var(--border-width)] border-border {} flex flex-col",
            if is_rail { "w-16" } else { "w-64" }
        ));
        let aside_style = if off_canvas {
            format!(
                "position: fixed; top: 0; bottom: 0; left: 0; transform: {}; transition: transform 200ms ease-in-out;",
                if open_now { "translateX(0)" } else { "translateX(-100%)" }
            )
        } else {
            "position: static; transform: none;".to_string()
        };
        let head_class = stack_class(
            true,
            "gap-4",
            "items-center",
            if is_rail {
                "justify-center"
            } else {
                "justify-between"
            },
            &format!(
                "h-16 border-b-[length:var(--border-width)] border-border {}",
                if is_rail { "px-2" } else { "px-4" }
            ),
        );
        let mark = brand_mark(
            "w-8 h-8 bg-primary rounded-interactive flex items-center justify-center",
            "text-primary-foreground font-bold text-sm",
        );
        let brand_name = (!is_rail).then(|| {
            registry.render_spec(
                &text_spec(&app_name, "label", "heading-voice text-foreground", &[]),
                None,
                &view,
            )
        });
        let brand = nav_anchor(
            "flex items-center gap-2".to_string(),
            "/",
            false,
            is_rail.then(|| app_name.clone()),
            None,
            navigate,
            None,
            rsx! { {mark} if let Some(brand_name) = brand_name { {brand_name} } },
        );
        let close_icon = registry.render_spec(&icon_spec("x", "h-5 w-5"), None, &view);
        let close_class = ghost_button_class("p-2");
        let nav_class = stack_class(
            false,
            "gap-0",
            "items-stretch",
            "justify-start",
            &format!(
                "py-4 space-y-1 overflow-y-auto {} {}",
                if slot(p, "sidebarContent").is_some() && !is_rail {
                    "shrink-0"
                } else {
                    "flex-1"
                },
                if is_rail { "px-2" } else { "px-3" }
            ),
        );
        let links: Vec<Element> = nav_items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                rsx! {
                    NavLink {
                        key: "{index}:{get_string_or(item, \"href\", \"\")}",
                        item: item.clone(),
                        active_href: active.clone(),
                        compact: is_rail,
                        view: props.view.clone(),
                        registry: props.registry.clone(),
                    }
                }
            })
            .collect();
        let content = slot(p, "sidebarContent")
            .filter(|_| !is_rail)
            .map(|node| view.render_node(registry, node));
        let footer = slot(p, "sidebarFooter").map(|node| view.render_node(registry, node));
        let inert = (off_canvas && !open_now).then_some("");
        rsx! {
            aside { class: "{aside_class}", style: "{aside_style}", "inert": inert,
                div { class: "{head_class}",
                    {brand}
                    if off_canvas {
                        button { class: "{close_class}", r#type: "button", "aria-label": "Close panel",
                            onclick: move |_| sidebar_open.set(false),
                            span { class: "{BUTTON_CONTENT}", {close_icon} }
                        }
                    }
                }
                nav { class: "{nav_class}", "aria-label": "Main navigation",
                    for link in links { {link} }
                }
                if let Some(content) = content {
                    div { class: box_class("flex-1 overflow-y-auto border-t-[length:var(--border-width)] border-border px-2 py-3"), {content} }
                }
                if let Some(footer) = footer {
                    div { class: box_class("p-4 border-t-[length:var(--border-width)] border-border"), {footer} }
                }
            }
        }
    } else {
        rsx! {}
    };

    let header = if show_header {
        let header_class = box_class(
            "sticky top-0 z-20 pt-[env(safe-area-inset-top)] bg-card surface-material border-b-[length:var(--border-width)] border-border",
        );
        let row_class = stack_class(
            true,
            "gap-4",
            "items-center",
            "justify-between",
            "h-16 px-3 @sm/dashboard:px-4 gap-2 @sm/dashboard:gap-4",
        );
        let menu_class = ghost_button_class(
            "p-2 rounded-interactive hover:bg-muted text-muted-foreground touch-manipulation min-h-[44px] min-w-[44px] flex items-center justify-center",
        );
        let menu_icon = registry.render_spec(&icon_spec("menu", "h-5 w-5"), None, &view);
        let phone_title = (is_mobile && !search_open()).then(|| {
            let spec = pattern_spec(
                "typography",
                [
                    ("content", Value::string(app_name.as_str())),
                    ("variant", Value::string("label")),
                    ("weight", Value::string("medium")),
                    ("truncate", Value::Bool(true)),
                    ("className", Value::string("flex-1 min-w-0 text-foreground")),
                ],
            );
            registry.render_spec(&spec, None, &view)
        });
        let drawer_brand = (is_drawer && !is_mobile).then(|| {
            let name = registry.render_spec(
                &text_spec(&app_name, "label", "heading-voice text-foreground", &[]),
                None,
                &view,
            );
            nav_anchor(
                "flex items-center gap-2 shrink-0".to_string(),
                "/",
                false,
                None,
                None,
                navigate,
                None,
                rsx! {
                    if let Some(node) = logo { {view.render_node(registry, node)} }
                    {name}
                },
            )
        });
        let top_nav = (is_top_nav && !is_mobile).then(|| {
            let nav_class = stack_class(
                true,
                "gap-0",
                "items-center",
                "justify-start",
                "flex items-center gap-1 overflow-x-auto",
            );
            let mark = brand_mark(
                "w-7 h-7 bg-primary rounded-interactive flex items-center justify-center",
                "text-primary-foreground font-bold text-xs",
            );
            let name = registry.render_spec(
                &text_spec(&app_name, "label", "heading-voice text-foreground", &[]),
                None,
                &view,
            );
            let brand = nav_anchor(
                "flex items-center gap-2 mr-3 shrink-0".to_string(),
                "/",
                false,
                None,
                None,
                navigate,
                None,
                rsx! { {mark} {name} },
            );
            let entries: Vec<Element> = nav_items
                .iter()
                .map(|item| {
                    let spec = pattern_spec(
                        "top-nav-item",
                        [
                            ("item", item.clone()),
                            (
                                "activeHref",
                                Value::string(active.clone().unwrap_or_default()),
                            ),
                            ("tone", Value::string("solid")),
                        ],
                    );
                    registry.render_spec(&spec, None, &view)
                })
                .collect();
            rsx! {
                nav { class: "{nav_class}", "aria-label": "Main navigation",
                    {brand}
                    for entry in entries { {entry} }
                }
            }
        });
        let search_box = search_enabled.then(|| {
            let mut fields = vec![
                ("inputType", Value::string("search")),
                ("leftIcon", Value::string("search")),
                ("placeholder", Value::string("Search...")),
                ("className", Value::string("w-full")),
            ];
            if let Some(event) = &search_event {
                fields.push(("action", Value::string(event.as_str())));
            }
            let input = registry.render_spec(&pattern_spec("input", fields), None, &view);
            let class = box_class(&format!(
                "flex-1 min-w-0 @xl/dashboard:max-w-md {}",
                if is_mobile && !search_open() {
                    "hidden"
                } else {
                    "block"
                }
            ));
            rsx! { div { class: "{class}", "data-dashboard-search": "", {input} } }
        });
        let search_toggle = (search_enabled && is_mobile).then(|| {
            let icon = registry.render_spec(
                &icon_spec(
                    if search_open() { "x" } else { "search" },
                    "h-icon-default w-icon-default",
                ),
                None,
                &view,
            );
            let class = ghost_button_class("shrink-0 min-h-[44px] min-w-[44px]");
            rsx! {
                button { class: "{class}", r#type: "button", "aria-label": "Search",
                    "aria-expanded": if search_open() { "true" } else { "false" },
                    onclick: move |_| { let next = !search_open(); search_open.set(next); },
                    span { class: "{BUTTON_CONTENT}", span { class: "flex-shrink-0", {icon} } }
                }
            }
        });
        let spacer = (!search_enabled).then(|| rsx! { div { class: box_class("flex-1") } });
        let right_class = stack_class(true, "gap-1", "items-center", "justify-start", "");
        let header_actions = slot(p, "headerActions").map(|node| view.render_node(registry, node));
        let theme_toggle = show_theme_toggle.then(|| {
            registry.render_spec(
                &pattern_spec("theme-toggle", [("size", Value::string("md"))]),
                None,
                &view,
            )
        });
        let bar_actions: Vec<Element> = top_bar_actions
            .iter()
            .map(|action| {
                let label = get_string(action, "label");
                let icon_name = get_string(action, "icon");
                let aria = label.clone().or_else(|| icon_name.clone());
                let icon = icon_name.as_ref().map(|name| {
                    registry.render_spec(&icon_spec(name, "h-5 w-5 text-muted-foreground"), None, &view)
                });
                let badge = badge_text(action).map(|text| {
                    let tone = match get_string(action, "variant").as_deref() {
                        Some("danger") => "bg-error text-error-foreground",
                        Some("primary") => "bg-primary text-primary-foreground",
                        _ => "bg-foreground text-background",
                    };
                    (
                        box_class(&format!(
                            "absolute -top-0.5 -right-0.5 min-w-[18px] h-[18px] px-1 rounded-full text-xs font-semibold flex items-center justify-center {tone}"
                        )),
                        text,
                    )
                });
                let event = get_string(action, "event");
                let route = get_string(action, "navigatesTo");
                let class = ghost_button_class("relative p-2");
                rsx! {
                    button { class: "{class}", r#type: "button", "aria-label": aria.as_deref(),
                        onclick: move |_| {
                            if let (Some(sink), Some(event)) = (sink, &event) {
                                sink.call((event.clone(), empty_payload()));
                            }
                            if let (Some(navigate), Some(route)) = (navigate, &route) {
                                navigate.call(route.clone());
                            }
                        },
                        span { class: "{BUTTON_CONTENT}",
                            if let Some(icon) = icon { {icon} }
                            if let Some((badge_class, text)) = badge { span { class: "{badge_class}", "{text}" } }
                        }
                    }
                }
            })
            .collect();
        let bell = unread.map(|count| {
            let icon = registry.render_spec(&icon_spec("bell", "h-5 w-5 text-muted-foreground"), None, &view);
            let class = ghost_button_class("relative p-2");
            let event = get_string(p, "notificationClickEvent").filter(|event| !event.is_empty());
            let badge_class = box_class(
                "absolute -top-0.5 -right-0.5 min-w-[18px] h-[18px] px-1 bg-error rounded-full text-xs font-semibold text-error-foreground flex items-center justify-center",
            );
            let shown = if count > 99 { "99+".to_string() } else { count.to_string() };
            rsx! {
                button { class: "{class}", r#type: "button", "aria-label": "Notifications",
                    onclick: move |_| {
                        if let (Some(sink), Some(event)) = (sink, &event) {
                            sink.call((event.clone(), empty_payload()));
                        }
                    },
                    span { class: "{BUTTON_CONTENT}",
                        {icon}
                        if count > 0 { span { class: "{badge_class}", "{shown}" } }
                    }
                }
            }
        });
        let user_menu = p
            .get("user")
            .filter(|user| !get_string_or(user, "name", "").is_empty())
            .map(|user| {
                let name = get_string_or(user, "name", "");
                let email = get_string(user, "email").filter(|email| !email.is_empty());
                let mut header_children =
                    vec![text_spec(&name, "small", "", &[("weight", "medium")])];
                if let Some(email) = &email {
                    header_children.push(text_spec(email, "caption", "", &[("color", "muted")]));
                }
                let header_spec = pattern_spec(
                    "vstack",
                    [
                        ("gap", Value::string("none")),
                        ("children", Value::array(header_children)),
                    ],
                );
                let mut avatar_fields = vec![
                    ("alt", Value::string(name.as_str())),
                    ("initials", Value::string(initials_of(&name))),
                    ("size", Value::string("sm")),
                ];
                if let Some(src) = get_string(user, "avatar") {
                    avatar_fields.push(("src", Value::string(src)));
                }
                let trigger = pattern_spec(
                    "flex",
                    [
                        (
                            "className",
                            Value::string(ghost_button_class("flex items-center gap-2 p-2")),
                        ),
                        (
                            "children",
                            Value::array(vec![
                                pattern_spec("avatar", avatar_fields),
                                text_spec(
                                    &name,
                                    "small",
                                    "hidden @sm/dashboard:block text-foreground",
                                    &[("weight", "medium")],
                                ),
                                icon_spec(
                                    "chevron-down",
                                    "hidden @sm/dashboard:block h-4 w-4 text-muted-foreground",
                                ),
                            ]),
                        ),
                    ],
                );
                let mut items: Vec<Value> = user_menu_items
                    .iter()
                    .enumerate()
                    .map(|(index, action)| action_item(action, index))
                    .collect();
                let mut sign_out = vec![
                    ("id", Value::string("sign-out")),
                    ("label", Value::string("Sign out")),
                    ("icon", Value::string("log-out")),
                    ("variant", Value::string("danger")),
                ];
                if let Some(event) = get_string(p, "onSignOut").filter(|event| !event.is_empty()) {
                    sign_out.push(("event", Value::string(event)));
                }
                let mut sign_out_map = ValueMap::new();
                for (key, value) in sign_out {
                    sign_out_map.insert(key.into(), value);
                }
                items.push(Value::Object(sign_out_map.into()));
                let menu = pattern_spec(
                    "menu",
                    [
                        ("position", Value::string("bottom-end")),
                        ("header", header_spec),
                        ("trigger", trigger),
                        ("items", Value::array(items)),
                    ],
                );
                registry.render_spec(&menu, None, &view)
            });
        rsx! {
            header { class: "{header_class}",
                div { class: "{row_class}",
                    if show_sidebar && off_canvas {
                        button { class: "{menu_class}", r#type: "button", "aria-label": "Open sidebar",
                            onclick: move |_| sidebar_open.set(true),
                            span { class: "{BUTTON_CONTENT}", {menu_icon} }
                        }
                    }
                    if let Some(title) = phone_title { {title} }
                    if let Some(brand) = drawer_brand { {brand} }
                    if let Some(top_nav) = top_nav { {top_nav} }
                    if let Some(search_box) = search_box { {search_box} }
                    if let Some(search_toggle) = search_toggle { {search_toggle} }
                    if let Some(spacer) = spacer { {spacer} }
                    div { class: "{right_class}",
                        if let Some(actions) = header_actions { {actions} }
                        if let Some(toggle) = theme_toggle { {toggle} }
                        for entry in bar_actions { {entry} }
                        if let Some(bell) = bell { {bell} }
                        if let Some(menu) = user_menu { {menu} }
                    }
                }
            }
        }
    } else {
        rsx! {}
    };

    let children = child_patterns(p, view.trait_content);
    let column_class = stack_class(
        false,
        "gap-0",
        "items-stretch",
        "justify-start",
        &format!(
            "flex-1 min-w-0 {}",
            if full_bleed {
                "h-full min-h-0"
            } else {
                "min-h-screen"
            }
        ),
    );
    let main_class = box_class(&format!(
        "{} {}",
        if full_bleed {
            "flex-1 min-h-0 flex flex-col overflow-y-auto focus:outline-none"
        } else {
            "flex-1 p-3 @sm/dashboard:p-4 @md/dashboard:p-6 focus:outline-none"
        },
        if show_bottom_nav {
            "pb-[calc(5rem+env(safe-area-inset-bottom))]"
        } else {
            ""
        }
    ));
    let transition_class = box_class(&format!(
        "animate-page-in {}",
        if full_bleed {
            "w-full flex-1 min-h-0"
        } else {
            "mx-auto w-full max-w-[1440px]"
        }
    ));
    let bottom_nav = show_bottom_nav.then(|| {
        let nav_class = box_class(
            "fixed bottom-0 inset-x-0 z-20 pb-[env(safe-area-inset-bottom)] bg-card surface-material border-t-[length:var(--border-width)] border-border",
        );
        let row = stack_class(true, "gap-4", "items-center", "justify-around", "h-16 px-2");
        let links: Vec<Element> = nav_items
            .iter()
            .map(|item| bottom_link(item, active.as_deref(), navigate, &view, registry))
            .collect();
        rsx! {
            nav { class: "{nav_class}", "aria-label": "Main navigation",
                div { class: "{row}", for link in links { {link} } }
            }
        }
    });

    rsx! {
        div { class: "{root_class}",
            {skip}
            if show_sidebar && off_canvas && open_now {
                div { class: box_class("fixed inset-0 bg-scrim z-20"), onclick: move |_| sidebar_open.set(false) }
            }
            {sidebar}
            div { class: "{column_class}",
                {header}
                main { class: "{main_class}", id: "main-content", tabindex: "-1",
                    div { class: "{transition_class}", "data-page-transition": "owner",
                        for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
                    }
                }
                if let Some(bottom_nav) = bottom_nav { {bottom_nav} }
            }
        }
    }
}

impl DioxusPatternRenderer for DashboardLayoutRenderer {
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
        rsx! { DashboardLayout { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
