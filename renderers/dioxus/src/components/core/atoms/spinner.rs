//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `spinner` pattern.
pub struct SpinnerRenderer;

impl SpinnerRenderer {
    const NAME: &str = "spinner";
}

impl DioxusPatternRenderer for SpinnerRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        _view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let size = get_string_or(props, "size", "md");
        let size_class = match size.as_str() {
            "xs" => "h-3 w-3",
            "sm" => "h-icon-default w-icon-default",
            "lg" => "h-8 w-8",
            _ => "h-6 w-6",
        };
        let overlay = get_bool_or(props, "overlay", false);
        let root_class = if overlay {
            "absolute inset-0 z-10 flex items-center justify-center bg-background/60 backdrop-blur-sm"
        } else {
            "text-foreground"
        };
        let class = merge_class_name(root_class, props);
        let mut root_attrs = crate::registry::shared_root_attrs(props);
        root_attrs.push(Attribute::new("data-loading-state", "", None, false));
        rsx! { div { class: "{class}", role: "status", aria_label: "Loading", ..root_attrs,
            span {
                "data-loader-mark": "",
                aria_hidden: "true",
                class: "bg-center bg-no-repeat bg-contain {size_class}",
                style: "display:var(--loader-mark-display, none);background-image:var(--illustration-loading)"
            }
            span {
                "data-loader-icon": "",
                style: "display:var(--loader-icon-display, inline-flex)",
                span { class: "almadar-spinner {size_class}" }
            }
        } }
    }
}
