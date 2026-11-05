//! Native Dioxus renderer for the `split-pane` pattern, mirroring the web `SplitPane` organism.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `split-pane` pattern.
pub struct SplitPaneRenderer;

impl SplitPaneRenderer {
    const NAME: &str = "split-pane";
}

impl DioxusPatternRenderer for SplitPaneRenderer {
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
        let horizontal = get_string_or(props, "direction", "horizontal") != "vertical";
        let ratio = Value::Number(get_number_or(props, "ratio", 50.0)).to_string_value();
        let resizable = get_bool_or(props, "resizable", true);
        let root_class = merge_class_name(
            &cn(&[
                "flex w-full h-full",
                if horizontal { "flex-row" } else { "flex-col" },
            ]),
            props,
        );
        let left_class = cn(&[
            "flex flex-col overflow-auto",
            &get_string_or(props, "leftClassName", ""),
        ]);
        let right_class = cn(&[
            "flex flex-col flex-1 min-h-0 min-w-0 overflow-auto",
            &get_string_or(props, "rightClassName", ""),
        ]);
        let left_style = format!(
            "{}:{ratio}%;flex-shrink:0",
            if horizontal { "width" } else { "height" }
        );
        let handle_class = cn(&[
            "flex-shrink-0 bg-border transition-colors touch-none",
            if horizontal {
                "w-1 cursor-col-resize hover:w-1.5 hover:bg-muted-foreground"
            } else {
                "h-1 cursor-row-resize hover:h-1.5 hover:bg-muted-foreground"
            },
        ]);
        let fields = props.as_object();
        let left = fields.and_then(|fields| fields.get("left"));
        let right = fields.and_then(|fields| fields.get("right"));
        rsx! { div { class: "{root_class}", ..shared_root_attrs(props),
            div { class: "{left_class}", style: "{left_style}",
                if let Some(left) = left { {view.render_node(registry, left)} }
            }
            if resizable { div { class: "{handle_class}" } }
            div { class: "{right_class}",
                if let Some(right) = right { {view.render_node(registry, right)} }
            }
        } }
    }
}
