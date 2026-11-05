//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `section-header` pattern.
pub struct SectionHeaderRenderer;

impl SectionHeaderRenderer {
    const NAME: &str = "section-header";
}

impl DioxusPatternRenderer for SectionHeaderRenderer {
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
        let align = get_string_or(props, "align", "center");
        let level = get_number_or(props, "level", 2.0) as i64;
        let align_class = match align.as_str() {
            "left" => "items-start text-left",
            "right" => "items-end text-right",
            _ => "items-center text-center",
        };
        let heading_class = match level {
            1 => "text-4xl",
            3 => "text-2xl",
            _ => "text-3xl",
        };
        let base = format!("flex flex-col gap-2 {align_class}");
        let class = merge_class_name(&base, props);
        let title = get_string_or(props, "title", "");
        let subtitle = get_string(props, "subtitle");
        rsx! { div { class: "{class}", ..crate::registry::shared_root_attrs(props), div { class: "{heading_class} heading-voice text-foreground font-bold", "{title}" } if let Some(sub) = subtitle { div { class: "text-base font-normal text-muted-foreground", "{sub}" } } } }
    }
}
