//! Native Dioxus renderer for the `svg-flow` pattern.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_MARKER: AtomicU64 = AtomicU64::new(1);

/// Dioxus renderer for the `svg-flow` pattern.
pub struct SvgFlowRenderer;

impl SvgFlowRenderer {
    const NAME: &str = "svg-flow";
}

impl DioxusPatternRenderer for SvgFlowRenderer {
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
        let points = if props
            .as_object()
            .is_some_and(|object| object.contains_key("points"))
        {
            get_array(props, "points")
                .into_iter()
                .filter_map(|point| {
                    let pair = point.as_array()?;
                    Some((pair.first()?.as_number()?, pair.get(1)?.as_number()?))
                })
                .collect::<Vec<_>>()
        } else {
            vec![(10.0, 50.0), (50.0, 20.0), (90.0, 50.0)]
        };
        if points.len() < 2 {
            return rsx! {};
        }
        let path_data = points
            .iter()
            .enumerate()
            .map(|(i, (x, y))| {
                if i == 0 {
                    format!("M{x},{y}")
                } else {
                    format!("L{x},{y}")
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let color = get_color_or(props, "color", "var(--color-primary)");
        let stroke_width = get_number_or(props, "strokeWidth", 1.5);
        let animated = get_bool_or(props, "animated", false);
        let opacity = get_number_or(props, "opacity", 1.0);
        let motion_class = if animated {
            "almadar-svg-flow-dash"
        } else {
            ""
        };
        let dash = if animated { "8 6" } else { "none" };
        let marker_id = format!(
            "almadar-flow-arrow-{}",
            NEXT_MARKER.fetch_add(1, Ordering::Relaxed)
        );
        let marker_end = format!("url(#{marker_id})");
        let as_root = get_bool_or(props, "asRoot", true);
        let width = get_number_or(props, "width", 100.0);
        let height = get_number_or(props, "height", 100.0);
        let inner_attrs = if as_root {
            Vec::new()
        } else {
            shared_root_attrs(props)
        };
        let inner = rsx! { g { class: "{class}", opacity: "{opacity}", ..inner_attrs,
            defs { marker { id: "{marker_id}", marker_width: "8", marker_height: "6", ref_x: "7", ref_y: "3", orient: "auto", marker_units: "strokeWidth",
                path { d: "M0,0 L8,3 L0,6 Z", fill: "{color}" }
            } }
            path { class: "{motion_class}", d: "{path_data}", fill: "none", stroke: "{color}", stroke_width: "{stroke_width}", stroke_linecap: "round", stroke_linejoin: "round", marker_end: "{marker_end}", stroke_dasharray: "{dash}" }
        } };
        if as_root {
            rsx! { svg { view_box: "0 0 {width} {height}", width: "{width}", height: "{height}", ..shared_root_attrs(props), {inner} } }
        } else {
            inner
        }
    }
}
