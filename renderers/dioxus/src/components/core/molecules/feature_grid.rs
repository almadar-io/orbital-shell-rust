use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, dom_passthrough, pattern_spec, DioxusPatternRenderer, DioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `feature-grid` pattern.
pub struct FeatureGridRenderer;

impl FeatureGridRenderer {
    const NAME: &str = "feature-grid";
}

impl DioxusPatternRenderer for FeatureGridRenderer {
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
        let cards = get_array(props, "items")
            .iter()
            .map(|item| {
                let mut fields: Vec<(String, Value)> = item
                    .as_object()
                    .map(|object| {
                        object
                            .iter()
                            .map(|(key, value)| (key.to_string(), value.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                fields.retain(|(key, _)| key != "type");
                pattern_spec("feature-card", fields)
            })
            .collect::<Vec<_>>();
        let mut fields = dom_passthrough(props);
        fields.push((
            "cols".to_string(),
            Value::Int(
                get_number(props, "columns")
                    .map(|cols| cols as i64)
                    .unwrap_or(3),
            ),
        ));
        fields.push((
            "gap".to_string(),
            Value::string(get_string_or(props, "gap", "md")),
        ));
        fields.push((
            "className".to_string(),
            Value::string(get_string_or(props, "className", "")),
        ));
        fields.push(("children".to_string(), array_value(cards)));
        registry.render_spec(&pattern_spec("simple-grid", fields), None, view)
    }
}
