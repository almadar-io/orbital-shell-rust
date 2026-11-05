use crate::registry::DioxusPatternRegistry;
use crate::renderer::{pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_bool_or, get_string, get_string_or};
use orbital_core::Value;

/// Dioxus renderer for the `confirm-dialog` pattern.
pub struct ConfirmDialogRenderer;

impl ConfirmDialogRenderer {
    const NAME: &str = "confirm-dialog";
}

fn first_string(props: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| get_string(props, key))
}

impl DioxusPatternRenderer for ConfirmDialogRenderer {
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
        let variant = get_string_or(props, "variant", "danger");
        let (icon, icon_bg, icon_color, confirm_variant) = match variant.as_str() {
            "warning" => (
                "alert-triangle",
                "bg-warning",
                "text-warning-foreground",
                "warning",
            ),
            "info" => ("check", "bg-info", "text-info-foreground", "primary"),
            "default" => ("check", "bg-primary", "text-primary-foreground", "primary"),
            _ => ("trash-2", "bg-error", "text-error-foreground", "danger"),
        };
        let is_loading = get_bool_or(props, "isLoading", false);
        let title = get_string_or(props, "title", "");
        let confirm_text = first_string(props, &["confirmText", "confirmLabel"])
            .unwrap_or_else(|| "Confirm".to_string());
        let cancel_text = first_string(props, &["cancelText", "cancelLabel"])
            .unwrap_or_else(|| "Cancel".to_string());
        let confirm_event = get_string_or(props, "confirmEvent", "CONFIRM");
        let cancel_event = get_string_or(props, "cancelEvent", "CANCEL");
        let fields = props.as_object();
        let message = fields
            .and_then(|fields| fields.get("message").or_else(|| fields.get("description")))
            .cloned();

        let cancel = pattern_spec(
            "button",
            [
                ("variant", Value::string("secondary")),
                ("label", Value::String(cancel_text.into())),
                ("disabled", Value::Bool(is_loading)),
                ("action", Value::String(cancel_event.clone().into())),
            ],
        );
        let confirm = pattern_spec(
            "button",
            [
                ("variant", Value::string(confirm_variant)),
                (
                    "label",
                    Value::String(
                        if is_loading {
                            "Loading...".to_string()
                        } else {
                            confirm_text
                        }
                        .into(),
                    ),
                ),
                ("isLoading", Value::Bool(is_loading)),
                ("action", Value::String(confirm_event.into())),
            ],
        );
        let footer = pattern_spec(
            "hstack",
            [
                ("className", Value::string("justify-end gap-3")),
                ("gap", Value::string("none")),
                ("children", Value::Array(vec![cancel, confirm].into())),
            ],
        );

        let icon_box = pattern_spec(
            "box",
            [
                (
                    "className",
                    Value::String(
                        format!(
                            "flex-shrink-0 w-12 h-12 flex items-center justify-center {icon_bg}"
                        )
                        .into(),
                    ),
                ),
                (
                    "children",
                    Value::Array(
                        vec![pattern_spec(
                            "icon",
                            [
                                ("name", Value::string(icon)),
                                ("size", Value::string("lg")),
                                ("className", Value::string(icon_color)),
                            ],
                        )]
                        .into(),
                    ),
                ),
            ],
        );
        let mut content = vec![pattern_spec(
            "typography",
            [
                ("variant", Value::string("h5")),
                ("className", Value::string("mb-2")),
                ("content", Value::String(title.into())),
            ],
        )];
        match message {
            Some(Value::String(text)) => content.push(pattern_spec(
                "typography",
                [
                    ("variant", Value::string("body2")),
                    ("className", Value::string("text-muted-foreground")),
                    ("content", Value::String(text)),
                ],
            )),
            Some(node) if !node.is_null() => content.push(node),
            _ => content.push(pattern_spec(
                "typography",
                [
                    ("variant", Value::string("body2")),
                    ("className", Value::string("text-muted-foreground")),
                    ("content", Value::string("")),
                ],
            )),
        }
        if let Some(error) = fields
            .and_then(|fields| fields.get("error"))
            .filter(|error| !error.is_null())
        {
            content.push(pattern_spec(
                "typography",
                [
                    ("variant", Value::string("body2")),
                    ("color", Value::string("error")),
                    ("role", Value::string("alert")),
                    ("className", Value::string("mt-2")),
                    (
                        "content",
                        Value::String(get_string_or(error, "message", "").into()),
                    ),
                ],
            ));
        }
        let body = pattern_spec(
            "hstack",
            [
                ("className", Value::string("gap-4")),
                (
                    "children",
                    Value::Array(
                        vec![
                            icon_box,
                            pattern_spec(
                                "box",
                                [
                                    ("className", Value::string("flex-1")),
                                    ("children", Value::Array(content.into())),
                                ],
                            ),
                        ]
                        .into(),
                    ),
                ),
            ],
        );

        let mut modal = vec![
            ("isOpen", Value::Bool(get_bool_or(props, "isOpen", true))),
            (
                "size",
                Value::String(get_string_or(props, "size", "sm").into()),
            ),
            ("showCloseButton", Value::Bool(false)),
            ("closeOnOverlayClick", Value::Bool(!is_loading)),
            ("closeOnEscape", Value::Bool(!is_loading)),
            ("closeEvent", Value::String(cancel_event.into())),
            ("footer", footer),
            ("children", Value::Array(vec![body].into())),
        ];
        if let Some(class) = get_string(props, "className") {
            modal.push(("className", Value::String(class.into())));
        }
        for key in [
            "id",
            "aria-label",
            "aria-labelledby",
            "aria-describedby",
            "role",
        ] {
            if let Some(value) = fields.and_then(|fields| fields.get(key)) {
                modal.push((key, value.clone()));
            }
        }
        registry.render_spec(&pattern_spec("modal", modal), None, view)
    }
}
