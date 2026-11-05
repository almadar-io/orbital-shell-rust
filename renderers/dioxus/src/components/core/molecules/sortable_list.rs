use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_string_or, render_item_param_body, substitute_row_bindings,
};
use orbital_core::Value;

const BOX: &str = "bg-transparent rounded-none shadow-none";

pub struct SortableListRenderer;

fn items(props: &Value) -> Vec<Value> {
    match props.get("items") {
        Some(Value::Array(items)) => items.iter().cloned().collect(),
        Some(item @ Value::Object(_)) => vec![item.clone()],
        _ => Vec::new(),
    }
}

fn item_json(item: &Value) -> String {
    serde_json::to_string(item).unwrap_or_else(|error| {
        tracing::error!(%error, "sortable-list item");
        String::new()
    })
}

impl DioxusPatternRenderer for SortableListRenderer {
    fn pattern_name(&self) -> &str {
        "sortable-list"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let root = cn(&[
            "flex flex-col gap-0 items-stretch justify-start",
            &cn(&["w-full", get_string_or(props, "className", "").as_str()]),
        ]);
        let row_class = cn(&[
            "flex flex-row gap-2 items-center justify-start",
            "transition-opacity duration-fast",
        ]);
        let handle_class = cn(&[
            BOX,
            "flex items-center justify-center text-muted-foreground hover:text-foreground transition-colors duration-instant px-1",
        ]);
        let content_class = cn(&[BOX, "flex-1 min-w-0"]);
        let handle_left = get_string_or(props, "dragHandlePosition", "left") == "left";
        let lambda = props.get("renderItem").and_then(render_item_param_body);
        let grip = pattern_spec(
            "icon",
            [
                ("name", Value::string("grip-vertical")),
                ("size", Value::string("sm")),
            ],
        );
        let rows = items(props);
        rsx! {
            div { class: "{root}", ..shared_root_attrs(props),
                for (index, item) in rows.iter().enumerate() {
                    div { key: "{index}", class: "{BOX}", style: "opacity:1",
                        div { class: "{row_class}",
                            if handle_left {
                                div { class: "{handle_class}", style: "cursor:grab;touch-action:none", role: "button", "aria-grabbed": "false",
                                    {registry.render_spec(&grip, None, view)}
                                }
                            }
                            div { class: "{content_class}",
                                if let Some((param, body)) = lambda.as_ref() {
                                    {registry.render_spec(&substitute_row_bindings(body, param, item), None, view)}
                                } else {
                                    {item_json(item)}
                                }
                            }
                            if !handle_left {
                                div { class: "{handle_class}", style: "cursor:grab;touch-action:none", role: "button", "aria-grabbed": "false",
                                    {registry.render_spec(&grip, None, view)}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
