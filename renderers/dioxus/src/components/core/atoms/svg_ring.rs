//! Native Dioxus renderer for the `svg-ring` pattern.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_GRADIENT: AtomicU64 = AtomicU64::new(1);

/// Dioxus renderer for the `svg-ring` pattern.
pub struct SvgRingRenderer;

impl SvgRingRenderer {
    const NAME: &str = "svg-ring";
}

impl DioxusPatternRenderer for SvgRingRenderer {
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
        let r = get_number_or(props, "r", 40.0);
        let variant = get_string_or(props, "variant", "solid");
        let color = get_color_or(props, "color", "var(--color-primary)");
        let stroke_width = get_number_or(props, "strokeWidth", 1.5);
        let opacity = get_number_or(props, "opacity", 1.0);
        let label = get_string_or(props, "label", "");
        let label_y = cy - r - 6.0;
        let dash = if variant == "dashed" { "6 4" } else { "none" };
        let gradient_id = format!(
            "almadar-ring-glow-{}",
            NEXT_GRADIENT.fetch_add(1, Ordering::Relaxed)
        );
        let gradient_fill = format!("url(#{gradient_id})");
        let as_root = get_bool_or(props, "asRoot", true);
        let width = get_number_or(props, "width", 100.0);
        let height = get_number_or(props, "height", 100.0);
        let inner_attrs = if as_root {
            Vec::new()
        } else {
            shared_root_attrs(props)
        };
        let inner = rsx! { g { class: "{class}", opacity: "{opacity}", ..inner_attrs,
            if variant == "glow" {
                defs { radialGradient { id: "{gradient_id}", cx: "50%", cy: "50%", r: "50%",
                    stop { offset: "0%", stop_color: "{color}", stop_opacity: "0.15" }
                    stop { offset: "100%", stop_color: "{color}", stop_opacity: "0" }
                } }
                circle { cx: "{cx}", cy: "{cy}", r: "{r}", fill: "{gradient_fill}" }
            }
            circle { cx: "{cx}", cy: "{cy}", r: "{r}", fill: "none", stroke: "{color}", stroke_width: "{stroke_width}", stroke_dasharray: "{dash}" }
            if !label.is_empty() { text { x: "{cx}", y: "{label_y}", text_anchor: "middle", fill: "{color}", font_size: "11", font_family: "inherit", "{label}" } }
        } };
        if as_root {
            rsx! { svg { view_box: "0 0 {width} {height}", width: "{width}", height: "{height}", ..shared_root_attrs(props), {inner} } }
        } else {
            inner
        }
    }
}
