//! Native Dioxus renderer for the `svg-node` pattern.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `svg-node` pattern.
pub struct SvgNodeRenderer;

impl SvgNodeRenderer {
    const NAME: &str = "svg-node";
}

impl DioxusPatternRenderer for SvgNodeRenderer {
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
        let x = get_number_or(props, "x", 50.0);
        let y = get_number_or(props, "y", 50.0);
        let r = get_number_or(props, "r", 6.0);
        let variant = get_string_or(props, "variant", "filled");
        let color = get_color_or(props, "color", "var(--color-primary)");
        let opacity = get_number_or(props, "opacity", 1.0);
        let label = get_string_or(props, "label", "");
        let fill = if variant == "stroked" {
            "none"
        } else {
            color.as_str()
        };
        let stroke = if variant == "stroked" {
            color.as_str()
        } else {
            "none"
        };
        let stroke_width = if variant == "stroked" { 2 } else { 0 };
        let pulse_radius = r * 2.0;
        let label_y = y + r + 14.0;
        let as_root = get_bool_or(props, "asRoot", true);
        let width = get_number_or(props, "width", 100.0);
        let height = get_number_or(props, "height", 100.0);
        let inner_attrs = if as_root {
            Vec::new()
        } else {
            shared_root_attrs(props)
        };
        let inner = rsx! { g { class: "{class}", opacity: "{opacity}", ..inner_attrs,
            if variant == "pulse" {
                circle { class: "almadar-svg-pulse-ring", cx: "{x}", cy: "{y}", r: "{pulse_radius}", fill: "none", stroke: "{color}", stroke_width: "1", opacity: "0.3" }
            }
            circle { cx: "{x}", cy: "{y}", r: "{r}", fill: "{fill}", stroke: "{stroke}", stroke_width: "{stroke_width}" }
            if !label.is_empty() { text { x: "{x}", y: "{label_y}", text_anchor: "middle", fill: "{color}", font_size: "11", font_family: "inherit", "{label}" } }
        } };
        if as_root {
            rsx! { svg { view_box: "0 0 {width} {height}", width: "{width}", height: "{height}", ..shared_root_attrs(props), {inner} } }
        } else {
            inner
        }
    }
}
