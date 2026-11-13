use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, dom_passthrough, layout_box_spec, object_value, pattern_spec,
    DioxusPatternRenderer, DioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `change-list` pattern.
pub struct ChangeListRenderer;

impl ChangeListRenderer {
    const NAME: &str = "change-list";
}

struct Change {
    kind: &'static str,
    title: String,
    description: String,
    before: String,
    after: String,
}

fn kind_of(value: &str) -> Option<&'static str> {
    match value {
        "added" => Some("added"),
        "modified" => Some("modified"),
        "removed" => Some("removed"),
        _ => None,
    }
}

fn change_of(item: &Value) -> Option<Change> {
    let kind = kind_of(&get_string(item, "type")?)?;
    Some(Change {
        kind,
        title: get_string(item, "title")?,
        description: get_string_or(item, "description", ""),
        before: get_string_or(item, "before", ""),
        after: get_string_or(item, "after", ""),
    })
}

fn text(
    variant: &str,
    color: Option<&str>,
    weight: Option<&str>,
    class: &str,
    content: &str,
) -> Value {
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
    if !class.is_empty() {
        fields.push(("className", Value::string(class)));
    }
    pattern_spec("typography", fields)
}

fn diff_box(class: &str, label: &str, body: &str) -> Value {
    layout_box_spec(
        class,
        None,
        Vec::new(),
        vec![
            text("caption", Some("muted"), Some("semibold"), "", label),
            text(
                "caption",
                None,
                None,
                "block font-mono whitespace-pre-wrap",
                body,
            ),
        ],
    )
}

fn card(change: &Change, inspect: Option<&str>) -> Value {
    let (icon, badge, text_class, label) = match change.kind {
        "added" => ("plus-circle", "success", "text-success", "Added"),
        "removed" => ("minus-circle", "error", "text-error", "Removed"),
        _ => ("edit-3", "warning", "text-warning", "Modified"),
    };
    let mut body = vec![pattern_spec(
        "hstack",
        [
            ("gap", Value::string("sm")),
            ("align", Value::string("center")),
            ("className", Value::string("min-w-0")),
            (
                "children",
                array_value(vec![
                    pattern_spec(
                        "icon",
                        [
                            ("name", Value::string(icon)),
                            ("size", Value::string("sm")),
                            ("className", Value::string(text_class)),
                        ],
                    ),
                    text("body2", None, Some("semibold"), "truncate", &change.title),
                    pattern_spec(
                        "badge",
                        [
                            ("variant", Value::string(badge)),
                            ("size", Value::string("sm")),
                            ("label", Value::string(label)),
                        ],
                    ),
                ]),
            ),
        ],
    )];
    if !change.description.is_empty() {
        body.push(text(
            "caption",
            Some("muted"),
            None,
            "",
            &change.description,
        ));
    }
    if change.kind == "modified" && (!change.before.is_empty() || !change.after.is_empty()) {
        let mut sides = Vec::new();
        if !change.before.is_empty() {
            sides.push(diff_box(
                "flex-1 rounded-container border border-error/20 bg-error/5 p-2",
                &crate::i18n::t("changeList.before"),
                &change.before,
            ));
        }
        if !change.after.is_empty() {
            sides.push(diff_box(
                "flex-1 rounded-container border border-success/20 bg-success/5 p-2",
                &crate::i18n::t("changeList.after"),
                &change.after,
            ));
        }
        body.push(pattern_spec(
            "hstack",
            [
                ("gap", Value::string("sm")),
                ("align", Value::string("stretch")),
                ("children", array_value(sides)),
            ],
        ));
    }
    if let Some(event) = inspect {
        let button = pattern_spec(
            "button",
            [
                ("variant", Value::string("ghost")),
                ("size", Value::string("sm")),
                ("icon", Value::string("external-link")),
                ("label", Value::string(crate::i18n::t("changeList.inspect"))),
                ("action", Value::string(event)),
                (
                    "actionPayload",
                    object_value([
                        ("title", Value::string(change.title.to_string())),
                        ("type", Value::string(change.kind)),
                    ]),
                ),
            ],
        );
        body.push(layout_box_spec("", None, Vec::new(), vec![button]));
    }
    pattern_spec(
        "card",
        [
            ("data-testid", Value::string("change-list-item")),
            ("data-change-type", Value::string(change.kind)),
            (
                "children",
                array_value(vec![pattern_spec(
                    "vstack",
                    [
                        ("gap", Value::string("xs")),
                        ("className", Value::string("p-3")),
                        ("children", array_value(body)),
                    ],
                )]),
            ),
        ],
    )
}

impl DioxusPatternRenderer for ChangeListRenderer {
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
        let class_name = get_string_or(props, "className", "");
        let changes = get_array(props, "changes")
            .iter()
            .filter_map(change_of)
            .collect::<Vec<_>>();
        let mut extra = dom_passthrough(props);
        if changes.is_empty() {
            extra.push((
                "data-testid".to_string(),
                Value::string("change-list-empty"),
            ));
            let empty = layout_box_spec(
                &cn(&["py-6", class_name.trim()]),
                None,
                extra,
                vec![pattern_spec(
                    "typography",
                    [
                        ("variant", Value::string("body2")),
                        ("color", Value::string("muted")),
                        ("align", Value::string("center")),
                        ("content", Value::string(crate::i18n::t("changeList.empty"))),
                    ],
                )],
            );
            return registry.render_spec(&empty, None, view);
        }
        let inspect = get_string(props, "inspectEvent").filter(|event| !event.is_empty());
        let cards = changes
            .iter()
            .map(|change| card(change, inspect.as_deref()))
            .collect::<Vec<_>>();
        let mut fields = extra;
        fields.push(("gap".to_string(), Value::string("sm")));
        fields.push(("className".to_string(), Value::string(class_name)));
        fields.push(("children".to_string(), array_value(cards)));
        registry.render_spec(&pattern_spec("vstack", fields), None, view)
    }
}
