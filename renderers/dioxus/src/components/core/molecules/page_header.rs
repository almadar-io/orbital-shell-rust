//! Native Dioxus body for the `page-header` pattern, ported from `PageHeader.tsx`.

use crate::components::core::templates::dashboard_layout::{box_class};
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::{Value, ValueMap};

/// Dioxus renderer for the `page-header` pattern.
pub struct PageHeaderRenderer;

impl PageHeaderRenderer {
    const NAME: &str = "page-header";
}

const MAX_VISIBLE_ACTIONS: usize = 2;

fn text_of(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.to_string()),
        Value::Int(number) => Some(number.to_string()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn menu_items(actions: &[Value], is_loading: bool) -> Value {
    let items = actions
        .iter()
        .map(|action| {
            let mut map = ValueMap::new();
            map.insert(
                "label".into(),
                Value::string(get_string_or(action, "label", "")),
            );
            if let Some(icon) = get_string(action, "icon") {
                map.insert("icon".into(), Value::string(icon));
            }
            map.insert(
                "disabled".into(),
                Value::Bool(get_bool_or(action, "disabled", false) || is_loading),
            );
            map.insert(
                "variant".into(),
                Value::string(
                    if get_string(action, "variant").as_deref() == Some("danger") {
                        "danger"
                    } else {
                        "default"
                    },
                ),
            );
            if let Some(event) = get_string(action, "event") {
                map.insert("event".into(), Value::string(event));
            }
            if let Some(route) = get_string(action, "navigatesTo") {
                map.insert("href".into(), Value::string(route));
            }
            Value::Object(map.into())
        })
        .collect::<Vec<_>>();
    Value::array(items)
}

fn overflow_menu(
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
    actions: &[Value],
    is_loading: bool,
) -> Element {
    let trigger = pattern_spec(
        "button",
        [
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("label", Value::string("")),
            ("icon", Value::string("more-horizontal")),
            (
                "aria-label",
                Value::string(crate::i18n::t("common.actions")),
            ),
        ],
    );
    let menu = pattern_spec(
        "menu",
        [
            ("position", Value::string("bottom-end")),
            ("trigger", trigger),
            ("items", menu_items(actions, is_loading)),
        ],
    );
    registry.render_spec(&menu, None, view)
}

impl DioxusPatternRenderer for PageHeaderRenderer {
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
        let class = box_class(&crate::renderer::cn(&[
            "w-full min-w-0",
            &get_string_or(props, "className", ""),
        ]));
        let is_loading = get_bool_or(props, "isLoading", false);
        let title = props.get("title").and_then(text_of).unwrap_or_default();
        let subtitle = props
            .get("subtitle")
            .and_then(text_of)
            .filter(|text| !text.is_empty());
        let status = match props.get("status") {
            Some(Value::String(label)) if !label.is_empty() => {
                Some((label.to_string(), "default".to_string()))
            }
            Some(object @ Value::Object(_)) => Some((
                get_string_or(object, "label", ""),
                get_string(object, "variant")
                    .filter(|variant| !variant.is_empty())
                    .unwrap_or_else(|| "default".to_string()),
            )),
            _ => None,
        };

        let crumbs = get_array(props, "breadcrumbs");
        let breadcrumb = (!crumbs.is_empty()).then(|| {
            let items = crumbs
                .iter()
                .map(|crumb| {
                    let mut map = ValueMap::new();
                    map.insert(
                        "label".into(),
                        Value::string(get_string_or(crumb, "label", "")),
                    );
                    if let Some(href) = get_string(crumb, "href") {
                        map.insert("href".into(), Value::string(href));
                    }
                    Value::Object(map.into())
                })
                .collect::<Vec<_>>();
            registry.render_spec(
                &pattern_spec(
                    "breadcrumb",
                    [
                        ("items", Value::array(items)),
                        ("className", Value::string("mb-4")),
                    ],
                ),
                None,
                view,
            )
        });

        let back = get_bool_or(props, "showBack", false).then(|| {
            registry.render_spec(
                &pattern_spec(
                    "button",
                    [
                        ("variant", Value::string("ghost")),
                        ("label", Value::string("")),
                        ("icon", Value::string("arrow-left")),
                        ("aria-label", Value::string(crate::i18n::t("common.back"))),
                        ("className", Value::string("mt-1 p-2 rtl:-scale-x-100")),
                        (
                            "action",
                            Value::string(get_string_or(props, "backEvent", "BACK")),
                        ),
                    ],
                ),
                None,
                view,
            )
        });
        let icon = get_string(props, "icon")
            .filter(|name| !name.is_empty())
            .map(|name| {
                registry.render_spec(
                    &pattern_spec(
                        "icon",
                        [
                            ("name", Value::string(name)),
                            ("size", Value::string("lg")),
                            ("className", Value::string("shrink-0 text-muted-foreground")),
                        ],
                    ),
                    None,
                    view,
                )
            });
        let heading = registry.render_spec(
            &pattern_spec(
                "typography",
                [
                    ("content", Value::string(title)),
                    ("variant", Value::string("h1")),
                    ("className", Value::string("text-2xl text-foreground")),
                ],
            ),
            None,
            view,
        );
        let status_badge = status.map(|(label, variant)| {
            registry.render_spec(
                &pattern_spec(
                    "badge",
                    [
                        ("label", Value::string(label)),
                        ("variant", Value::string(variant)),
                        ("size", Value::string("sm")),
                    ],
                ),
                None,
                view,
            )
        });
        let sub = subtitle.map(|text| {
            registry.render_spec(
                &pattern_spec(
                    "typography",
                    [
                        ("content", Value::string(text)),
                        ("variant", Value::string("body")),
                        ("color", Value::string("muted")),
                        ("className", Value::string("mt-1 text-sm")),
                    ],
                ),
                None,
                view,
            )
        });

        let actions = get_array(props, "actions");
        let action_row = (!actions.is_empty()).then(|| {
            let buttons: Vec<Element> = actions
                .iter()
                .take(MAX_VISIBLE_ACTIONS)
                .enumerate()
                .map(|(index, action)| {
                    let variant = get_string(action, "variant")
                        .filter(|variant| !variant.is_empty())
                        .unwrap_or_else(|| {
                            (if index == 0 { "primary" } else { "secondary" }).to_string()
                        });
                    let mut fields = vec![
                        ("label", Value::string(get_string_or(action, "label", ""))),
                        ("variant", Value::string(variant)),
                        (
                            "isLoading",
                            Value::Bool(get_bool_or(action, "loading", false) || is_loading),
                        ),
                        (
                            "disabled",
                            Value::Bool(get_bool_or(action, "disabled", false)),
                        ),
                    ];
                    if let Some(event) = get_string(action, "event") {
                        fields.push(("action", Value::string(event)));
                    }
                    if let Some(route) = get_string(action, "navigatesTo") {
                        fields.push(("href", Value::string(route)));
                    }
                    if let Some(icon) = get_string(action, "icon").filter(|icon| !icon.is_empty()) {
                        fields.push(("leftIcon", Value::string(icon)));
                    }
                    if index > 0 {
                        fields.push(("className", Value::string("hidden sm:inline-flex")));
                    }
                    registry.render_spec(&pattern_spec("button", fields), None, view)
                })
                .collect();
            let compact = (actions.len() > 1).then(|| {
                let menu = overflow_menu(registry, view, &actions[1..], is_loading);
                rsx! { div { class: box_class("sm:hidden"), {menu} } }
            });
            let wide = (actions.len() > MAX_VISIBLE_ACTIONS).then(|| {
                let menu =
                    overflow_menu(registry, view, &actions[MAX_VISIBLE_ACTIONS..], is_loading);
                rsx! { div { class: box_class("hidden sm:block"), {menu} } }
            });
            rsx! {
                div { class: box_class("flex items-center gap-2 shrink-0"),
                    for button in buttons { {button} }
                    if let Some(compact) = compact { {compact} }
                    if let Some(wide) = wide { {wide} }
                }
            }
        });

        let tabs = get_array(props, "tabs");
        let tab_bar = (!tabs.is_empty()).then(|| {
            let items = tabs
                .iter()
                .map(|tab| {
                    let mut map = ValueMap::new();
                    map.insert("id".into(), Value::string(get_string_or(tab, "value", "")));
                    map.insert(
                        "label".into(),
                        Value::string(get_string_or(tab, "label", "")),
                    );
                    if let Some(count) = tab.get("count") {
                        map.insert("badge".into(), count.clone());
                    }
                    Value::Object(map.into())
                })
                .collect::<Vec<_>>();
            let mut fields = vec![
                ("items", Value::array(items)),
                ("variant", Value::string("underline")),
                ("className", Value::string("mt-6")),
            ];
            if let Some(active) = get_string(props, "activeTab") {
                fields.push(("activeTab", Value::string(active)));
            }
            if let Some(event) = get_string(props, "onTabChange").filter(|event| !event.is_empty())
            {
                fields.push(("tabChangeEvent", Value::string(event)));
            }
            registry.render_spec(&pattern_spec("tabs", fields), None, view)
        });

        let children = child_patterns(props, view.trait_content);
        rsx! {
            div { class: "{class}", ..shared_root_attrs(props),
                if let Some(breadcrumb) = breadcrumb { {breadcrumb} }
                div { class: box_class("flex flex-wrap items-start justify-between gap-x-4 gap-y-3"),
                    div { class: box_class("flex items-start gap-4 min-w-0"),
                        if let Some(back) = back { {back} }
                        div { class: box_class("min-w-0"),
                            div { class: box_class("flex items-center gap-3"),
                                if let Some(icon) = icon { {icon} }
                                {heading}
                                if let Some(badge) = status_badge { {badge} }
                            }
                            if let Some(sub) = sub { {sub} }
                        }
                    }
                    if let Some(action_row) = action_row { {action_row} }
                }
                if let Some(tab_bar) = tab_bar { {tab_bar} }
                for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
            }
        }
    }
}
