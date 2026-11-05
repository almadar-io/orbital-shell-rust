use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `label` pattern.
pub struct LabelRenderer;

impl LabelRenderer {
    const NAME: &str = "label";
}

impl DioxusPatternRenderer for LabelRenderer {
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
        let class = merge_class_name("block text-sm font-bold text-foreground", props);
        let text = get_string_or(props, "text", "");
        let html_for = get_string_or(props, "htmlFor", "");
        let required = get_bool_or(props, "required", false);
        rsx! {
            label { class: "{class}", for: "{html_for}", ..shared_root_attrs(props),
                "{text}"
                if required { span { class: "text-error ml-1", aria_hidden: "true", "*" } }
            }
        }
    }
}
