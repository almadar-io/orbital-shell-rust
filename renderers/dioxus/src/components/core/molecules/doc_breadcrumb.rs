//! Native Dioxus renderer for the `doc-breadcrumb` pattern (web: DocBreadcrumb.tsx).

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, layout_box_spec, nav_landmark, pattern_spec, DioxusPatternRenderer, DioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `doc-breadcrumb` pattern.
pub struct DocBreadcrumbRenderer;

impl DocBreadcrumbRenderer {
    const NAME: &str = "doc-breadcrumb";
}

impl DioxusPatternRenderer for DocBreadcrumbRenderer {
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
        let mut nodes: Vec<Value> = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let is_last = index + 1 == items.len();
            let label = get_string_or(item, "label", "");
            let href = get_string(item, "href").filter(|href| !href.is_empty());
            if index > 0 {
                nodes.push(pattern_spec(
                    "icon",
                    [
                        ("name", Value::string("chevron-right")),
                        ("size", Value::string("xs")),
                        ("className", Value::string("text-muted-foreground shrink-0")),
                    ],
                ));
            }
            match (is_last, href) {
                (false, Some(href)) => nodes.push(pattern_spec(
                    "button",
                    [
                        ("variant", Value::string("link")),
                        ("href", Value::string(href)),
                        (
                            "className",
                            Value::string("h-auto px-0 text-xs font-normal text-muted-foreground hover:text-foreground"),
                        ),
                        ("label", Value::string(label)),
                    ],
                )),
                _ => nodes.push(pattern_spec(
                    "typography",
                    [
                        ("variant", Value::string("caption")),
                        ("as", Value::string("span")),
                        (
                            "className",
                            Value::string(if is_last {
                                "text-foreground font-medium"
                            } else {
                                "text-muted-foreground"
                            }),
                        ),
                        ("content", Value::string(label)),
                    ],
                )),
            }
        }
        let row = pattern_spec(
            "hstack",
            [
                ("gap", Value::string("xs")),
                ("align", Value::string("center")),
                ("wrap", Value::Bool(true)),
                ("children", array_value(nodes)),
            ],
        );
        let class = cn(&["w-full", &get_string_or(props, "className", "")]);
        let spec = layout_box_spec(
            &class,
            None,
            nav_landmark(props, &crate::i18n::t("aria.breadcrumb")),
            vec![row],
        );
        registry.render_spec(&spec, None, view)
    }
}
