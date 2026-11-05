//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::{BindingContextBuilder, Evaluator, SExpression, Value, ValueMap};

fn condition_holds(props: &Value) -> Result<bool, String> {
    let fields = props
        .as_object()
        .ok_or("conditional-wrapper props must be an object")?;
    let Some(condition) = fields.get("condition") else {
        return Ok(true);
    };
    if matches!(condition, Value::Null) {
        return Ok(true);
    }

    let context = fields
        .get("context")
        .and_then(Value::as_object)
        .ok_or("conditional-wrapper context must be an object")?;
    let mut entity = ValueMap::default();
    for key in ["formValues", "globalVariables"] {
        let value = context
            .get(key)
            .ok_or_else(|| format!("conditional-wrapper context.{key} is required"))?;
        entity.insert(key.into(), value.clone());
    }
    entity.insert(
        "localVariables".into(),
        context
            .get("localVariables")
            .cloned()
            .unwrap_or_else(|| Value::object(ValueMap::default())),
    );
    if let Some(extra) = context.get("entity") {
        let extra = extra
            .as_object()
            .ok_or("conditional-wrapper context.entity must be an object")?;
        for (key, value) in extra {
            entity.insert(key.clone(), value.clone());
        }
    }
    let guard: SExpression = serde_json::from_value(to_json(condition))
        .map_err(|error| format!("conditional-wrapper condition: {error}"))?;
    let context = BindingContextBuilder::new()
        .entity(Value::object(entity))
        .state(Value::string("active"))
        .build();
    Evaluator::new()
        .evaluate_guard(&guard, &context)
        .map_err(|error| format!("conditional-wrapper condition: {error}"))
}

/// Dioxus renderer for the `conditional-wrapper` pattern.
pub struct ConditionalWrapperRenderer;

impl ConditionalWrapperRenderer {
    const NAME: &str = "conditional-wrapper";
}

impl DioxusPatternRenderer for ConditionalWrapperRenderer {
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
        let show = condition_holds(props).unwrap_or_else(|error| panic!("{error}"));
        let fallback = props.as_object().and_then(|fields| fields.get("fallback"));
        let children = child_patterns(props, view.trait_content);
        if get_bool_or(props, "animate", false) {
            let state = if show {
                "opacity-100 max-h-[1000px]"
            } else {
                "opacity-0 max-h-0"
            };
            let class = merge_class_name(
                &format!("transition-all duration-fast overflow-hidden {state}"),
                props,
            );
            return rsx! { div { class: "{class}", ..shared_root_attrs(props),
                if show { for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} } }
                else if let Some(fallback) = fallback { {registry.render_spec(fallback, None, view)} }
            } };
        }
        rsx! {
            if show { for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} } }
            else if let Some(fallback) = fallback { {registry.render_spec(fallback, None, view)} }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::condition_holds;
    use orbital_core::client::helpers::from_json;

    #[test]
    fn evaluates_declared_condition_against_render_context() {
        let props = from_json(serde_json::json!({
            "condition": ["=", "@entity.formValues.vehicleType", "commercial"],
            "context": {
                "formValues": {"vehicleType": "commercial"},
                "globalVariables": {}
            }
        }));
        assert_eq!(condition_holds(&props), Ok(true));

        let props = from_json(serde_json::json!({
            "condition": ["=", "@entity.globalVariables.enabled", true],
            "context": {
                "formValues": {},
                "globalVariables": {"enabled": false}
            }
        }));
        assert_eq!(condition_holds(&props), Ok(false));
    }
}
