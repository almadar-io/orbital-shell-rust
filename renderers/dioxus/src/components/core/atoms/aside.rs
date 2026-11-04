//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `aside` pattern.
pub struct AsideRenderer;

impl AsideRenderer {
    const NAME: &str = "aside";
}

impl DioxusPatternRenderer for AsideRenderer {
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
        let class = merge_class_name("", props);
        let children = child_patterns(props, view.trait_content);
        rsx! { aside { class: "{class}", ..shared_root_attrs(props), for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} } } }
    }
}
