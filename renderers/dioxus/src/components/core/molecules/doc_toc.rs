//! Native Dioxus renderer for the `doc-toc` pattern (web: DocTOC.tsx).

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, layout_box_spec, nav_landmark, pattern_spec, DioxusPatternRenderer,
    DioxusView, DOC_NAV_RULE_HOVER,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `doc-toc` pattern.
pub struct DocTOCRenderer;

impl DocTOCRenderer {
    const NAME: &str = "doc-toc";
}

impl DioxusPatternRenderer for DocTOCRenderer {
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
        let items = get_array(props, "items");
        let active_id = get_string(props, "activeId");
        let rows: Vec<Value> = items
            .iter()
            .map(|item| {
                let id = get_string_or(item, "id", "");
                let is_active = active_id.as_deref() == Some(id.as_str());
                let indent = if get_number(item, "level").is_some_and(|level| level >= 3.0) {
                    "pl-4"
                } else {
                    "pl-0"
                };
                let class = cn(&[
                    "h-auto w-full justify-start rounded-none py-1.5 pr-0 font-normal border-l-heavy",
                    "pl-3",
                    indent,
                    DOC_NAV_RULE_HOVER,
                    if is_active {
                        "border-l-accent text-foreground hover:text-foreground"
                    } else {
                        "border-l-transparent text-muted-foreground hover:border-l-border"
                    },
                ]);
                let label = pattern_spec(
                    "typography",
                    [
                        ("variant", Value::string("caption")),
                        ("color", Value::string("inherit")),
                        ("weight", Value::string(if is_active { "medium" } else { "normal" })),
                        ("className", Value::string("transition-colors")),
                        ("as", Value::string("span")),
                        ("content", Value::string(get_string_or(item, "label", ""))),
                    ],
                );
                let mut fields = vec![
                    ("variant", Value::string("ghost")),
                    ("href", Value::string(format!("#{id}"))),
                    ("className", Value::string(class)),
                    ("children", array_value(vec![label])),
                ];
                if is_active {
                    fields.push(("aria-current", Value::string("location")));
                }
                pattern_spec("button", fields)
            })
            .collect();
        let column = pattern_spec(
            "vstack",
            [
                ("gap", Value::string("none")),
                ("children", array_value(rows)),
            ],
        );
        let class = cn(&["w-full", &get_string_or(props, "className", "")]);
        let spec = layout_box_spec(
            &class,
            None,
            nav_landmark(props, &crate::i18n::t("aria.tableOfContents")),
            vec![column],
        );
        registry.render_spec(&spec, None, view)
    }
}
