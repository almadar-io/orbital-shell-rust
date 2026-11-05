//! Native Dioxus renderer for the `grid` pattern, mirroring the web `Grid` molecule.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use crate::styles::style_with_props;
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `grid` pattern.
pub struct GridRenderer;

impl GridRenderer {
    const NAME: &str = "grid";
}

fn cols_token(value: &Value) -> Option<String> {
    if value.as_str() == Some("none") {
        return Some("none".to_string());
    }
    value
        .as_int()
        .filter(|n| (1..=12).contains(n))
        .map(|n| n.to_string())
}

fn cols_class(value: &Value) -> Option<String> {
    if let Some(fields) = value.as_object() {
        let mut classes = Vec::new();
        for (key, prefix) in [
            ("base", ""),
            ("sm", "sm:"),
            ("md", "md:"),
            ("lg", "lg:"),
            ("xl", "xl:"),
        ] {
            if let Some(token) = fields.get(key).and_then(cols_token) {
                classes.push(format!("{prefix}grid-cols-{token}"));
            }
        }
        return Some(classes.join(" "));
    }
    cols_token(value).map(|token| format!("grid-cols-{token}"))
}

fn gap_step(gap: &str) -> Option<&'static str> {
    match gap {
        "none" => Some("0"),
        "xs" => Some("1"),
        "sm" => Some("2"),
        "md" => Some("4"),
        "lg" => Some("6"),
        "xl" => Some("8"),
        "2xl" => Some("12"),
        _ => None,
    }
}

fn mapped(props: &Value, key: &str, table: &[(&str, &str)]) -> Option<String> {
    let value = get_string(props, key)?;
    table
        .iter()
        .find(|(name, _)| *name == value)
        .map(|(_, class)| (*class).to_string())
}

impl DioxusPatternRenderer for GridRenderer {
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
        let cols = match props.as_object().and_then(|fields| fields.get("cols")) {
            Some(value) => cols_class(value),
            None => Some("grid-cols-1".to_string()),
        };
        let row_gap = get_string(props, "rowGap");
        let col_gap = get_string(props, "colGap");
        let gap = get_string_or(props, "gap", "md");
        let mut gap_classes = Vec::new();
        if let Some(step) = row_gap.as_deref().and_then(gap_step) {
            gap_classes.push(format!("gap-y-{step}"));
        } else if col_gap.is_none() {
            if let Some(step) = gap_step(&gap) {
                gap_classes.push(format!("gap-{step}"));
            }
        }
        if let Some(step) = col_gap.as_deref().and_then(gap_step) {
            gap_classes.push(format!("gap-x-{step}"));
        }
        let align = mapped(
            props,
            "alignItems",
            &[
                ("start", "items-start"),
                ("center", "items-center"),
                ("end", "items-end"),
                ("stretch", "items-stretch"),
                ("baseline", "items-baseline"),
            ],
        );
        let justify = mapped(
            props,
            "justifyItems",
            &[
                ("start", "justify-items-start"),
                ("center", "justify-items-center"),
                ("end", "justify-items-end"),
                ("stretch", "justify-items-stretch"),
            ],
        );
        let flow = mapped(
            props,
            "flow",
            &[
                ("row", "grid-flow-row"),
                ("col", "grid-flow-col"),
                ("row-dense", "grid-flow-row-dense"),
                ("col-dense", "grid-flow-col-dense"),
            ],
        );
        let mut parts = vec!["grid".to_string()];
        parts.extend(cols);
        parts.extend(gap_classes);
        parts.extend(align);
        parts.extend(justify);
        parts.extend(flow);
        let base = parts
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let class = merge_class_name(&base, props);
        let rows_style = match props
            .as_object()
            .and_then(|fields| fields.get("rows"))
            .and_then(Value::as_int)
        {
            Some(rows) if rows != 0 => {
                format!("grid-template-rows:repeat({rows}, minmax(0, 1fr))")
            }
            _ => String::new(),
        };
        let style = style_with_props(&rows_style, props);
        let children = child_patterns(props, view.trait_content);
        rsx! { div { class: "{class}", style: "{style}", ..shared_root_attrs(props),
            for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
        } }
    }
}
