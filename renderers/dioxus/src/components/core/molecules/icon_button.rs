//! Native Dioxus renderer for the `icon-button` pattern: the web `IconButton`, a ghost `button` around an `icon`, wrapped in a `tooltip`.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{array_value, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `icon-button` pattern.
pub struct IconButtonRenderer;

impl IconButtonRenderer {
    const NAME: &str = "icon-button";
}

impl DioxusPatternRenderer for IconButtonRenderer {
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
        let label = get_string_or(props, "label", "");
        let icon = get_string_or(props, "icon", "");
        let position = get_string_or(props, "tooltipPosition", "right");
        let class_name = get_string_or(props, "className", "");
        let mut fields: Vec<(String, Value)> = Vec::new();
        if let Some(object) = props.as_object() {
            for (key, value) in object.iter() {
                let key: &str = key.as_ref();
                if matches!(
                    key,
                    "type"
                        | "icon"
                        | "label"
                        | "tooltipPosition"
                        | "className"
                        | "variant"
                        | "size"
                        | "children"
                        | "enter"
                        | "enterDelay"
                        | "skeleton"
                        | "leftIcon"
                        | "rightIcon"
                        | "iconRight"
                ) {
                    continue;
                }
                fields.push((key.to_string(), value.clone()));
            }
        }
        fields.push((
            "variant".to_string(),
            Value::string(get_string_or(props, "variant", "ghost")),
        ));
        fields.push((
            "size".to_string(),
            Value::string(get_string_or(props, "size", "sm")),
        ));
        fields.push(("aria-label".to_string(), Value::string(label.clone())));
        fields.push((
            "className".to_string(),
            Value::string(
                format!("justify-center px-0 aspect-square {class_name}")
                    .trim()
                    .to_string(),
            ),
        ));
        let glyph = pattern_spec(
            "icon",
            [("name", Value::string(icon)), ("size", Value::string("sm"))],
        );
        fields.push(("children".to_string(), array_value(vec![glyph])));
        let button = pattern_spec("button", fields);
        let tooltip = pattern_spec(
            "tooltip",
            [
                ("content", Value::string(label)),
                ("position", Value::string(position)),
                ("children", array_value(vec![button])),
            ],
        );
        registry.render_spec(&tooltip, None, view)
    }
}
