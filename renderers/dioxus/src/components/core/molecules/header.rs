//! Native Dioxus body for the `header` pattern, ported from `Header.tsx`.

use crate::components::core::templates::dashboard_layout::{
    box_class, ghost_button_spec, slot, stack_class, truthy,
};
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

        let class = box_class(&crate::renderer::cn(&[
            "h-16 border-b border-border flex items-center px-4 justify-between bg-card",
            if sticky { "sticky top-0 z-50" } else { "" },
            if variant == "mobile" { "lg:hidden" } else { "" },
            look_style(&look),
            &get_string_or(props, "className", ""),
        ]));

        let logo_node: Option<Value> = match slot(props, "logo") {
            Some(Value::String(src)) => Some(logo_image(src, &brand_name)),
            Some(node) => Some(node.clone()),
            None => get_string(props, "logoSrc")
                .filter(|src| !src.is_empty())
                .map(|src| logo_image(&src, &brand_name)),
        };
        let name_node: Option<Value> = (!brand_name.is_empty()).then(|| {
            pattern_spec(
                "typography",
                [
                    ("content", Value::string(brand_name.as_str())),
                    ("variant", Value::string("h5")),
                    ("className", Value::string("text-lg text-foreground")),
                ],
            )
        });
        let brand_nodes: Vec<Value> = logo_node.into_iter().chain(name_node).collect();

        let left_class = stack_class(true, "gap-0", "items-center", "justify-start", "gap-3");
        let toggle = show_menu_toggle.then(|| {
            let icon_spec = pattern_spec(
                "icon",
                [
                    (
                        "name",
                        Value::string(if is_menu_open { "x" } else { "menu" }),
                    ),
                    ("size", Value::string("lg")),
                ],
            );
            let label = if is_menu_open {
                crate::i18n::t("aria.closeMenu")
            } else {
                crate::i18n::t("aria.openMenu")
            };
            let mut extra = vec![("aria-label", Value::string(label))];
            if let Some(event) = on_menu_toggle.as_ref() {
                extra.push(("action", Value::string(event.as_str())));
            }
            let spec = ghost_button_spec(
                "p-2 -ml-2 text-foreground hover:bg-muted transition-colors",
                vec![icon_spec],
                extra,
            );
            registry.render_spec(&spec, None, view)
        });

        let brand = if let Some(href) = brand_href {
            let spec = ghost_button_spec(
                "gap-2 px-0 hover:bg-transparent",
                brand_nodes.clone(),
                vec![
                    ("href", Value::string(href.as_str())),
                    ("aria-label", Value::string(brand_name.as_str())),
                ],
            );
            registry.render_spec(&spec, None, view)
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
            let brand_content = rsx! {
                for node in brand_nodes.iter() { {view.render_node(registry, node)} }
            };
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
                Value::string(get_string_or(
                    props,
                    "searchPlaceholder",
                    &crate::i18n::t("common.search"),
                )),
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
            let button_class = format!(
                "w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary font-bold text-xs hover:ring-2 hover:ring-ring transition-all{}",
                if clickable { " cursor-pointer" } else { "" }
            );
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
            let mut extra: Vec<(&str, Value)> = Vec::new();
            if let Some(event) = on_user_click.as_ref() {
                extra.push(("action", Value::string(event.as_str())));
            }
            let children = match src {
                Some(src) => {
                    let alt = avatar
                        .and_then(|a| get_string(a, "alt"))
                        .filter(|alt| !alt.is_empty())
                        .or_else(|| user_name.clone())
                        .unwrap_or_default();
                    vec![pattern_spec(
                        "avatar",
                        [
                            ("src", Value::string(src)),
                            ("alt", Value::string(alt)),
                            ("size", Value::string("sm")),
                        ],
                    )]
                }
                None => {
                    extra.push(("label", Value::string(initials.as_str())));
                    Vec::new()
                }
            };
            let spec = ghost_button_spec(&button_class, children, extra);
            registry.render_spec(&spec, None, view)
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
