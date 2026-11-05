//! Native Dioxus renderer for the `svg-branch` pattern.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `svg-branch` pattern.
pub struct SvgBranchRenderer;

impl SvgBranchRenderer {
    const NAME: &str = "svg-branch";
}

impl DioxusPatternRenderer for SvgBranchRenderer {
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
        let x = get_number_or(props, "x", 5.0);
        let y = get_number_or(props, "y", 50.0);
        let variant = get_string_or(props, "variant", "fork");
        let branches = get_number_or(props, "branches", 2.0).max(0.0) as usize;
        let size = get_number_or(props, "size", 1.0);
        let color = get_color_or(props, "color", "var(--color-primary)");
        let opacity = get_number_or(props, "opacity", 1.0);
        let in_length = 30.0 * size;
        let out_length = 30.0 * size;
        let spread = 20.0 * size;
        let junction_x = x + in_length;
        let mut paths = Vec::with_capacity(branches + 1);
        if variant == "fork" {
            paths.push(format!("M {x} {y} L {junction_x} {y}"));
            for i in 0..branches {
                let fraction = if branches == 1 {
                    0.0
                } else {
                    i as f64 / (branches - 1) as f64 * 2.0 - 1.0
                };
                let end_y = y + fraction * spread;
                let end_x = junction_x + out_length;
                let cp_x = junction_x + out_length * 0.5;
                paths.push(format!(
                    "M {junction_x} {y} C {cp_x} {y} {cp_x} {end_y} {end_x} {end_y}"
                ));
            }
        } else if variant == "merge" {
            for i in 0..branches {
                let fraction = if branches == 1 {
                    0.0
                } else {
                    i as f64 / (branches - 1) as f64 * 2.0 - 1.0
                };
                let start_y = y + fraction * spread;
                let cp_x = x + in_length * 0.5;
                paths.push(format!(
                    "M {x} {start_y} C {cp_x} {start_y} {cp_x} {y} {junction_x} {y}"
                ));
            }
            paths.push(format!(
                "M {junction_x} {y} L {} {y}",
                junction_x + out_length
            ));
        }
        let diamond = format!(
            "{} {},{} {},{} {},{} {}",
            x + 20.0 * size,
            y,
            x + 40.0 * size,
            y + 14.0 * size,
            x + 20.0 * size,
            y + 28.0 * size,
            x,
            y + 14.0 * size
        );
        let as_root = get_bool_or(props, "asRoot", true);
        let width = get_number_or(props, "width", 100.0);
        let height = get_number_or(props, "height", 100.0);
        let inner_attrs = if as_root {
            Vec::new()
        } else {
            shared_root_attrs(props)
        };
        let inner = rsx! { g { class: "{class}", opacity: "{opacity}", ..inner_attrs,
            if variant == "diamond" { polygon { points: "{diamond}", fill: "none", stroke: "{color}", stroke_width: "2", stroke_linejoin: "round" } }
            for d in paths.iter() { path { d: "{d}", fill: "none", stroke: "{color}", stroke_width: "2", stroke_linecap: "round" } }
            if variant == "fork" || variant == "merge" { circle { cx: "{junction_x}", cy: "{y}", r: "3", fill: "{color}" } }
        } };
        if as_root {
            rsx! { svg { view_box: "0 0 {width} {height}", width: "{width}", height: "{height}", ..shared_root_attrs(props), {inner} } }
        } else {
            inner
        }
    }
}
