//! Native Dioxus renderer for the `graph-canvas` pattern: the web's card shell, header, canvas host and legend (the canvas itself is orbital-canvas territory).

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{cn, merge_class_name, pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::hash::{Hash, Hasher};

/// Dioxus renderer for the `graph-canvas` pattern.
pub struct GraphCanvasRenderer;

impl GraphCanvasRenderer {
    const NAME: &str = "graph-canvas";
}

const BOX: &str = "bg-transparent rounded-none shadow-none";
const SERIES_COUNT: usize = 8;

fn typography(variant: &str, color: Option<&str>, content: &str) -> Value {
    let mut fields = vec![
        ("variant", Value::string(variant)),
        ("content", Value::string(content)),
    ];
    if let Some(color) = color {
        fields.push(("color", Value::string(color)));
    }
    pattern_spec("typography", fields)
}

fn icon_button(icon: &str, label: &str) -> Value {
    pattern_spec(
        "button",
        [
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("icon", Value::string(icon)),
            ("aria-label", Value::string(label)),
        ],
    )
}

impl DioxusPatternRenderer for GraphCanvasRenderer {
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
        let class_name = get_string_or(props, "className", "");
        let with_class = |fields: Vec<(&'static str, Value)>| -> Vec<(&'static str, Value)> {
            let mut fields = fields;
            if !class_name.is_empty() {
                fields.push(("className", Value::string(class_name.as_str())));
            }
            fields
        };
        if get_bool_or(props, "isLoading", false) {
            let spec = pattern_spec(
                "loading-state",
                with_class(vec![("message", Value::string("Loading..."))]),
            );
            return registry.render_spec(&spec, None, view);
        }
        if let Some(error) = props
            .get("error")
            .filter(|error| !matches!(error, Value::Null))
        {
            let spec = pattern_spec(
                "error-state",
                with_class(vec![
                    ("title", Value::string("Graph error")),
                    (
                        "message",
                        Value::string(get_string_or(error, "message", "")),
                    ),
                ]),
            );
            return registry.render_spec(&spec, None, view);
        }
        let nodes = get_array(props, "nodes");
        if nodes.is_empty() {
            let spec = pattern_spec(
                "empty-state",
                with_class(vec![
                    ("title", Value::string("No graph data")),
                    ("description", Value::string("No nodes to display.")),
                ]),
            );
            return registry.render_spec(&spec, None, view);
        }

        let title = get_string(props, "title").filter(|title| !title.is_empty());
        let actions = get_array(props, "actions");
        let interactive = get_bool_or(props, "interactive", true);
        let height = get_number_or(props, "height", 400.0);
        let description = get_string(props, "description").filter(|text| !text.is_empty());
        let selected_label = get_string(props, "selectedNodeId").and_then(|id| {
            nodes
                .iter()
                .find(|node| get_string(node, "id").as_deref() == Some(id.as_str()))
                .map(|node| {
                    get_string(node, "label")
                        .filter(|label| !label.is_empty())
                        .unwrap_or(id)
                })
        });
        let mut groups: Vec<String> = Vec::new();
        for node in nodes.iter() {
            if let Some(group) = get_string(node, "group").filter(|group| !group.is_empty()) {
                if !groups.contains(&group) {
                    groups.push(group);
                }
            }
        }

        let card_class = merge_class_name(
            "rounded-container relative transition-all duration-normal chrome-panel surface-content p-card-md overflow-hidden",
            props,
        );
        let show_header = title.is_some() || !actions.is_empty() || interactive;
        let header_class = cn(&[
            "flex flex-row gap-2 items-center justify-between",
            "px-4 py-2 border-b border-border",
        ]);
        let canvas_host_class = cn(&[BOX, "w-full bg-background"]);
        let label = title.clone().unwrap_or_else(|| "Graph".to_string());
        let canvas_style = format!("height:{height}px");
        let desc_id = description.as_ref().map(|text| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut hasher);
            format!("gc-desc-{:x}", hasher.finish())
        });
        let title_node = title
            .as_ref()
            .map(|title| registry.render_spec(&typography("h6", None, title), None, view));
        let zoom_out = registry.render_spec(&icon_button("zoom-out", "Zoom out"), None, view);
        let zoom_in = registry.render_spec(&icon_button("zoom-in", "Zoom in"), None, view);
        let reset = registry.render_spec(&icon_button("rotate-ccw", "Reset"), None, view);
        let sink = view.sink;
        let action_nodes: Vec<(Element, Option<String>)> = actions
            .iter()
            .map(|action| {
                let spec = pattern_spec(
                    "badge",
                    [
                        ("variant", Value::string("default")),
                        (
                            "className",
                            Value::string("cursor-pointer hover:opacity-80 transition-opacity"),
                        ),
                        ("label", Value::string(get_string_or(action, "label", ""))),
                    ],
                );
                (
                    registry.render_spec(&spec, None, view),
                    get_string(action, "event"),
                )
            })
            .collect();
        let legend: Vec<(String, Element)> = groups
            .iter()
            .enumerate()
            .map(|(index, group)| {
                (
                    format!(
                        "background-color:var(--color-series-{})",
                        index % SERIES_COUNT + 1
                    ),
                    registry.render_spec(
                        &typography("caption", Some("secondary"), group),
                        None,
                        view,
                    ),
                )
            })
            .collect();

        rsx! {
            div { class: "{card_class}", style: "height:auto",
                div { class: "flex flex-col gap-0 items-stretch justify-start",
                    if show_header {
                        div { class: "{header_class}",
                            if let Some(node) = title_node { {node} }
                            div { class: "flex flex-row gap-1 items-center justify-start",
                                if interactive {
                                    {zoom_out}
                                    {zoom_in}
                                    {reset}
                                }
                                for (index, (badge, event)) in action_nodes.into_iter().enumerate() {
                                    span {
                                        key: "{index}",
                                        class: "contents",
                                        onclick: move |_| {
                                            if let (Some(sink), Some(event)) = (sink, &event) {
                                                sink.call((event.clone(), Value::Object(orbital_core::ValueMap::new().into())));
                                            }
                                        },
                                        {badge}
                                    }
                                }
                            }
                        }
                    }
                    div { class: "{canvas_host_class}",
                        canvas {
                            class: "w-full cursor-grab active:cursor-grabbing touch-none",
                            style: "{canvas_style}",
                            role: "img",
                            "aria-label": "{label}",
                            "aria-describedby": desc_id.clone(),
                        }
                        if let (Some(id), Some(text)) = (desc_id.clone(), description.clone()) {
                            div { id: "{id}", class: "{BOX} sr-only", "{text}" }
                        }
                        if let Some(label) = selected_label {
                            div { class: "{BOX} sr-only", role: "status", "Selected: {label}" }
                        }
                    }
                    if groups.len() > 1 {
                        div { class: "flex flex-row gap-4 items-stretch justify-start flex-wrap px-4 py-2 border-t border-border",
                            for (index, (swatch, caption)) in legend.into_iter().enumerate() {
                                div { key: "{index}", class: "flex flex-row gap-1 items-center justify-start",
                                    div { class: "{BOX} w-3 h-3 rounded-full flex-shrink-0", style: "{swatch}" }
                                    {caption}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
