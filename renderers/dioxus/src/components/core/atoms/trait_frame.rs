//! Native Dioxus renderer for the shared `trait-frame` pattern.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::Value;

pub struct TraitFrameRenderer;

impl DioxusPatternRenderer for TraitFrameRenderer {
    fn pattern_name(&self) -> &str {
        "trait-frame"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let Some(trait_name) = props
            .as_object()
            .and_then(|fields| fields.get("traitName"))
            .and_then(Value::as_str)
        else {
            return rsx! {};
        };
        if let Some(content) = view.trait_content.get(trait_name) {
            return registry.render_spec(
                content,
                None,
                &view.for_trait_source(trait_name.to_string()),
            );
        }
        match props.as_object().and_then(|fields| fields.get("fallback")) {
            Some(Value::String(text)) => rsx! { "{text}" },
            Some(fallback @ (Value::Object(_) | Value::Array(_))) => {
                registry.render_spec(fallback, None, view)
            }
            _ => rsx! {},
        }
    }
}
