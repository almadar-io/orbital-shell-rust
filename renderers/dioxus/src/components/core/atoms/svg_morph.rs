//! Native Dioxus renderer for the `svg-morph` pattern.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct SvgMorphRenderer;

impl DioxusPatternRenderer for SvgMorphRenderer {
    fn pattern_name(&self) -> &str {
        "svg-morph"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        _view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let class = merge_class_name("", props);
        let x = get_number_or(props, "x", 5.0);
        let y = get_number_or(props, "y", 10.0);
        let size = get_number_or(props, "size", 1.0);
        let variant = get_string_or(props, "variant", "generic");
        let color = get_color_or(props, "color", "var(--color-primary)");
        let opacity = get_number_or(props, "opacity", 1.0);
        let gap = 40.0 * size;
        let mid_y = y + 10.0 * size;
        let content = match variant.as_str() {
            "text-to-code" => {
                let left_end = x + 30.0 * size;
                let right_start = left_end + gap;
                rsx! { {text_lines(x, y, size, &color)} {flow_arrow(left_end, mid_y, right_start, size, &color)} {code_brackets(right_start, y, size, &color)} }
            }
            "code-to-app" => {
                let left_end = x + 26.0 * size;
                let right_start = left_end + gap;
                rsx! { {code_brackets(x, y, size, &color)} {flow_arrow(left_end, mid_y, right_start, size, &color)} {app_rect(right_start, y, size, &color)} }
            }
            _ => {
                let radius = 10.0 * size;
                let circle_x = x + radius;
                let square_x = x + radius * 2.0 + gap;
                let arrow_x = circle_x + radius + 2.0 * size;
                let square_y = mid_y - radius;
                let square_size = radius * 2.0;
                let corner = 3.0 * size;
                let stroke = 2.0 * size;
                rsx! {
                    circle { cx: "{circle_x}", cy: "{mid_y}", r: "{radius}", fill: "none", stroke: "{color}", stroke_width: "{stroke}" }
                    {flow_arrow(arrow_x, mid_y, square_x, size, &color)}
                    rect { x: "{square_x}", y: "{square_y}", width: "{square_size}", height: "{square_size}", rx: "{corner}", fill: "none", stroke: "{color}", stroke_width: "{stroke}" }
                }
            }
        };
        let as_root = get_bool_or(props, "asRoot", true);
        let width = get_number_or(props, "width", 130.0);
        let height = get_number_or(props, "height", 50.0);
        let inner_attrs = if as_root {
            Vec::new()
        } else {
            shared_root_attrs(props)
        };
        let inner = rsx! { g { class: "{class}", opacity: "{opacity}", ..inner_attrs, {content} } };
        if as_root {
            rsx! { svg { view_box: "0 0 {width} {height}", width: "{width}", height: "{height}", ..shared_root_attrs(props), {inner} } }
        } else {
            inner
        }
    }
}

fn text_lines(x: f64, y: f64, size: f64, color: &str) -> Element {
    let lines = [(28.0, 0.7), (22.0, 0.6), (26.0, 0.5)];
    let height = 3.0 * size;
    let corner = 1.5 * size;
    rsx! { g { for (index, (width, opacity)) in lines.iter().enumerate() {
        rect { x: "{x}", y: "{y + index as f64 * 8.0 * size}", width: "{width * size}", height: "{height}", rx: "{corner}", fill: "{color}", opacity: "{opacity}" }
    } } }
}

fn code_brackets(x: f64, y: f64, size: f64, color: &str) -> Element {
    let h = 20.0 * size;
    let w = 8.0 * size;
    let left = format!(
        "M {} {y} L {x} {} L {} {}",
        x + w,
        y + h / 2.0,
        x + w,
        y + h
    );
    let right = format!(
        "M {} {y} L {} {} L {} {}",
        x + w * 2.0,
        x + w * 3.0,
        y + h / 2.0,
        x + w * 2.0,
        y + h
    );
    let stroke = 2.0 * size;
    rsx! { g {
        path { d: "{left}", fill: "none", stroke: "{color}", stroke_width: "{stroke}", stroke_linecap: "round", stroke_linejoin: "round" }
        path { d: "{right}", fill: "none", stroke: "{color}", stroke_width: "{stroke}", stroke_linecap: "round", stroke_linejoin: "round" }
    } }
}

fn app_rect(x: f64, y: f64, size: f64, color: &str) -> Element {
    let w = 22.0 * size;
    let h = 20.0 * size;
    let stroke = 2.0 * size;
    let corner = 4.0 * size;
    let inner_x = x + 4.0 * size;
    let first_y = y + 4.0 * size;
    let second_y = y + 10.0 * size;
    let first_width = w - 8.0 * size;
    let second_width = first_width * 0.6;
    let bar_height = 3.0 * size;
    let bar_corner = size;
    rsx! { g {
        rect { x: "{x}", y: "{y}", width: "{w}", height: "{h}", rx: "{corner}", fill: "none", stroke: "{color}", stroke_width: "{stroke}" }
        rect { x: "{inner_x}", y: "{first_y}", width: "{first_width}", height: "{bar_height}", rx: "{bar_corner}", fill: "{color}", opacity: "0.5" }
        rect { x: "{inner_x}", y: "{second_y}", width: "{second_width}", height: "{bar_height}", rx: "{bar_corner}", fill: "{color}", opacity: "0.3" }
    } }
}

fn flow_arrow(x1: f64, y: f64, x2: f64, size: f64, color: &str) -> Element {
    let arrow_size = 4.0 * size;
    let line_end = x2 - arrow_size;
    let stroke = 1.5 * size;
    let dash = format!("{} {}", 3.0 * size, 3.0 * size);
    let points = format!(
        "{x2} {y}, {line_end} {}, {line_end} {}",
        y - arrow_size / 2.0,
        y + arrow_size / 2.0
    );
    rsx! { g {
        line { x1: "{x1}", y1: "{y}", x2: "{line_end}", y2: "{y}", stroke: "{color}", stroke_width: "{stroke}", stroke_dasharray: "{dash}", opacity: "0.5" }
        polygon { points: "{points}", fill: "{color}", opacity: "0.6" }
    } }
}
