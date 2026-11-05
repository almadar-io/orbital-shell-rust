//! Native Dioxus renderer for the `svg-lobe` pattern.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `svg-lobe` pattern.
pub struct SvgLobeRenderer;

impl SvgLobeRenderer {
    const NAME: &str = "svg-lobe";
}

impl DioxusPatternRenderer for SvgLobeRenderer {
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
        let cx = get_number_or(props, "cx", 50.0);
        let cy = get_number_or(props, "cy", 50.0);
        let rx = get_number_or(props, "rx", 14.0);
        let ry = get_number_or(props, "ry", 20.0);
        let rotation = get_number_or(props, "rotation", 0.0);
        let shells = (get_number_or(props, "shells", 2.0) as usize).clamp(1, 3);
        let color = get_color_or(props, "color", "var(--color-primary)");
        let opacity = get_number_or(props, "opacity", 1.0);
        let transform = format!("rotate({rotation}, {cx}, {cy})");
        let specs: Vec<(f64, f64, f64, f64, f64)> = (0..shells)
            .map(|index| {
                let scale = if index == 0 {
                    1.0
                } else {
                    1.0 - index as f64 * 0.3
                };
                let shell_opacity = if index == 0 { 1.0 } else { 0.5 };
                let shell_rx = rx * scale;
                let shell_ry = ry * scale;
                (
                    shell_rx,
                    shell_ry,
                    cy - shell_ry,
                    cy + shell_ry,
                    shell_opacity,
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
        let inner = rsx! { g { class: "{class}", opacity: "{opacity}", transform: "{transform}", ..inner_attrs,
            for (shell_rx, shell_ry, upper_y, lower_y, shell_opacity) in specs.iter() {
                g { opacity: "{shell_opacity}",
                    ellipse { cx: "{cx}", cy: "{upper_y}", rx: "{shell_rx}", ry: "{shell_ry}", fill: "none", stroke: "{color}", stroke_width: "1.5" }
                    ellipse { cx: "{cx}", cy: "{lower_y}", rx: "{shell_rx}", ry: "{shell_ry}", fill: "none", stroke: "{color}", stroke_width: "1.5" }
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
