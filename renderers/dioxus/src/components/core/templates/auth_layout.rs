//! Native Dioxus body for the `auth-layout` pattern, ported from `AuthLayout.tsx`.

use super::dashboard_layout::{box_class, slot, stack_class};
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `auth-layout` pattern.
pub struct AuthLayoutRenderer;

impl AuthLayoutRenderer {
    const NAME: &str = "auth-layout";
}

fn text(content: &str, variant: &str, class: &str) -> Value {
    pattern_spec(
        "typography",
        [
            ("content", Value::string(content)),
            ("variant", Value::string(variant)),
            ("className", Value::string(class)),
        ],
    )
}

fn home_link(
    class: &str,
    navigate: Option<crate::renderer::NavigationSink>,
    body: Element,
) -> Element {
    rsx! {
        a { class: "{class}", href: "/",
            onclick: move |event| {
                if let Some(navigate) = navigate {
                    event.prevent_default();
                    navigate.call("/".to_string());
                }
            },
            {body}
        }
    }
}

impl DioxusPatternRenderer for AuthLayoutRenderer {
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
        let app_name = get_string_or(props, "appName", "My App");
        let initial = app_name
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default();
        let show_branding = get_bool_or(props, "showBranding", true);
        let logo = slot(props, "logo");
        let navigate = view.navigate;
        let children = child_patterns(props, view.trait_content);

        let branding = show_branding.then(|| {
            let panel_class = stack_class(
                false,
                "gap-0",
                "items-stretch",
                "justify-start",
                "hidden lg:flex lg:w-1/2 bg-primary relative overflow-hidden justify-between p-12",
            );
            let panel_style = get_string(props, "backgroundImage")
                .filter(|image| !image.is_empty())
                .map(|image| format!("background-image: url({image}); background-size: cover;"))
                .unwrap_or_default();
            let mark = match logo {
                Some(node) => view.render_node(registry, node),
                None => {
                    let letter = registry.render_spec(
                        &text(&initial, "body1", "text-primary-foreground font-bold text-lg"),
                        None,
                        view,
                    );
                    rsx! { div { class: box_class("w-10 h-10 bg-primary-foreground/20 rounded-container flex items-center justify-center backdrop-blur"), {letter} } }
                }
            };
            let name = registry.render_spec(
                &text(&app_name, "body1", "text-2xl font-bold text-primary-foreground"),
                None,
                view,
            );
            let brand = home_link("flex items-center gap-3", navigate, rsx! { {mark} {name} });
            let copy = match slot(props, "brandingContent") {
                Some(node) => view.render_node(registry, node),
                None => {
                    let welcome = registry.render_spec(
                        &text(
                            &format!("Welcome to {app_name}"),
                            "h1",
                            "text-4xl text-primary-foreground leading-tight",
                        ),
                        None,
                        view,
                    );
                    let subtitle = registry.render_spec(
                        &text(
                            "Sign in to access your dashboard and manage your account.",
                            "body1",
                            "text-lg text-primary-foreground/80 max-w-md",
                        ),
                        None,
                        view,
                    );
                    let stack = stack_class(false, "gap-6", "items-stretch", "justify-start", "");
                    rsx! { div { class: "{stack}", {welcome} {subtitle} } }
                }
            };
            rsx! {
                div { class: "{panel_class}", style: "{panel_style}",
                    div { class: box_class("absolute inset-0 bg-gradient-to-br from-primary/90 to-primary-hover/90") }
                    div { class: box_class("relative z-10"), {brand} }
                    div { class: box_class("relative z-10"), {copy} }
                    div { class: box_class("absolute -bottom-32 -right-32 w-96 h-96 bg-primary-foreground/5 rounded-full") }
                    div { class: box_class("absolute -top-16 -right-16 w-64 h-64 bg-primary-foreground/5 rounded-full") }
                }
            }
        });

        let mobile_mark = match logo {
            Some(node) => view.render_node(registry, node),
            None => {
                let letter = registry.render_spec(
                    &text(
                        &initial,
                        "body1",
                        "text-primary-foreground font-bold text-xl",
                    ),
                    None,
                    view,
                );
                rsx! { div { class: box_class("w-12 h-12 bg-primary rounded-container flex items-center justify-center"), {letter} } }
            }
        };
        let mobile_name = registry.render_spec(
            &text(&app_name, "body1", "text-2xl font-bold text-foreground"),
            None,
            view,
        );
        let mobile_brand = home_link(
            "inline-flex items-center gap-3",
            navigate,
            rsx! { {mobile_mark} {mobile_name} },
        );

        rsx! {
            div { class: box_class("min-h-screen flex"), ..shared_root_attrs(props),
                if let Some(branding) = branding { {branding} }
                div { class: box_class("flex-1 flex items-center justify-center p-6 sm:p-12 bg-background surface-page"),
                    div { class: box_class("w-full max-w-md"),
                        div { class: box_class("lg:hidden mb-8 text-center"), {mobile_brand} }
                        for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
                    }
                }
            }
        }
    }
}
