//! Dioxus renderer for the `violation-alert` pattern: the web's `ViolationAlert` composed over the `alert` pattern.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, dom_passthrough, object_value, pattern_spec, DioxusPatternRenderer, DioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `violation-alert` pattern.
pub struct ViolationAlertRenderer;

impl ViolationAlertRenderer {
    const NAME: &str = "violation-alert";
}

fn text(variant: &str, color: Option<&str>, weight: Option<&str>, content: &str) -> Value {
    let mut fields = vec![
        ("variant", Value::string(variant)),
        ("content", Value::string(content)),
    ];
    if let Some(color) = color {
        fields.push(("color", Value::string(color)));
    }
    if let Some(weight) = weight {
        fields.push(("weight", Value::string(weight)));
    }
    pattern_spec("typography", fields)
}

fn labelled(label: &str, value: &str) -> Value {
    pattern_spec(
        "hstack",
        [
            ("gap", Value::string("xs")),
            ("align", Value::string("center")),
            ("wrap", Value::Bool(true)),
            (
                "children",
                array_value(vec![
                    text("caption", Some("muted"), None, label),
                    text("caption", None, Some("semibold"), value),
                ]),
            ),
        ],
    )
}

fn callback(props: &Value, key: &str) -> Option<Value> {
    props
        .get(key)
        .filter(|value| !value.is_null() && value.as_str() != Some(""))
        .cloned()
}

impl DioxusPatternRenderer for ViolationAlertRenderer {
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
        let violation = props
            .get("violation")
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or_else(|| {
                object_value([
                    ("id", Value::string("unknown")),
                    ("law", Value::string("")),
                    ("article", Value::string("")),
                    (
                        "message",
                        Value::string(
                            get_string(props, "message").unwrap_or_else(|| "Violation".to_string()),
                        ),
                    ),
                    ("actionType", Value::string("measure")),
                ])
            });
        let law = get_string_or(&violation, "law", "");
        let article = get_string_or(&violation, "article", "");
        let message = get_string_or(&violation, "message", "");
        let action_type = get_string_or(&violation, "actionType", "");
        let severity = get_string(props, "severity").unwrap_or_else(|| {
            if action_type == "measure" {
                "warning"
            } else {
                "error"
            }
            .to_string()
        });
        let citation = format!("{law} Art. {article}");
        let icon = match action_type.as_str() {
            "measure" => Some("alert-triangle"),
            "admin" => Some("alert-circle"),
            "penalty" => Some("shield-alert"),
            _ => None,
        };
        let class_name = get_string_or(props, "className", "");
        let on_dismiss = callback(props, "onDismiss");

        let mut fields: Vec<(String, Value)> = dom_passthrough(props);
        fields.push(("variant".to_string(), Value::string(severity)));
        if let Some(icon) = icon {
            fields.push(("icon".to_string(), Value::string(icon)));
        }
        fields.push(("title".to_string(), Value::string(citation)));
        if let (true, Some(on_dismiss)) = (get_bool_or(props, "dismissible", false), on_dismiss) {
            fields.push(("dismissible".to_string(), Value::Bool(true)));
            fields.push(("onDismiss".to_string(), on_dismiss));
        }

        if get_bool_or(props, "compact", false) {
            fields.push((
                "className".to_string(),
                Value::string(cn(&["px-3 py-2", class_name.as_str()])),
            ));
            fields.push(("message".to_string(), Value::string(message)));
            return registry.render_spec(&pattern_spec("alert", fields), None, view);
        }

        if !class_name.is_empty() {
            fields.push(("className".to_string(), Value::string(class_name)));
        }
        let field_id = get_string_or(&violation, "fieldId", "");
        if let (false, Some(navigate)) = (field_id.is_empty(), callback(props, "onNavigateToField"))
        {
            fields.push((
                "actions".to_string(),
                array_value(vec![pattern_spec(
                    "button",
                    [
                        ("variant", Value::string("ghost")),
                        ("size", Value::string("sm")),
                        ("icon", Value::string("arrow-right")),
                        ("label", Value::string("Go to field")),
                        ("onClick", navigate),
                        (
                            "actionPayload",
                            object_value([("fieldId", Value::string(field_id))]),
                        ),
                    ],
                )]),
            ));
        }

        let type_label = match action_type.as_str() {
            "measure" => "Corrective Measure",
            "admin" => "Administrative Action",
            "penalty" => "Penalty Proceedings",
            _ => "",
        };
        let mut body = vec![
            text("caption", Some("muted"), None, type_label),
            text("body2", None, None, &message),
        ];
        let admin_action = get_string_or(&violation, "adminAction", "");
        if !admin_action.is_empty() {
            body.push(labelled("Admin:", &admin_action));
        }
        let penalty_action = get_string_or(&violation, "penaltyAction", "");
        if !penalty_action.is_empty() {
            body.push(labelled("Penalty:", &penalty_action));
        }
        fields.push((
            "children".to_string(),
            array_value(vec![pattern_spec(
                "vstack",
                [
                    ("gap", Value::string("xs")),
                    ("children", array_value(body)),
                ],
            )]),
        ));
        registry.render_spec(&pattern_spec("alert", fields), None, view)
    }
}
