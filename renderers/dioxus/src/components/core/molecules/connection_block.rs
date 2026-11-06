//! Native body of the `connection-block` pattern, mirroring the web `ConnectionBlock` molecule.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{array_value, cn, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `connection-block` pattern.
pub struct ConnectionBlockRenderer;

impl ConnectionBlockRenderer {
    const NAME: &str = "connection-block";
}

impl DioxusPatternRenderer for ConnectionBlockRenderer {
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
        let content = get_string_or(props, "content", "");
        let class = cn(&[
            "bg-success/10 border-s-heavy border-success rounded-e-container p-5 mb-6",
            get_string_or(props, "className", "").trim(),
        ]);
        let icon = pattern_spec(
            "icon",
            [
                ("name", Value::string("link-2")),
                ("size", Value::string("sm")),
                (
                    "className",
                    Value::string("w-5 h-5 text-success flex-shrink-0 mt-1"),
                ),
            ],
        );
        let title = pattern_spec(
            "typography",
            [
                ("variant", Value::string("h6")),
                ("as", Value::string("h4")),
                ("className", Value::string("mb-2")),
                ("content", Value::string("Building On What You Know")),
            ],
        );
        let markdown = pattern_spec(
            "markdown-content",
            [
                ("content", Value::string(content)),
                ("className", Value::string("text-sm")),
            ],
        );
        let body = pattern_spec(
            "box",
            [
                ("className", Value::string("flex-1")),
                ("children", array_value(vec![title, markdown])),
            ],
        );
        let row = pattern_spec(
            "hstack",
            [
                ("gap", Value::string("sm")),
                ("align", Value::string("start")),
                ("children", array_value(vec![icon, body])),
            ],
        );
        let mut fields = vec![
            ("className", Value::string(class)),
            ("children", array_value(vec![row])),
        ];
        for key in ["id", "aria-label", "role"] {
            if let Some(value) = props.get(key) {
                fields.push((key, value.clone()));
            }
        }
        registry.render_spec(&pattern_spec("box", fields), None, view)
    }
}
