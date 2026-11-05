use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `form-section-header` pattern.
pub struct FormSectionHeaderRenderer;

impl FormSectionHeaderRenderer {
    const NAME: &str = "form-section-header";
}

fn text(value: impl AsRef<str>) -> Value {
    Value::string(value.as_ref().to_string())
}

fn list(items: Vec<Value>) -> Value {
    Value::Array(items.into())
}

impl DioxusPatternRenderer for FormSectionHeaderRenderer {
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
        let title = get_string_or(props, "title", "");
        let subtitle = get_string(props, "subtitle").filter(|subtitle| !subtitle.is_empty());
        let is_collapsed = get_bool_or(props, "isCollapsed", false);
        let on_toggle = get_string(props, "onToggle").filter(|event| !event.is_empty());
        let badge = get_string(props, "badge").filter(|badge| !badge.is_empty());
        let badge_variant = get_string_or(props, "badgeVariant", "default");
        let icon = get_string(props, "icon").filter(|icon| !icon.is_empty());
        let has_errors = get_bool_or(props, "hasErrors", false);
        let is_complete = get_bool_or(props, "isComplete", false);
        let clickable = on_toggle.is_some();

        let effective_variant = if has_errors {
            "danger"
        } else if is_complete {
            "success"
        } else {
            badge_variant.as_str()
        };
        let status_icon = if has_errors {
            Some("alert-circle")
        } else if is_complete {
            Some("check-circle")
        } else {
            None
        };

        let mut leading = Vec::new();
        if let Some(icon) = &icon {
            leading.push(pattern_spec(
                "icon",
                vec![
                    ("name", text(icon)),
                    ("size", text("md")),
                    ("className", text("text-primary shrink-0")),
                ],
            ));
        }
        if let Some(status) = status_icon {
            leading.push(pattern_spec(
                "icon",
                vec![
                    ("name", text(status)),
                    ("size", text("md")),
                    (
                        "className",
                        text(if has_errors {
                            "shrink-0 text-error"
                        } else {
                            "shrink-0 text-success"
                        }),
                    ),
                ],
            ));
        }
        let mut titles = vec![pattern_spec(
            "typography",
            vec![
                ("variant", text("subheading")),
                ("weight", text("semibold")),
                ("className", text("text-foreground")),
                ("content", text(&title)),
            ],
        )];
        if let Some(subtitle) = &subtitle {
            titles.push(pattern_spec(
                "typography",
                vec![
                    ("variant", text("caption")),
                    ("color", text("muted")),
                    ("className", text("leading-snug")),
                    ("content", text(subtitle)),
                ],
            ));
        }
        leading.push(pattern_spec(
            "box",
            vec![
                ("className", text("space-y-0.5")),
                ("children", list(titles)),
            ],
        ));

        let mut trailing = Vec::new();
        if let Some(badge) = &badge {
            trailing.push(pattern_spec(
                "badge",
                vec![
                    ("variant", text(effective_variant)),
                    ("size", text("sm")),
                    ("label", text(badge)),
                ],
            ));
        }
        if clickable {
            trailing.push(pattern_spec(
                "icon",
                vec![
                    ("name", text("chevron-down")),
                    ("size", text("sm")),
                    (
                        "className",
                        text(if is_collapsed {
                            "text-muted-foreground transition-transform duration-fast shrink-0 -rotate-90"
                        } else {
                            "text-muted-foreground transition-transform duration-fast shrink-0"
                        }),
                    ),
                ],
            ));
        }

        let row = pattern_spec(
            "hstack",
            vec![
                ("justify", text("between")),
                ("align", text("center")),
                (
                    "children",
                    list(vec![
                        pattern_spec(
                            "hstack",
                            vec![
                                ("gap", text("sm")),
                                ("align", text("center")),
                                ("children", list(leading)),
                            ],
                        ),
                        pattern_spec(
                            "hstack",
                            vec![
                                ("gap", text("sm")),
                                ("align", text("center")),
                                ("children", list(trailing)),
                            ],
                        ),
                    ]),
                ),
            ],
        );

        let base = "px-5 py-4 bg-muted/60 rounded-container border border-border border-s-heavy border-s-primary";
        let class = if clickable {
            format!("{base} cursor-pointer hover:bg-muted transition-colors")
        } else {
            base.to_string()
        };
        let mut fields = vec![
            ("className", text(merge_class_name(&class, props))),
            ("children", list(vec![row])),
        ];
        for key in [
            "id",
            "aria-label",
            "aria-labelledby",
            "aria-describedby",
            "aria-live",
            "aria-current",
            "lang",
            "dir",
        ] {
            if let Some(value) = props.get(key) {
                fields.push((key, value.clone()));
            }
        }
        if let Some(event) = on_toggle {
            fields.push(("role", text("button")));
            fields.push(("tabIndex", Value::Int(0)));
            fields.push(("aria-expanded", Value::Bool(!is_collapsed)));
            fields.push(("action", text(event)));
        } else {
            if let Some(role) = props.get("role") {
                fields.push(("role", role.clone()));
            }
            if let Some(expanded) = props.get("aria-expanded") {
                fields.push(("aria-expanded", expanded.clone()));
            }
        }
        registry.render_spec(&pattern_spec("box", fields), None, view)
    }
}
