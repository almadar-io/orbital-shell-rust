//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{object_value, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `markdown-content` pattern.
pub struct MarkdownContentRenderer;

impl MarkdownContentRenderer {
    const NAME: &str = "markdown-content";
}

const PROSE_VARS: [(&str, &str); 17] = [
    ("color", "var(--color-foreground)"),
    ("--tw-prose-body", "var(--color-foreground)"),
    ("--tw-prose-headings", "var(--color-foreground)"),
    ("--tw-prose-lead", "var(--color-muted-foreground)"),
    ("--tw-prose-links", "var(--color-primary)"),
    ("--tw-prose-bold", "var(--color-foreground)"),
    ("--tw-prose-counters", "var(--color-muted-foreground)"),
    ("--tw-prose-bullets", "var(--color-muted-foreground)"),
    ("--tw-prose-hr", "var(--color-border)"),
    ("--tw-prose-quotes", "var(--color-foreground)"),
    ("--tw-prose-quote-borders", "var(--color-primary)"),
    ("--tw-prose-captions", "var(--color-muted-foreground)"),
    ("--tw-prose-code", "var(--color-foreground)"),
    ("--tw-prose-pre-code", "var(--color-foreground)"),
    ("--tw-prose-pre-bg", "var(--color-muted)"),
    ("--tw-prose-th-borders", "var(--color-border)"),
    ("--tw-prose-td-borders", "var(--color-border)"),
];

impl DioxusPatternRenderer for MarkdownContentRenderer {
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
        let direction = get_string_or(props, "direction", "ltr");
        let mut style: Vec<(&str, Value)> = PROSE_VARS
            .iter()
            .map(|(name, value)| (*name, Value::string(*value)))
            .collect();
        style.push(("direction", Value::string(direction)));
        let mut fields = vec![
            (
                "className",
                Value::string(format!(
                    "prose max-w-none {}",
                    get_string_or(props, "className", "")
                )),
            ),
            ("style", object_value(style)),
            (
                "children",
                Value::Array(
                    vec![pattern_spec(
                        "typography",
                        [
                            ("content", Value::string(content)),
                            ("variant", Value::string("body")),
                            ("color", Value::string("inherit")),
                            (
                                "className",
                                Value::string("whitespace-pre-wrap break-words"),
                            ),
                        ],
                    )]
                    .into(),
                ),
            ),
        ];
        for key in ["id", "aria-label", "role"] {
            if let Some(value) = props.get(key) {
                fields.push((key, value.clone()));
            }
        }
        let spec = pattern_spec("box", fields);
        registry.render_spec(&spec, None, view)
    }
}
