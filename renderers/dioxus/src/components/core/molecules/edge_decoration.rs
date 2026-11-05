//! Native Dioxus renderer for the `edge-decoration` pattern: the web's edge SVGs (arch, vine, lattice).

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{merge_class_name, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `edge-decoration` pattern.
pub struct EdgeDecorationRenderer;

impl EdgeDecorationRenderer {
    const NAME: &str = "edge-decoration";
}

fn f(n: f64) -> String {
    format!("{n:.2}")
}

fn arch_paths(right: bool, w: f64, h: f64) -> Vec<String> {
    let mut paths = Vec::new();
    let arch_count = 3;
    let arch_h = h / arch_count as f64;
    let rings = 5;
    let sweep = if right { 0 } else { 1 };
    let edge_x = if right { w } else { 0.0 };
    for a in 0..arch_count {
        let base_y = a as f64 * arch_h + arch_h;
        let top_y = a as f64 * arch_h + arch_h * 0.05;
        for r in 0..rings {
            let scale = 1.0 - r as f64 * 0.17;
            let arch_w = w * 0.95 * scale;
            let a_top_y = top_y + (1.0 - scale) * (base_y - top_y) * 0.5;
            let a_base_y = base_y - (1.0 - scale) * arch_h * 0.05;
            let inner_x = if right { w - arch_w } else { arch_w };
            let mid_y = (a_top_y + a_base_y) / 2.0;
            let radius = (arch_w * arch_w + (a_base_y - mid_y) * (a_base_y - mid_y)).sqrt();
            paths.push(format!(
                "M {},{} A {} {} 0 0 {} {},{} A {} {} 0 0 {} {},{}",
                f(edge_x),
                f(a_base_y),
                f(radius),
                f(radius),
                sweep,
                f(inner_x),
                f(a_top_y + (a_base_y - a_top_y) * 0.5),
                f(radius),
                f(radius),
                sweep,
                f(edge_x),
                f(a_top_y),
            ));
        }
        let inner_x = if right { w * 0.05 } else { w * 0.95 };
        paths.push(format!(
            "M {},{} L {},{}",
            f(edge_x),
            f(base_y),
            f(inner_x),
            f(base_y)
        ));
    }
    paths
}

fn vine_paths(right: bool, w: f64, h: f64) -> Vec<String> {
    let mut paths = Vec::new();
    let dir = if right { -1.0 } else { 1.0 };
    let edge_x = if right { w * 0.85 } else { w * 0.15 };
    let segments = 5;
    let seg_h = h / segments as f64;
    for i in 0..segments {
        let y0 = i as f64 * seg_h;
        let y1 = y0 + seg_h;
        let sway = w * 0.12 * if i % 2 == 0 { 1.0 } else { -1.0 };

        paths.push(format!(
            "M {},{} C {},{} {},{} {},{}",
            f(edge_x + sway),
            f(y0),
            f(edge_x - sway),
            f(y0 + seg_h * 0.33),
            f(edge_x + sway),
            f(y0 + seg_h * 0.66),
            f(edge_x - sway),
            f(y1),
        ));

        let branch_y = y0 + seg_h * 0.35;
        let branch_x = edge_x;
        let leaf_w = w * 0.55;
        let leaf_h = seg_h * 0.35;
        let leaf_tip_x = branch_x + dir * leaf_w;
        let leaf_tip_y = branch_y - leaf_h * 0.2;

        paths.push(format!(
            "M {},{} C {},{} {},{} {},{}",
            f(branch_x),
            f(branch_y),
            f(branch_x + dir * leaf_w * 0.3),
            f(branch_y - leaf_h),
            f(leaf_tip_x - dir * leaf_w * 0.1),
            f(leaf_tip_y - leaf_h * 0.3),
            f(leaf_tip_x),
            f(leaf_tip_y),
        ));
        paths.push(format!(
            "M {},{} C {},{} {},{} {},{}",
            f(leaf_tip_x),
            f(leaf_tip_y),
            f(leaf_tip_x - dir * leaf_w * 0.2),
            f(leaf_tip_y + leaf_h * 0.6),
            f(branch_x + dir * leaf_w * 0.15),
            f(branch_y + leaf_h * 0.4),
            f(branch_x),
            f(branch_y + leaf_h * 0.1),
        ));

        let tendril_y = y0 + seg_h * 0.7;
        let tendril_w = w * 0.35;
        let tendril_tip_x = edge_x + dir * tendril_w;
        paths.push(format!(
            "M {},{} Q {},{} {},{}",
            f(edge_x),
            f(tendril_y),
            f(edge_x + dir * tendril_w * 0.6),
            f(tendril_y - seg_h * 0.12),
            f(tendril_tip_x),
            f(tendril_y + seg_h * 0.05),
        ));

        let sp_r = leaf_w * 0.08;
        paths.push(format!(
            "M {},{} A {} {} 0 1 {} {},{}",
            f(leaf_tip_x),
            f(leaf_tip_y),
            f(sp_r),
            f(sp_r),
            if right { 0 } else { 1 },
            f(leaf_tip_x + dir * sp_r * 0.5),
            f(leaf_tip_y + sp_r),
        ));
    }
    paths
}

fn lattice_paths(w: f64, h: f64) -> Vec<String> {
    let mut paths = Vec::new();
    let cols = 5;
    let cell_w = w / cols as f64;
    let rows = (h / cell_w).ceil() as usize;
    let cell_h = cell_w;
    let bulge = cell_w * 0.3;
    for row in 0..rows {
        for col in 0..cols {
            let cx = col as f64 * cell_w + cell_w / 2.0;
            let cy = row as f64 * cell_h + cell_h / 2.0;
            let hw = cell_w * 0.5;
            let hh = cell_h * 0.5;
            paths.push(format!(
                "M {},{} Q {},{} {},{}",
                f(cx),
                f(cy - hh),
                f(cx + bulge),
                f(cy),
                f(cx),
                f(cy + hh)
            ));
            paths.push(format!(
                "M {},{} Q {},{} {},{}",
                f(cx),
                f(cy - hh),
                f(cx - bulge),
                f(cy),
                f(cx),
                f(cy + hh)
            ));
            paths.push(format!(
                "M {},{} Q {},{} {},{}",
                f(cx - hw),
                f(cy),
                f(cx),
                f(cy - bulge * 0.5),
                f(cx + hw),
                f(cy)
            ));
            paths.push(format!(
                "M {},{} Q {},{} {},{}",
                f(cx - hw),
                f(cy),
                f(cx),
                f(cy + bulge * 0.5),
                f(cx + hw),
                f(cy)
            ));
        }
    }
    paths
}

fn resolve_color(color: &str) -> String {
    match color {
        "primary" | "secondary" | "success" | "warning" | "error" | "muted" => {
            format!("var(--color-{color})")
        }
        raw => raw.to_string(),
    }
}

impl DioxusPatternRenderer for EdgeDecorationRenderer {
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
        let variant = get_string_or(props, "variant", "arch");
        let side = get_string_or(props, "side", "both");
        let opacity = get_number_or(props, "opacity", 0.15);
        let color = resolve_color(&get_string_or(props, "color", "primary"));
        let stroke_width = get_number_or(props, "strokeWidth", 0.5);
        let width = get_number_or(props, "width", 15.0);
        let sides: Vec<bool> = match side.as_str() {
            "both" => vec![false, true],
            "left" => vec![false],
            "right" => vec![true],
            other => {
                tracing::error!(
                    side = other,
                    "edge-decoration side is not left, right or both"
                );
                return rsx! {};
            }
        };
        if !matches!(variant.as_str(), "arch" | "vine" | "lattice") {
            tracing::error!(variant = %variant, "edge-decoration variant is not arch, vine or lattice");
            return rsx! {};
        }
        let (w, h) = (200.0, 600.0);
        let style = format!("width:{width}%;opacity:{opacity}");
        rsx! {
            for right in sides {
                {
                    let class = merge_class_name(
                        if right {
                            "absolute top-0 h-full pointer-events-none right-0"
                        } else {
                            "absolute top-0 h-full pointer-events-none left-0"
                        },
                        props,
                    );
                    let paths = match variant.as_str() {
                        "arch" => arch_paths(right, w, h),
                        "vine" => vine_paths(right, w, h),
                        _ => lattice_paths(w, h),
                    };
                    rsx! {
                        svg { key: "{right}", class: "{class}", style: "{style}", view_box: "0 0 200 600", "preserveAspectRatio": "none", "aria-hidden": "true",
                            g { fill: "none", stroke: "{color}", stroke_width: "{stroke_width}",
                                for (index, d) in paths.iter().enumerate() {
                                    path { key: "{index}", d: "{d}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
