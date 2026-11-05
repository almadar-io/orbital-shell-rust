//! Native Dioxus renderer for the shared pattern contract.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `overlay` pattern.
pub struct OverlayRenderer;

impl OverlayRenderer {
    const NAME: &str = "overlay";
}

impl DioxusPatternRenderer for OverlayRenderer {
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
        if !get_bool_or(props, "isVisible", true) {
            return rsx! {};
        }
        let blur = get_bool_or(props, "blur", false);
        let base = if blur {
            "fixed inset-0 z-40 bg-scrim backdrop-blur-sm"
        } else {
            "fixed inset-0 z-40 bg-scrim"
        };
        let class = merge_class_name(base, props);
        let action = get_string(props, "action");
        let sink = view.sink;
        let children = child_patterns(props, view.trait_content);
        rsx! { div { class: "{class}", aria_hidden: "true",
            onclick: move |_| {
                if let (Some(action), Some(sink)) = (&action, sink) {
                    sink.call((action.clone(), Value::Object(orbital_core::ValueMap::new().into())));
                }
            },
            ..shared_root_attrs(props),
            for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
        } }
    }
}
