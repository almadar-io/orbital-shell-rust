//! Native Dioxus renderer for the `svg-shield` pattern.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `svg-shield` pattern.
pub struct SvgShieldRenderer;

impl SvgShieldRenderer {
    const NAME: &str = "svg-shield";
}

impl DioxusPatternRenderer for SvgShieldRenderer {
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
        let size = get_number_or(props, "size", 1.0);
        let variant = get_string_or(props, "variant", "outline");
        let color = get_color_or(props, "color", "var(--color-primary)");
        let opacity = get_number_or(props, "opacity", 1.0);
        let transform = format!(
            "translate({}, {}) scale({size})",
            x - 15.0 * size,
            y - 18.0 * size
        );
        let fill = if variant == "filled" || variant == "check" {
            color.as_str()
        } else {
            "none"
        };
        let as_root = get_bool_or(props, "asRoot", true);
        let width = get_number_or(props, "width", 100.0);
        let height = get_number_or(props, "height", 100.0);
        let inner_attrs = if as_root {
            Vec::new()
        } else {
            shared_root_attrs(props)
        };
        let inner = rsx! { g { class: "{class}", opacity: "{opacity}", transform: "{transform}", ..inner_attrs,
            path { d: "M15,2 C15,2 5,5 2,6 C2,6 2,18 5,24 C8,30 15,34 15,34 C15,34 22,30 25,24 C28,18 28,6 28,6 C25,5 15,2 15,2 Z", fill: "{fill}", stroke: "{color}", stroke_width: "2", stroke_linejoin: "round" }
            if variant == "check" {
                path { d: "M10,18 L14,22 L21,13", fill: "none", stroke: "var(--surface-diagram)", stroke_width: "2.5", stroke_linecap: "round", stroke_linejoin: "round" }
            }
        } };
        if as_root {
            rsx! { svg { view_box: "0 0 {width} {height}", width: "{width}", height: "{height}", ..shared_root_attrs(props), {inner} } }
        } else {
            inner
        }
    }
}
