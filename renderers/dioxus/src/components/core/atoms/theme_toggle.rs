//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs_with_defaults, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    from_json, get_bool_or, get_string, get_string_or, };
use orbital_core::{ThemeMode, Value};

/// Dioxus renderer for the `theme-toggle` pattern.
pub struct ThemeToggleRenderer;

impl ThemeToggleRenderer {
    const NAME: &str = "theme-toggle";
}

impl DioxusPatternRenderer for ThemeToggleRenderer {
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
        let size = get_string_or(props, "size", "md");
        let padding = match size.as_str() {
            "sm" => "p-1.5",
            "lg" => "p-2.5",
            _ => "p-2",
        };
        let class = merge_class_name(
            &format!(
                "inline-flex items-center justify-center gap-2 text-foreground hover:bg-muted border-[length:var(--border-width)] border-transparent hover:border-border focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 transition-colors duration-fast {padding}"
            ),
            props,
        );
        let is_dark = view.theme_mode == Some(ThemeMode::Dark);
        let icon_name = if is_dark { "sun" } else { "moon" };
        let icon = from_json(serde_json::json!({
            "type": "icon",
            "name": icon_name,
            "size": size,
            "className": "text-foreground"
        }));
        let switch_label = get_string(
            props,
            if is_dark {
                "switchToLightLabel"
            } else {
                "switchToDarkLabel"
            },
        );
        let title = switch_label
            .clone()
            .or_else(|| get_string(props, "aria-label"))
            .unwrap_or_default();
        let label = get_string_or(props, if is_dark { "lightLabel" } else { "darkLabel" }, "");
        let defaults = switch_label
            .map(|label| vec![("aria-label", Value::string(label))])
            .unwrap_or_default();
        let attrs = shared_root_attrs_with_defaults(props, &defaults);
        let toggle = view.toggle_theme;
        let disabled = toggle.is_none();
        let show_label = get_bool_or(props, "showLabel", false);
        rsx! {
            button {
                r#type: "button",
                class: "{class}",
                title: "{title}",
                disabled: disabled,
                onclick: move |_| {
                    if let Some(toggle) = toggle {
                        toggle.call(());
                    }
                },
                ..attrs,
                {registry.render_spec(&icon, None, view)}
                if show_label { span { class: "text-sm font-medium", "{label}" } }
            }
        }
    }
}
