use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `container` pattern.
pub struct ContainerRenderer;

impl ContainerRenderer {
    const NAME: &str = "container";
}

impl DioxusPatternRenderer for ContainerRenderer {
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
        let size = get_string(props, "maxWidth")
            .or_else(|| get_string(props, "size"))
            .unwrap_or_else(|| "lg".to_string());
        let size_class = match size.as_str() {
            "xs" => "max-w-xs",
            "sm" => "max-w-screen-sm",
            "md" => "max-w-screen-md",
            "xl" => "max-w-screen-xl",
            "2xl" => "max-w-screen-2xl",
            "full" => "max-w-full",
            _ => "max-w-screen-lg",
        };
        let padding_class = match get_string_or(props, "padding", "md").as_str() {
            "none" => "px-0",
            "sm" => "px-3 sm:px-4",
            "lg" => "px-4 sm:px-6 md:px-8",
            "xl" => "px-4 sm:px-8 md:px-12",
            _ => "px-4 sm:px-6",
        };
        let center = if get_bool_or(props, "center", true) {
            "mx-auto"
        } else {
            ""
        };
        let class = cn(&[
            "w-full",
            size_class,
            padding_class,
            center,
            &get_string_or(props, "className", ""),
        ]);
        let children = child_patterns(props, view.trait_content);
        rsx! {
            div { class: "{class}", ..shared_root_attrs(props),
                for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
            }
        }
    }
}
