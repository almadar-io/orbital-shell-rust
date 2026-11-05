use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::Value;

/// Dioxus renderer for the `form-section` pattern, which maps to the same `Form` component as `form`.
pub struct FormRenderer;

impl FormRenderer {
    const NAME: &str = "form-section";
}

impl DioxusPatternRenderer for FormRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        super::form::FormRenderer.render(props, entity, view, registry)
    }
}
