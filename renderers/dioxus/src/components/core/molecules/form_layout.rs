use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `form-layout` pattern.
pub struct FormLayoutRenderer;

impl FormLayoutRenderer {
    const NAME: &str = "form-layout";
}

impl DioxusPatternRenderer for FormLayoutRenderer {
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
        let dividers = get_bool_or(props, "dividers", true);
        let divider_class = if dividers {
            " [&>*+*]:pt-8 [&>*+*]:border-t [&>*+*]:border-border"
        } else {
            ""
        };
        let class = merge_class_name(
            &format!("flex flex-col gap-6 items-stretch justify-start{divider_class}"),
            props,
        );
        let children = child_patterns(props, view.trait_content);
        rsx! { div { class: "{class}", style: "height:auto",
            ..shared_root_attrs(props),
            for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
        } }
    }
}
