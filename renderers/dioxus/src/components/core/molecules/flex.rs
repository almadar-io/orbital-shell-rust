//! Native Dioxus renderer for the `flex` pattern, mirroring the web `Flex` molecule.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use crate::styles::style_with_props;
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// Dioxus renderer for the `flex` pattern.
pub struct FlexRenderer;

impl FlexRenderer {
    const NAME: &str = "flex";
}

fn mapped(props: &Value, key: &str, default: &str, table: &[(&str, &str)]) -> String {
    let value = get_string_or(props, key, default);
    table
        .iter()
        .find(|(name, _)| *name == value)
        .map(|(_, class)| (*class).to_string())
        .unwrap_or_default()
}

fn flex_number(props: &Value, key: &str) -> Option<String> {
    let value = props.as_object()?.get(key)?;
    match value {
        Value::Bool(true) => Some("1".to_string()),
        Value::Bool(false) => Some("0".to_string()),
        other if other.is_numeric() => Some(other.to_string_value()),
        _ => None,
    }
}

impl DioxusPatternRenderer for FlexRenderer {
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
        let direction = mapped(
            props,
            "direction",
            "row",
            &[
                ("row", "flex-row"),
                ("row-reverse", "flex-row-reverse"),
                ("col", "flex-col"),
                ("col-reverse", "flex-col-reverse"),
            ],
        );
        let wrap = mapped(
            props,
            "wrap",
            "nowrap",
            &[
                ("nowrap", "flex-nowrap"),
                ("wrap", "flex-wrap"),
                ("wrap-reverse", "flex-wrap-reverse"),
            ],
        );
        let align = mapped(
            props,
            "align",
            "stretch",
            &[
                ("start", "items-start"),
                ("center", "items-center"),
                ("end", "items-end"),
                ("stretch", "items-stretch"),
                ("baseline", "items-baseline"),
            ],
        );
        let justify = mapped(
            props,
            "justify",
            "start",
            &[
                ("start", "justify-start"),
                ("center", "justify-center"),
                ("end", "justify-end"),
                ("between", "justify-between"),
                ("around", "justify-around"),
                ("evenly", "justify-evenly"),
            ],
        );
        let gap = mapped(
            props,
            "gap",
            "none",
            &[
                ("none", "gap-0"),
                ("xs", "gap-1"),
                ("sm", "gap-2"),
                ("md", "gap-4"),
                ("lg", "gap-6"),
                ("xl", "gap-8"),
                ("2xl", "gap-12"),
            ],
        );
        let display = if get_bool_or(props, "inline", false) {
            "inline-flex"
        } else {
            "flex"
        };
        let base = [display, &direction, &wrap, &align, &justify, &gap]
            .iter()
            .filter(|part| !part.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        let class = merge_class_name(&base, props);

        let grow = flex_number(props, "grow");
        let shrink = flex_number(props, "shrink");
        let basis = props.as_object().and_then(|fields| fields.get("basis"));
        let mut flex_style = Vec::new();
        if grow.is_some() || shrink.is_some() || basis.is_some() {
            if let Some(grow) = grow {
                flex_style.push(format!("flex-grow:{grow}"));
            }
            if let Some(shrink) = shrink {
                flex_style.push(format!("flex-shrink:{shrink}"));
            }
            match basis {
                Some(value) if value.is_numeric() => {
                    flex_style.push(format!("flex-basis:{}px", value.to_string_value()));
                }
                Some(value) => {
                    if let Some(text) = value.as_str() {
                        flex_style.push(format!("flex-basis:{text}"));
                    }
                }
                None => {}
            }
        }
        let style = style_with_props(&flex_style.join(";"), props);
        let children = child_patterns(props, view.trait_content);
        rsx! { div { class: "{class}", style: "{style}", ..shared_root_attrs(props),
            for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
        } }
    }
}
