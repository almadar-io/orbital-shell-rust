//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use crate::styles::style_with_props;
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `center` pattern.
pub struct CenterRenderer;

impl CenterRenderer {
    const NAME: &str = "center";
}

impl DioxusPatternRenderer for CenterRenderer {
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
        let inline = get_bool_or(props, "inline", false);
        let horizontal = get_bool_or(props, "horizontal", true);
        let vertical = get_bool_or(props, "vertical", true);
        let full_height = get_bool_or(props, "fullHeight", false);
        let full_width = get_bool_or(props, "fullWidth", false);
        let min_height = match props.as_object().and_then(|fields| fields.get("minHeight")) {
            Some(Value::String(value)) => value.to_string(),
            Some(value) => value
                .as_number()
                .map(|number| format!("{number}px"))
                .unwrap_or_default(),
            None => String::new(),
        };
        let style = style_with_props(
            &if min_height.is_empty() {
                String::new()
            } else {
                format!("min-height:{min_height}")
            },
            props,
        );
        let base = format!(
            "{} {} {} {} {}",
            if inline { "inline-flex" } else { "flex" },
            if horizontal { "justify-center" } else { "" },
            if vertical { "items-center" } else { "" },
            if full_height { "h-full" } else { "" },
            if full_width { "w-full" } else { "" }
        );
        let class = merge_class_name(&base, props);
        let children = child_patterns(props, view.trait_content);
        rsx! { div { class: "{class}", style: "{style}", ..shared_root_attrs(props), for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} } } }
    }
}
