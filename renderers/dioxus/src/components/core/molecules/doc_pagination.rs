//! Native Dioxus renderer for the `doc-pagination` pattern (web: DocPagination.tsx).

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, layout_box_spec, nav_landmark, pattern_spec, typography_spec,
    DioxusPatternRenderer, DioxusView, DOC_NAV_CARD_HOVER,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `doc-pagination` pattern.
pub struct DocPaginationRenderer;

impl DocPaginationRenderer {
    const NAME: &str = "doc-pagination";
}

const LINK_CARD: &str = "border border-border rounded-container p-4 transition-all hover:shadow-elevation-dialog no-underline flex-1 min-w-0 cursor-pointer";
const ARROW_CLASS: &str =
    "text-muted-foreground group-hover:text-accent transition-colors flex-shrink-0";

fn link_of(props: &Value, key: &str) -> Option<Value> {
    props
        .get(key)
        .filter(|link| link.as_object().is_some())
        .cloned()
}

fn stack(name: &str, fields: Vec<(&str, Value)>, children: Vec<Value>) -> Value {
    let mut fields = fields;
    fields.push(("children", array_value(children)));
    pattern_spec(name, fields)
}

fn icon(name: &str) -> Value {
    pattern_spec(
        "icon",
        [
            ("name", Value::string(name)),
            ("size", Value::string("md")),
            ("className", Value::string(ARROW_CLASS)),
        ],
    )
}

fn label_stack(link: &Value, align: &str) -> Value {
    let mut texts = Vec::new();
    if let Some(category) = get_string(link, "category").filter(|text| !text.is_empty()) {
        texts.push(typography_spec("caption", Some("muted"), "", &category));
    }
    texts.push(typography_spec(
        "body",
        None,
        "group-hover:text-accent transition-colors",
        &get_string_or(link, "label", ""),
    ));
    stack(
        "vstack",
        vec![
            ("gap", Value::string("none")),
            ("align", Value::string(align)),
        ],
        texts,
    )
}

fn card(link: &Value, class: &str, content: Value) -> Value {
    let mut fields = vec![
        ("variant", Value::string("ghost")),
        ("className", Value::string(class)),
        ("children", array_value(vec![content])),
    ];
    if let Some(href) = get_string(link, "href") {
        fields.push(("href", Value::string(href)));
    }
    pattern_spec("button", fields)
}

fn spacer() -> Value {
    layout_box_spec("flex-1", None, Vec::new(), Vec::new())
}

impl DioxusPatternRenderer for DocPaginationRenderer {
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
        let prev = link_of(props, "prev");
        let next = link_of(props, "next");
        if prev.is_none() && next.is_none() {
            return rsx! {};
        }
        let base = cn(&[LINK_CARD, DOC_NAV_CARD_HOVER]);
        let prev_node = match prev.as_ref() {
            Some(link) => {
                let class = cn(&[
                    &base,
                    "group",
                    "h-auto justify-start whitespace-normal font-normal",
                ]);
                let row = stack(
                    "hstack",
                    vec![
                        ("align", Value::string("center")),
                        ("gap", Value::string("sm")),
                    ],
                    vec![icon("arrow-left"), label_stack(link, "start")],
                );
                card(link, &class, row)
            }
            None => spacer(),
        };
        let next_node = match next.as_ref() {
            Some(link) => {
                let class = cn(&[
                    &base,
                    "group text-right",
                    "h-auto justify-end whitespace-normal font-normal",
                ]);
                let row = stack(
                    "hstack",
                    vec![
                        ("align", Value::string("center")),
                        ("justify", Value::string("end")),
                        ("gap", Value::string("sm")),
                    ],
                    vec![label_stack(link, "end"), icon("arrow-right")],
                );
                card(link, &class, row)
            }
            None => spacer(),
        };
        let mut fields: Vec<(String, Value)> =
            nav_landmark(props, &crate::i18n::t("aria.pageNavigation"));
        fields.push(("justify".to_string(), Value::string("between")));
        fields.push(("align".to_string(), Value::string("stretch")));
        fields.push(("gap".to_string(), Value::string("md")));
        fields.push((
            "className".to_string(),
            Value::string(cn(&["w-full", &get_string_or(props, "className", "")])),
        ));
        fields.push((
            "children".to_string(),
            array_value(vec![prev_node, next_node]),
        ));
        registry.render_spec(&pattern_spec("hstack", fields), None, view)
    }
}
