//! Native Dioxus renderer for the `svg-connection` pattern.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `svg-connection` pattern.
pub struct SvgConnectionRenderer;

impl SvgConnectionRenderer {
    const NAME: &str = "svg-connection";
}

impl DioxusPatternRenderer for SvgConnectionRenderer {
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
        let variant = get_string_or(props, "variant", "solid");
        let base = if variant == "animated" {
            "almadar-svg-flow-dash"
        } else {
            ""
        };
        let class = merge_class_name(base, props);
        let x1 = get_number_or(props, "x1", 10.0);
        let y1 = get_number_or(props, "y1", 50.0);
        let x2 = get_number_or(props, "x2", 90.0);
        let y2 = get_number_or(props, "y2", 50.0);
        let color = get_color_or(props, "color", "var(--color-primary)");
        let stroke_width = get_number_or(props, "strokeWidth", 1.5);
        let opacity = get_number_or(props, "opacity", 1.0);
        let as_root = get_bool_or(props, "asRoot", true);
        let width = get_number_or(props, "width", 100.0);
        let height = get_number_or(props, "height", 100.0);
        let dash = if variant == "solid" { "none" } else { "8 6" };
        let inner_attrs = if as_root {
            Vec::new()
        } else {
            shared_root_attrs(props)
        };
        let line = rsx! { line { class: "{class}", x1: "{x1}", y1: "{y1}", x2: "{x2}", y2: "{y2}", stroke: "{color}", stroke_width: "{stroke_width}", stroke_linecap: "round", stroke_dasharray: "{dash}", opacity: "{opacity}", ..inner_attrs } };
        if as_root {
            rsx! { svg { view_box: "0 0 {width} {height}", width: "{width}", height: "{height}", ..shared_root_attrs(props), {line} } }
        } else {
            line
        }
    }
}
