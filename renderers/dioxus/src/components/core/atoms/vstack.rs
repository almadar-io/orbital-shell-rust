//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `vstack` pattern.
pub struct VStackRenderer;

impl VStackRenderer {
    const NAME: &str = "vstack";
}

impl DioxusPatternRenderer for VStackRenderer {
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
        let Some(fields) = props.as_object() else {
            return rsx! {};
        };
        let mut fields = fields.clone();
        fields.insert(
            "direction".into(),
            Value::String("vertical".to_string().into()),
        );
        super::stack::StackRenderer.render(&Value::Object(fields.into()), None, view, registry)
    }
}
