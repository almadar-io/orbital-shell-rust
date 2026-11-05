use crate::registry::DioxusPatternRegistry;
use crate::renderer::{cn, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `date-range-selector` pattern.
pub struct DateRangeSelectorRenderer;

impl DateRangeSelectorRenderer {
    const NAME: &str = "date-range-selector";
}

impl DioxusPatternRenderer for DateRangeSelectorRenderer {
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
        let options: Vec<(String, String)> = match props.get("options").and_then(Value::as_array) {
            Some(items) => items
                .iter()
                .filter_map(|item| Some((get_string(item, "label")?, get_string(item, "value")?)))
                .collect(),
            None => [
                ("1W", "week"),
                ("1M", "month"),
                ("3M", "3months"),
                ("1Y", "year"),
            ]
            .iter()
            .map(|(label, value)| (label.to_string(), value.to_string()))
            .collect(),
        };
        let selected = get_string_or(props, "selected", "month");
        let action = get_string(props, "onSelect");
        let class = cn(&[
            "flex flex-row gap-1 items-center justify-start",
            &get_string_or(props, "className", ""),
        ]);
        let buttons: Vec<Value> = options
            .iter()
            .map(|(label, value)| {
                let mut fields = vec![
                    (
                        "variant",
                        Value::string(if *value == selected {
                            "primary"
                        } else {
                            "ghost"
                        }),
                    ),
                    ("size", Value::string("sm")),
                    ("label", Value::string(label.to_string())),
                ];
                if let Some(action) = &action {
                    fields.push(("action", Value::string(action.to_string())));
                    fields.push(("actionPayload", Value::string(value.to_string())));
                }
                pattern_spec("button", fields)
            })
            .collect();
        rsx! {
            div { class: "{class}", style: "height:auto",
                for spec in buttons.iter() { {registry.render_spec(spec, None, view)} }
            }
        }
    }
}
