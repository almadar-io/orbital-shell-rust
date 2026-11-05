//! Native geometric pattern: the web's SVG `<pattern>` tiling over the installed tile geometry.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    merge_class_name, pattern_tile_dimensions, pattern_tile_group, DioxusPatternRenderer,
    DioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;
use std::hash::{Hash, Hasher};

/// Dioxus renderer for the `geometric-pattern` pattern.
pub struct GeometricPatternRenderer;

impl GeometricPatternRenderer {
    const NAME: &str = "geometric-pattern";
}

struct Tile<'a> {
    variant: &'a str,
    size: f64,
    color: &'a str,
    stroke_width: f64,
    opacity: f64,
}

fn pattern_defs(id: &str, tile: &Tile<'_>) -> Element {
    let Some((width, height)) = pattern_tile_dimensions(tile.variant, tile.size) else {
        return rsx! {};
    };
    rsx! {
        defs {
            pattern { id: "{id}", x: "0", y: "0", width: "{width}", height: "{height}", pattern_units: "userSpaceOnUse",
                {pattern_tile_group(tile.variant, tile.size, tile.color, tile.stroke_width)}
            }
        }
    }
}

fn layer(mode: &str, id: &str, tile: &Tile<'_>, class: &str) -> Element {
    let opacity = tile.opacity.to_string();
    let fill = format!("url(#{id})");
    let defs = pattern_defs(id, tile);
    match mode {
        "left" | "right" => {
            let mask_id = format!("{id}-mask");
            let gradient_id = format!("{id}-grad");
            let (x1, x2) = if mode == "left" {
                ("0%", "60%")
            } else {
                ("100%", "40%")
            };
            let mask = format!("url(#{mask_id})");
            let gradient = format!("url(#{gradient_id})");
            rsx! {
                svg { class: "{class}", "preserveAspectRatio": "none", "aria-hidden": "true",
                    {defs}
                    defs {
                        linearGradient { id: "{gradient_id}", x1: "{x1}", y1: "0%", x2: "{x2}", y2: "0%",
                            stop { offset: "0%", stop_color: "white", stop_opacity: "1" }
                            stop { offset: "100%", stop_color: "white", stop_opacity: "0" }
                        }
                        mask { id: "{mask_id}",
                            rect { width: "100%", height: "100%", fill: "{gradient}" }
                        }
                    }
                    rect { width: "100%", height: "100%", fill: "{fill}", mask: "{mask}", opacity: "{opacity}" }
                }
            }
        }
        "dual" => {
            let mask_id = format!("{id}-dmask");
            let gradient_id = format!("{id}-dgrad");
            let mask = format!("url(#{mask_id})");
            let gradient = format!("url(#{gradient_id})");
            rsx! {
                svg { class: "{class}", "preserveAspectRatio": "none", "aria-hidden": "true",
                    {defs}
                    defs {
                        linearGradient { id: "{gradient_id}", x1: "0%", y1: "0%", x2: "100%", y2: "0%",
                            stop { offset: "0%", stop_color: "white", stop_opacity: "1" }
                            stop { offset: "35%", stop_color: "white", stop_opacity: "0" }
                            stop { offset: "65%", stop_color: "white", stop_opacity: "0" }
                            stop { offset: "100%", stop_color: "white", stop_opacity: "1" }
                        }
                        mask { id: "{mask_id}",
                            rect { width: "100%", height: "100%", fill: "{gradient}" }
                        }
                    }
                    rect { width: "100%", height: "100%", fill: "{fill}", mask: "{mask}", opacity: "{opacity}" }
                }
            }
        }
        "around" => {
            let mask_id = format!("{id}-amask");
            let gradient_id = format!("{id}-agrad");
            let mask = format!("url(#{mask_id})");
            let gradient = format!("url(#{gradient_id})");
            rsx! {
                svg { class: "{class}", "preserveAspectRatio": "none", "aria-hidden": "true",
                    {defs}
                    defs {
                        radialGradient { id: "{gradient_id}", cx: "50%", cy: "50%", r: "50%",
                            stop { offset: "0%", stop_color: "white", stop_opacity: "0" }
                            stop { offset: "55%", stop_color: "white", stop_opacity: "0" }
                            stop { offset: "85%", stop_color: "white", stop_opacity: "1" }
                            stop { offset: "100%", stop_color: "white", stop_opacity: "1" }
                        }
                        mask { id: "{mask_id}",
                            rect { width: "100%", height: "100%", fill: "{gradient}" }
                        }
                    }
                    rect { width: "100%", height: "100%", fill: "{fill}", mask: "{mask}", opacity: "{opacity}" }
                }
            }
        }
        _ => rsx! {
            svg { class: "{class}", "preserveAspectRatio": "none", "aria-hidden": "true",
                {defs}
                rect { width: "100%", height: "100%", fill: "{fill}", opacity: "{opacity}" }
            }
        },
    }
}

fn frame_strip(id: &str, tile: &Tile<'_>, height: f64) -> Element {
    let opacity = tile.opacity.to_string();
    let fill = format!("url(#{id})");
    let style = format!("height:{height}px");
    rsx! {
        svg { class: "w-full pointer-events-none", style: "{style}", "preserveAspectRatio": "none", "aria-hidden": "true",
            {pattern_defs(id, tile)}
            rect { width: "100%", height: "100%", fill: "{fill}", opacity: "{opacity}" }
        }
    }
}

impl DioxusPatternRenderer for GeometricPatternRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let variant = get_string_or(props, "variant", "star8");
        let mode = get_string_or(props, "mode", "background");
        let opacity = get_number_or(props, "opacity", 0.06);
        let color = get_string_or(props, "color", "var(--color-primary)");
        let scale = get_number_or(props, "scale", 1.0);
        let stroke_width = get_number_or(props, "strokeWidth", 0.5);
        let size = 60.0 * scale;
        let strip_height = 40.0 * scale;
        let tile = Tile {
            variant: &variant,
            size,
            color: &color,
            stroke_width,
            opacity,
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (&variant, size.to_bits(), &color, stroke_width.to_bits()).hash(&mut hasher);
        let pattern_id = format!("gp{:x}", hasher.finish());

        if mode == "frame" {
            let class = merge_class_name("bg-transparent rounded-none shadow-none relative", props);
            let children = child_patterns(props, view.trait_content);
            return rsx! {
                div { class: "{class}",
                    {frame_strip(&format!("{pattern_id}-top"), &tile, strip_height)}
                    for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
                    {frame_strip(&format!("{pattern_id}-bot"), &tile, strip_height)}
                }
            };
        }
        let class = merge_class_name("absolute inset-0 w-full h-full pointer-events-none", props);
        layer(&mode, &pattern_id, &tile, &class)
    }
}
