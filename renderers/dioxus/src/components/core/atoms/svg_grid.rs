//! Native Dioxus renderer for the `svg-grid` pattern.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `svg-grid` pattern.
pub struct SvgGridRenderer;

impl SvgGridRenderer {
    const NAME: &str = "svg-grid";
}

impl DioxusPatternRenderer for SvgGridRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        _view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let class = merge_class_name("", props);
        let x = get_number_or(props, "x", 10.0);
        let y = get_number_or(props, "y", 10.0);
        let cols = get_number_or(props, "cols", 4.0).max(0.0) as usize;
        let rows = get_number_or(props, "rows", 3.0).max(0.0) as usize;
        let spacing = get_number_or(props, "spacing", 20.0);
        let node_radius = get_number_or(props, "nodeRadius", 3.0);
        let color = get_color_or(props, "color", "var(--color-primary)");
        let opacity = get_number_or(props, "opacity", 1.0);
        let highlights = get_array(props, "highlights");
        let circles: Vec<(f64, f64, f64, f64)> = (0..rows)
            .flat_map(|row| (0..cols).map(move |col| (row, col)))
            .map(|(row, col)| {
                let highlighted = highlights
                    .iter()
                    .any(|value| value.as_int() == Some((row * cols + col) as i64));
                (
                    x + col as f64 * spacing,
                    y + row as f64 * spacing,
                    if highlighted {
                        node_radius * 1.6
                    } else {
                        node_radius
                    },
                    if highlighted { 1.0 } else { 0.4 },
                )
            })
            .collect();
        let as_root = get_bool_or(props, "asRoot", true);
        let width = get_number_or(props, "width", 100.0);
        let height = get_number_or(props, "height", 100.0);
        let inner_attrs = if as_root {
            Vec::new()
        } else {
            shared_root_attrs(props)
        };
        let inner = rsx! { g { class: "{class}", opacity: "{opacity}", ..inner_attrs,
            for (cx, cy, r, dot_opacity) in circles.iter() {
                circle { cx: "{cx}", cy: "{cy}", r: "{r}", fill: "{color}", opacity: "{dot_opacity}" }
            }
        } };
        if as_root {
            rsx! { svg { view_box: "0 0 {width} {height}", width: "{width}", height: "{height}", ..shared_root_attrs(props), {inner} } }
        } else {
            inner
        }
    }
}
