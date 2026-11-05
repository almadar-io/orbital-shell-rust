//! Native Dioxus body for the `header` pattern, ported from `Header.tsx`.

use crate::components::core::templates::dashboard_layout::{box_class, ghost_button_class, slot, stack_class, truthy, BUTTON_CONTENT};
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{emit_empty_event, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `header` pattern.
pub struct HeaderRenderer;

impl HeaderRenderer {
    const NAME: &str = "header";
}

fn look_style(look: &str) -> &'static str {
    match look {
        "hero" => "py-section min-h-[200px] [&_h1]:text-display-1",
        "breadcrumb" => "py-2 text-sm [&_h1]:text-base [&_h1]:font-medium",
        "contextual" => "py-3 [&_h1]:text-lg",
        "editorial-banner" => {
            "py-12 [&_h1]:text-display-2 [&_h1]:font-display-bold border-b border-border"
        }
        _ => "",
    }
}

fn logo_image(src: &str, alt: &str) -> Value {
    pattern_spec(
        "image",
        [
            ("src", Value::string(src)),
            ("alt", Value::string(alt)),
            ("fit", Value::string("contain")),
            ("rounded", Value::string("none")),
            ("loading", Value::string("eager")),
            ("className", Value::string("h-8 w-auto max-w-32")),
        ],
    )
}

impl DioxusPatternRenderer for HeaderRenderer {
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
        let brand_name = get_string_or(props, "brandName", "KFlow");
        let sticky = get_bool_or(props, "sticky", true);
        let variant = get_string_or(props, "variant", "mobile");
        let look = get_string_or(props, "look", "compact-bar");
        let show_menu_toggle = get_bool_or(props, "showMenuToggle", true);
        let is_menu_open = get_bool_or(props, "isMenuOpen", false);
        let show_search = get_bool_or(props, "showSearch", false);
        let on_menu_toggle = get_string(props, "onMenuToggle");
        let on_user_click = get_string(props, "onUserClick");
        let on_logo_click = get_string(props, "onLogoClick").filter(|event| !event.is_empty());
        let brand_href = get_string(props, "brandHref").filter(|href| !href.is_empty());
        let sink = view.sink;
        let navigate = view.navigate;

        let class = box_class(&crate::renderer::cn(&[
            "h-16 border-b border-border flex items-center px-4 justify-between bg-card",
            if sticky { "sticky top-0 z-50" } else { "" },
            if variant == "mobile" { "lg:hidden" } else { "" },
            look_style(&look),
            &get_string_or(props, "className", ""),
        ]));

        let logo_el = match slot(props, "logo") {
            Some(Value::String(src)) => {
                Some(registry.render_spec(&logo_image(src, &brand_name), None, view))
            }
            Some(node) => Some(view.render_node(registry, node)),
            None => get_string(props, "logoSrc")
                .filter(|src| !src.is_empty())
                .map(|src| registry.render_spec(&logo_image(&src, &brand_name), None, view)),
        };
        let name_el = (!brand_name.is_empty()).then(|| {
            registry.render_spec(
                &pattern_spec(
                    "typography",
                    [
                        ("content", Value::string(brand_name.as_str())),
                        ("variant", Value::string("h5")),
                        ("className", Value::string("text-lg text-foreground")),
                    ],
                ),
                None,
                view,
            )
        });
        let brand_content = rsx! {
            if let Some(logo_el) = logo_el { {logo_el} }
            if let Some(name_el) = name_el { {name_el} }
        };

        let left_class = stack_class(true, "gap-0", "items-center", "justify-start", "gap-3");
        let toggle = show_menu_toggle.then(|| {
            let icon = registry.render_spec(
                &pattern_spec(
                    "icon",
                    [
                        (
                            "name",
                            Value::string(if is_menu_open { "x" } else { "menu" }),
                        ),
                        ("size", Value::string("lg")),
                    ],
                ),
                None,
                view,
            );
            let toggle_class =
                ghost_button_class("p-2 -ml-2 text-foreground hover:bg-muted transition-colors");
            let label = if is_menu_open {
                "Close menu"
            } else {
                "Open menu"
            };
            let event = on_menu_toggle.clone();
            rsx! {
                button { class: "{toggle_class}", r#type: "button", "aria-label": "{label}",
                    onclick: move |_| emit_empty_event(sink, event.as_deref()),
                    span { class: "{BUTTON_CONTENT}", {icon} }
                }
            }
        });

        let brand = if let Some(href) = brand_href {
            let brand_class = ghost_button_class("gap-2 px-0 hover:bg-transparent");
            let target = href.clone();
            rsx! {
                a { class: "{brand_class}", href: "{href}", "aria-label": "{brand_name}",
                    onclick: move |event| {
                        if let Some(navigate) = navigate {
                            if target.starts_with('/') || target.starts_with('#') {
                                event.prevent_default();
                                navigate.call(target.clone());
                            }
                        }
                    },
                    span { class: "{BUTTON_CONTENT}", {brand_content} }
                }
            }
        } else {
            let brand_class = stack_class(
                true,
                "gap-0",
                "items-center",
                "justify-start",
                if on_logo_click.is_some() {
                    "gap-2 cursor-pointer"
                } else {
                    "gap-2"
                },
            );
            let pressable = on_logo_click.is_some();
            let label = on_logo_click.is_some().then(|| brand_name.clone());
            let event = on_logo_click.clone();
            rsx! {
                div { class: "{brand_class}",
                    role: pressable.then_some("button"),
                    tabindex: pressable.then_some("0"),
                    "aria-label": label.as_deref(),
                    onclick: move |_| emit_empty_event(sink, event.as_deref()),
                    {brand_content}
                }
            }
        };

        let nav_items = get_array(props, "navigationItems");
        let center = (variant == "desktop" && !nav_items.is_empty()).then(|| {
            let entries: Vec<Element> = nav_items
                .iter()
                .map(|item| {
                    registry.render_spec(
                        &pattern_spec(
                            "top-nav-item",
                            [("item", item.clone()), ("tone", Value::string("soft"))],
                        ),
                        None,
                        view,
                    )
                })
                .collect();
            rsx! {
                div { class: box_class("hidden md:flex items-center gap-1 flex-1 justify-center"), role: "navigation",
                    for entry in entries { {entry} }
                }
            }
        });

        let search = show_search.then(|| {
            let mut fields = vec![(
                "placeholder",
                Value::string(get_string_or(props, "searchPlaceholder", "Search...")),
            )];
            if let Some(event) = get_string(props, "onSearch").filter(|event| !event.is_empty()) {
                fields.push(("event", Value::string(event)));
            }
            let input = registry.render_spec(&pattern_spec("search-input", fields), None, view);
            rsx! { div { class: box_class("hidden lg:block flex-1 max-w-md mx-4"), {input} } }
        });

        let actions = slot(props, "actions").map(|node| view.render_node(registry, node));
        let avatar = props.get("userAvatar").filter(|value| truthy(value));
        let user_name = get_string(props, "userName").filter(|name| !name.is_empty());
        let user_button = (avatar.is_some() || user_name.is_some()).then(|| {
            let clickable = on_user_click.is_some();
            let button_class = ghost_button_class(&format!(
                "w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary font-bold text-xs hover:ring-2 hover:ring-ring transition-all{}",
                if clickable { " cursor-pointer" } else { "" }
            ));
            let src = avatar
                .and_then(|a| get_string(a, "src"))
                .filter(|src| !src.is_empty());
            let initials = avatar
                .and_then(|a| get_string(a, "initials"))
                .filter(|text| !text.is_empty())
                .or_else(|| {
                    user_name
                        .as_ref()
                        .and_then(|name| name.chars().next())
                        .map(|c| c.to_uppercase().to_string())
                })
                .unwrap_or_else(|| "U".to_string());
            let visual = src.map(|src| {
                let alt = avatar
                    .and_then(|a| get_string(a, "alt"))
                    .filter(|alt| !alt.is_empty())
                    .or_else(|| user_name.clone())
                    .unwrap_or_default();
                registry.render_spec(
                    &pattern_spec(
                        "avatar",
                        [
                            ("src", Value::string(src)),
                            ("alt", Value::string(alt)),
                            ("size", Value::string("sm")),
                        ],
                    ),
                    None,
                    view,
                )
            });
            let event = on_user_click.clone();
            rsx! {
                button { class: "{button_class}", r#type: "button",
                    onclick: move |_| emit_empty_event(sink, event.as_deref()),
                    span { class: "{BUTTON_CONTENT}",
                        if let Some(visual) = visual { {visual} } else { "{initials}" }
                    }
                }
            }
        });

        rsx! {
            header { class: "{class}", ..shared_root_attrs(props),
                div { class: "{left_class}",
                    if let Some(toggle) = toggle { {toggle} }
                    {brand}
                }
                if let Some(center) = center { {center} }
                if let Some(search) = search { {search} }
                div { class: "{left_class}",
                    if let Some(actions) = actions { {actions} }
                    if let Some(user_button) = user_button { {user_button} }
                }
            }
        }
    }
}
