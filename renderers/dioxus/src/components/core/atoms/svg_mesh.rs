//! Native Dioxus renderer for the `svg-mesh` pattern.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `svg-mesh` pattern.
pub struct SvgMeshRenderer;

impl SvgMeshRenderer {
    const NAME: &str = "svg-mesh";
}

impl DioxusPatternRenderer for SvgMeshRenderer {
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
        let cx = get_number_or(props, "cx", 60.0);
        let cy = get_number_or(props, "cy", 60.0);
        let nodes = get_number_or(props, "nodes", 6.0).max(0.0) as usize;
        let radius = get_number_or(props, "radius", 50.0);
        let color = get_color_or(props, "color", "var(--color-primary)");
        let density = get_number_or(props, "connectionDensity", 0.5).clamp(0.0, 1.0);
        let opacity = get_number_or(props, "opacity", 1.0);
        let positions: Vec<(f64, f64)> = (0..nodes)
            .map(|index| {
                let angle = index as f64 / nodes as f64 * std::f64::consts::TAU
                    - std::f64::consts::FRAC_PI_2;
                (cx + angle.cos() * radius, cy + angle.sin() * radius)
            })
            .collect();
        let all_pairs: Vec<(usize, usize)> = (0..nodes)
            .flat_map(|i| ((i + 1)..nodes).map(move |j| (i, j)))
            .collect();
        let connect_count = (all_pairs.len() as f64 * density).round() as usize;
        let connections: Vec<(f64, f64, f64, f64)> = all_pairs
            .into_iter()
            .take(connect_count)
            .map(|(a, b)| {
                (
                    positions[a].0,
                    positions[a].1,
                    positions[b].0,
                    positions[b].1,
                )
            })
            .collect();
        let as_root = get_bool_or(props, "asRoot", true);
        let width = get_number_or(props, "width", 120.0);
        let height = get_number_or(props, "height", 120.0);
        let inner_attrs = if as_root {
            Vec::new()
        } else {
            shared_root_attrs(props)
        };
        let inner = rsx! { g { class: "{class}", opacity: "{opacity}", ..inner_attrs,
            for (x1, y1, x2, y2) in connections.iter() { line { x1: "{x1}", y1: "{y1}", x2: "{x2}", y2: "{y2}", stroke: "{color}", stroke_width: "1", opacity: "0.3" } }
            for (px, py) in positions.iter() { circle { cx: "{px}", cy: "{py}", r: "4", fill: "{color}" } }
        } };
        if as_root {
            rsx! { svg { view_box: "0 0 {width} {height}", width: "{width}", height: "{height}", ..shared_root_attrs(props), {inner} } }
        } else {
            inner
        }
    }
}
