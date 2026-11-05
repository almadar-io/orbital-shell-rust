//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `progress-bar` pattern.
pub struct ProgressBarRenderer;

impl ProgressBarRenderer {
    const NAME: &str = "progress-bar";
}

impl DioxusPatternRenderer for ProgressBarRenderer {
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
        let value = get_number_or(props, "value", 0.0);
        let max = get_number_or(props, "max", 100.0);
        let percentage = if max > 0.0 {
            ((value / max) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
        let width = format!("{percentage}%");
        let percent_text = format!("{}%", percentage.round() as i64);
        let progress_type = get_string_or(props, "progressType", "linear");
        let color = get_string(props, "color")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| get_string_or(props, "variant", "primary"));
        let fill = match color.as_str() {
            "success" => "bg-success",
            "warning" => "bg-warning",
            "error" | "danger" => "bg-error",
            _ => "bg-primary",
        };
        let stroke = match color.as_str() {
            "success" => "var(--color-success)",
            "warning" => "var(--color-warning)",
            "error" | "danger" => "var(--color-error)",
            _ => "var(--color-primary)",
        };
        let show_percentage =
            get_bool_or(props, "showPercentage", false) || get_bool_or(props, "showLabel", false);
        let label = get_string_or(props, "label", "");
        let aria_label = get_string(props, "aria-label").unwrap_or_else(|| {
            if label.is_empty() {
                percent_text.clone()
            } else {
                label.clone()
            }
        });
        let class = merge_class_name("w-full", props);
        if progress_type == "circular" {
            let size = get_string_or(props, "size", "md");
            let (radius, dimensions) = match size.as_str() {
                "sm" => (28.0, "w-16 h-16"),
                "lg" => (56.0, "w-32 h-32"),
                _ => (40.0, "w-24 h-24"),
            };
            let circumference = 2.0 * std::f64::consts::PI * radius;
            let offset = circumference * (1.0 - percentage / 100.0);
            let class = merge_class_name("relative inline-flex items-center justify-center", props);
            return rsx! { div { class: "{class}", role: "progressbar", aria_label: "{aria_label}", aria_valuenow: "{value}", aria_valuemin: "0", aria_valuemax: "{max}", ..crate::registry::shared_root_attrs(props),
                svg { class: "transform -rotate-90 {dimensions}", view_box: "0 0 100 100",
                    circle { cx: "50", cy: "50", r: "{radius}", stroke: "var(--color-muted)", stroke_width: "8", fill: "none" }
                    circle { cx: "50", cy: "50", r: "{radius}", stroke: "{stroke}", stroke_width: "8", fill: "none", stroke_dasharray: "{circumference}", stroke_dashoffset: "{offset}", stroke_linecap: "round" }
                }
                if show_percentage { span { class: "absolute inset-0 flex items-center justify-center text-sm font-bold text-foreground", "{percent_text}" } }
            } };
        }
        if progress_type == "stepped" {
            let steps = get_number_or(props, "steps", 5.0).max(1.0) as usize;
            let progress = percentage * steps as f64 / 100.0;
            let step_widths: Vec<String> = (0..steps)
                .map(|index| format!("{}%", (progress - index as f64).clamp(0.0, 1.0) * 100.0))
                .collect();
            return rsx! { div { class: "{class}", role: "progressbar", aria_label: "{aria_label}", aria_valuenow: "{value}", aria_valuemin: "0", aria_valuemax: "{max}", ..crate::registry::shared_root_attrs(props),
                if !label.is_empty() || show_percentage { div { class: "flex items-center justify-between mb-2",
                    if !label.is_empty() { span { class: "text-sm font-medium text-foreground", "{label}" } }
                    if show_percentage { span { class: "text-sm text-muted-foreground", "{percent_text}" } }
                } }
                div { class: "flex gap-1", for step_width in step_widths.iter() { div { class: "flex-1 h-2 bg-muted border border-border overflow-hidden",
                    div { class: "h-full transition-all duration-normal ease-emphasized {fill}", style: "width:{step_width}" }
                } } }
            } };
        }
        rsx! { div { class: "{class}", role: "progressbar", aria_label: "{aria_label}", aria_valuenow: "{value}", aria_valuemin: "0", aria_valuemax: "{max}", ..crate::registry::shared_root_attrs(props),
            if !label.is_empty() || show_percentage { div { class: "flex items-center justify-between mb-1.5",
                if !label.is_empty() { span { class: "text-sm font-bold text-foreground", "{label}" } }
                if show_percentage { span { class: "text-sm text-foreground font-medium", "{percent_text}" } }
            } }
            div { class: "w-full h-2 bg-muted border border-border overflow-hidden", div { class: "h-full transition-all duration-normal ease-emphasized {fill}", style: "width:{width}" } }
        } }
    }
}
