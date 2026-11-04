//! Native Dioxus renderer for the `svg-stack` pattern.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `svg-stack` pattern.
pub struct SvgStackRenderer;

impl SvgStackRenderer {
    const NAME: &str = "svg-stack";
}

impl DioxusPatternRenderer for SvgStackRenderer {
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
        let y = get_number_or(props, "y", 40.0);
        let layers = (get_number_or(props, "layers", 3.0) as usize).clamp(2, 4);
        let layer_width = get_number_or(props, "width", 60.0);
        let layer_height = get_number_or(props, "height", 40.0);
        let color = get_color_or(props, "color", "var(--color-primary)");
        let opacity = get_number_or(props, "opacity", 1.0);
        let labels = get_string_array(props, "labels");
        let specs: Vec<(f64, f64, f64, String, f64)> = (0..layers)
            .rev()
            .map(|index| {
                let layer_opacity = 0.3 + index as f64 / (layers - 1) as f64 * 0.7;
                (
                    x + index as f64 * 4.0,
                    y - index as f64 * 8.0,
                    layer_opacity,
                    labels.get(index).cloned().unwrap_or_default(),
                    layer_opacity * 0.6,
                )
            })
            .collect();
        let as_root = get_bool_or(props, "asRoot", true);
        let width = get_number_or(props, "svgWidth", 90.0);
        let height = get_number_or(props, "svgHeight", 80.0);
        let inner_attrs = if as_root {
            Vec::new()
        } else {
            shared_root_attrs(props)
        };
        let inner = rsx! { g { class: "{class}", opacity: "{opacity}", ..inner_attrs,
            for (layer_x, layer_y, layer_opacity, label, stroke_opacity) in specs.iter() {
                g {
                    rect { x: "{layer_x}", y: "{layer_y}", width: "{layer_width}", height: "{layer_height}", rx: "6", ry: "6", fill: "{color}", opacity: "{layer_opacity}", stroke: "{color}", stroke_width: "1", stroke_opacity: "{stroke_opacity}" }
                    if !label.is_empty() { text { x: "{layer_x + layer_width / 2.0}", y: "{layer_y + layer_height / 2.0 + 4.0}", text_anchor: "middle", fill: "var(--surface-diagram)", font_size: "10", font_family: "inherit", opacity: "{layer_opacity}", "{label}" } }
                }
            }
        } };
        if as_root {
            rsx! { svg { view_box: "0 0 {width} {height}", width: "{width}", height: "{height}", ..shared_root_attrs(props), {inner} } }
        } else {
            inner
        }
    }
}
