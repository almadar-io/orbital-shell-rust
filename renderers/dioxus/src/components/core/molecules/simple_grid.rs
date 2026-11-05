//! Native Dioxus renderer for the `simple-grid` pattern, mirroring the web `SimpleGrid` molecule.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `simple-grid` pattern.
pub struct SimpleGridRenderer;

impl SimpleGridRenderer {
    const NAME: &str = "simple-grid";
}

fn gap_class_and_length(gap: &str) -> (&'static str, &'static str) {
    match gap {
        "none" => ("gap-0", "0px"),
        "xs" => ("gap-1", "0.25rem"),
        "sm" => ("gap-2", "0.5rem"),
        "lg" => ("gap-6", "1.5rem"),
        "xl" => ("gap-8", "2rem"),
        "md" => ("gap-4", "1rem"),
        _ => ("", "1rem"),
    }
}

fn cols_class(cols: i64) -> &'static str {
    match cols {
        1 => "grid-cols-1",
        2 => "grid-cols-1 sm:grid-cols-2 @max-sm:!grid-cols-1",
        3 => "grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 @max-lg:!grid-cols-2 @max-sm:!grid-cols-1",
        4 => "grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 @max-lg:!grid-cols-3 @max-md:!grid-cols-2 @max-sm:!grid-cols-1",
        5 => "grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 @max-xl:!grid-cols-4 @max-lg:!grid-cols-3 @max-md:!grid-cols-2 @max-sm:!grid-cols-1",
        6 => "grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6 @max-xl:!grid-cols-4 @max-lg:!grid-cols-3 @max-md:!grid-cols-2 @max-sm:!grid-cols-1",
        _ => "",
    }
}

impl DioxusPatternRenderer for SimpleGridRenderer {
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
        let (gap_class, gap_length) = gap_class_and_length(&get_string_or(props, "gap", "md"));
        let extra_class = get_string_or(props, "className", "");
        let fields = props.as_object();
        let cols = fields
            .and_then(|fields| fields.get("cols"))
            .and_then(Value::as_int)
            .filter(|cols| *cols != 0);
        let children = child_patterns(props, view.trait_content);
        let box_base = "bg-transparent rounded-none shadow-none";

        let (class, style) = if let Some(cols) = cols {
            (
                cn(&[box_base, "grid", cols_class(cols), gap_class, &extra_class]),
                String::new(),
            )
        } else {
            let min_width = match fields.and_then(|fields| fields.get("minChildWidth")) {
                Some(value) if value.is_numeric() => format!("{}px", value.to_string_value()),
                Some(Value::String(text)) => text.to_string(),
                _ => "250px".to_string(),
            };
            let max_cols = fields
                .and_then(|fields| fields.get("maxCols"))
                .and_then(Value::as_int)
                .filter(|max| *max != 0);
            let template = match max_cols {
                Some(max) => format!(
                    "repeat(auto-fit, minmax(max(min({min_width}, 100%), calc((100% - {} * {gap_length}) / {max})), 1fr))",
                    max - 1
                ),
                None => format!("repeat(auto-fit, minmax({min_width}, 1fr))"),
            };
            (
                cn(&[box_base, "grid", gap_class, &extra_class]),
                format!("grid-template-columns:{template}"),
            )
        };
        rsx! { div { class: "{class}", style: "{style}", ..shared_root_attrs(props),
            for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
        } }
    }
}
