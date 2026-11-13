//! Native Dioxus renderer for the `alert` pattern, the web `Alert` molecule.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    cn, passthrough_attrs, pattern_spec, DioxusPatternRenderer, DioxusView, BOX_ATOM_BASE,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `alert` pattern.
pub struct AlertRenderer;

impl AlertRenderer {
    const NAME: &str = "alert";
}

impl DioxusPatternRenderer for AlertRenderer {
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
        let variant = get_string_or(props, "variant", "info");
        let (border, icon_color, default_icon) = match variant.as_str() {
            "success" => ("border-success", "text-success", "check-circle"),
            "warning" => ("border-warning", "text-warning", "alert-triangle"),
            "error" => ("border-error", "text-error", "alert-circle"),
            _ => ("border-info", "text-info", "info"),
        };
        let urgent = variant == "error" || variant == "warning";
        let class = cn(&[
            "p-4 bg-card border-[length:var(--border-width)] border-border",
            BOX_ATOM_BASE,
            "rounded-container shadow-elevation-card",
            border,
            &get_string_or(props, "className", ""),
        ]);
        let mut attrs = passthrough_attrs(props, &[]);
        attrs.retain(|attr| attr.name != "role");
        let icon_name = get_string(props, "icon").filter(|name| !name.is_empty());
        let icon = pattern_spec(
            "icon",
            [
                (
                    "name",
                    Value::string(icon_name.unwrap_or_else(|| default_icon.to_string())),
                ),
                ("size", Value::string("md")),
                ("className", Value::string(icon_color)),
            ],
        );
        let title = get_string(props, "title")
            .filter(|title| !title.is_empty())
            .map(|title| {
                pattern_spec(
                    "typography",
                    [
                        ("variant", Value::string("h6")),
                        ("className", Value::string("mb-1")),
                        ("content", Value::string(title)),
                    ],
                )
            });
        let children = props.get("children").filter(|children| !children.is_null());
        let message = match children {
            Some(children) => pattern_spec(
                "typography",
                [
                    ("variant", Value::string("body2")),
                    ("children", children.clone()),
                ],
            ),
            None => {
                let text = props
                    .get("message")
                    .filter(|message| !message.is_null())
                    .cloned();
                pattern_spec(
                    "typography",
                    [
                        ("variant", Value::string("body2")),
                        ("content", text.unwrap_or_else(|| Value::string(""))),
                    ],
                )
            }
        };
        let actions = props.get("actions").filter(|actions| !actions.is_null());
        let dismiss_event = ["dismissEvent", "onDismiss", "onClose"]
            .iter()
            .find_map(|key| get_string(props, key).filter(|name| !name.is_empty()));
        let dismissible = get_bool_or(props, "dismissible", false) || dismiss_event.is_some();
        let mut dismiss_fields = vec![
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("icon", Value::string("x")),
            ("className", Value::string("flex-shrink-0")),
            (
                "aria-label",
                Value::string(crate::i18n::t("aria.closeAlert")),
            ),
        ];
        if let Some(event) = dismiss_event {
            dismiss_fields.push(("action", Value::string(event)));
        }
        let dismiss = pattern_spec("button", dismiss_fields);
        let role = if urgent { "alert" } else { "status" };
        let icon_box = cn(&[BOX_ATOM_BASE, "flex-shrink-0 mt-0.5"]);
        let body_box = cn(&[BOX_ATOM_BASE, "flex-1 min-w-0"]);
        rsx! {
            div { class: "{class}", role: "{role}", "data-pattern": "alert", ..attrs,
                div { class: "flex flex-row gap-2 items-start justify-start", style: "height:auto",
                    div { class: "{icon_box}", {registry.render_spec(&icon, None, view)} }
                    div { class: "{body_box}",
                        if let Some(title) = title.as_ref() { {registry.render_spec(title, None, view)} }
                        {registry.render_spec(&message, None, view)}
                        if let Some(actions) = actions {
                            div { class: "flex flex-row gap-2 items-stretch justify-start flex-wrap mt-3", style: "height:auto",
                                {view.render_node(registry, actions)}
                            }
                        }
                    }
                    if dismissible { {registry.render_spec(&dismiss, None, view)} }
                }
            }
        }
    }
}
