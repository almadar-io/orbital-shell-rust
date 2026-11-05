//! Native Dioxus renderer for the `split` pattern, mirroring the web `Split` molecule.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `split` pattern.
pub struct SplitRenderer;

impl SplitRenderer {
    const NAME: &str = "split";
}

fn ratio_widths(ratio: &str) -> (&'static str, &'static str) {
    match ratio {
        "1:2" => ("w-1/3", "w-2/3"),
        "2:1" => ("w-2/3", "w-1/3"),
        "1:3" => ("w-1/4", "w-3/4"),
        "3:1" => ("w-3/4", "w-1/4"),
        "1:4" => ("w-1/5", "w-4/5"),
        "4:1" => ("w-4/5", "w-1/5"),
        "2:3" => ("w-2/5", "w-3/5"),
        "3:2" => ("w-3/5", "w-2/5"),
        "1:1" => ("w-1/2", "w-1/2"),
        _ => ("", ""),
    }
}

impl DioxusPatternRenderer for SplitRenderer {
    fn pattern_name(&self) -> &str {
        "split"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let (left_ratio, right_ratio) = ratio_widths(&get_string_or(props, "ratio", "1:1"));
        let gap = match get_string_or(props, "gap", "md").as_str() {
            "none" => "gap-0",
            "sm" => "gap-2",
            "md" => "gap-4",
            "lg" => "gap-6",
            "xl" => "gap-8",
            _ => "",
        };
        let align = match get_string_or(props, "align", "stretch").as_str() {
            "start" => "items-start",
            "center" => "items-center",
            "end" => "items-end",
            "stretch" => "items-stretch",
            _ => "",
        };
        let reverse = get_bool_or(props, "reverse", false);
        let stack = get_bool_or(props, "stackOnMobile", true);
        let bp = match get_string_or(props, "stackBreakpoint", "md").as_str() {
            "sm" => "@sm:",
            "lg" => "@lg:",
            "xl" => "@xl:",
            _ => "@md:",
        };
        let class_name = get_string_or(props, "className", "");
        let left_class = get_string_or(props, "leftClassName", "");
        let right_class = get_string_or(props, "rightClassName", "");
        let children = child_patterns(props, view.trait_content);
        let left = children.first();
        let right = children.get(1);

        let (left_width, right_width, direction, reverse_class) = if stack {
            (
                format!("w-full {bp}{left_ratio}"),
                format!("w-full {bp}{right_ratio}"),
                format!("flex-col {bp}flex-row"),
                if reverse {
                    format!("{bp}flex-row-reverse")
                } else {
                    String::new()
                },
            )
        } else {
            (
                left_ratio.to_string(),
                right_ratio.to_string(),
                "flex-row".to_string(),
                if reverse {
                    "flex-row-reverse".to_string()
                } else {
                    String::new()
                },
            )
        };
        let box_base = "bg-transparent rounded-none shadow-none";
        let left_cls = cn(&[box_base, &left_width, &left_class]);
        let right_cls = cn(&[box_base, &right_width, &right_class]);
        let row_cls = cn(&[
            box_base,
            "flex",
            &direction,
            &reverse_class,
            gap,
            align,
            if stack { "" } else { &class_name },
        ]);
        let outer_cls = cn(&[box_base, "@container w-full", &class_name]);
        let panes = rsx! {
            div { class: "{left_cls}",
                if let Some((child, src)) = left { {view.render_child(registry, child, src.as_deref())} }
            }
            div { class: "{right_cls}",
                if let Some((child, src)) = right { {view.render_child(registry, child, src.as_deref())} }
            }
        };
        if stack {
            rsx! { div { class: "{outer_cls}", ..shared_root_attrs(props),
                div { class: "{row_cls}", {panes} }
            } }
        } else {
            rsx! { div { class: "{row_cls}", ..shared_root_attrs(props), {panes} } }
        }
    }
}
