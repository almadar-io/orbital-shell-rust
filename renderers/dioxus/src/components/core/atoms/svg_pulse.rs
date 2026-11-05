//! Native Dioxus renderer for the `svg-pulse` pattern.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `svg-pulse` pattern.
pub struct SvgPulseRenderer;

impl SvgPulseRenderer {
    const NAME: &str = "svg-pulse";
}

impl DioxusPatternRenderer for SvgPulseRenderer {
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
        let cx = get_number_or(props, "cx", 70.0);
        let cy = get_number_or(props, "cy", 70.0);
        let rings = get_number_or(props, "rings", 3.0).max(0.0) as usize;
        let max_radius = get_number_or(props, "maxRadius", 60.0);
        let color = get_color_or(props, "color", "var(--color-primary)");
        let animated = get_bool_or(props, "animated", true);
        let opacity = get_number_or(props, "opacity", 1.0);
        let specs: Vec<(f64, f64, f64)> = (0..rings)
            .map(|i| {
                let t = (i + 1) as f64 / rings as f64;
                (
                    t * max_radius,
                    (1.0 - i as f64 / rings as f64 * 0.6) * 0.4,
                    i as f64 * 0.6,
                )
            })
            .collect();
        let pulse_class = if animated {
            "almadar-svg-pulse-expand"
        } else {
            ""
        };
        let as_root = get_bool_or(props, "asRoot", true);
        let width = get_number_or(props, "width", 140.0);
        let height = get_number_or(props, "height", 140.0);
        let inner_attrs = if as_root {
            Vec::new()
        } else {
            shared_root_attrs(props)
        };
        let inner = rsx! { g { class: "{class}", opacity: "{opacity}", ..inner_attrs,
            for (radius, ring_opacity, delay) in specs.iter() {
                circle { class: "{pulse_class}", cx: "{cx}", cy: "{cy}", r: "{radius}", fill: "none", stroke: "{color}", stroke_width: "1.5", opacity: "{ring_opacity}", style: "animation-delay:{delay}s" }
            }
            circle { cx: "{cx}", cy: "{cy}", r: "3", fill: "{color}" }
        } };
        if as_root {
            rsx! { svg { view_box: "0 0 {width} {height}", width: "{width}", height: "{height}", ..shared_root_attrs(props), {inner} } }
        } else {
            inner
        }
    }
}
