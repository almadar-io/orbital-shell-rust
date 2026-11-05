//! Native Dioxus renderer for the `accordion` pattern, the web `Accordion` molecule.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    cn, object_value, passthrough_attrs, pattern_spec, DioxusPatternRenderer, DioxusView,
    OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::trait_ref_name;
use orbital_core::Value;

/// Dioxus renderer for the `accordion` pattern.
pub struct AccordionRenderer;

impl AccordionRenderer {
    const NAME: &str = "accordion";
}

impl DioxusPatternRenderer for AccordionRenderer {
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
        rsx! { AccordionBody { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}

#[derive(Props, Clone, PartialEq)]
struct AccordionBodyProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn header_of(item: &Value) -> Option<&Value> {
    item.get("header").or_else(|| item.get("title"))
}

fn dashed(text: &str) -> String {
    let mut out = String::new();
    let mut in_space = false;
    for ch in text.to_lowercase().chars() {
        if ch.is_whitespace() {
            if !in_space {
                out.push('-');
            }
            in_space = true;
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    out
}

fn item_id(item: &Value, index: usize) -> String {
    if let Some(id) = item
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
    {
        return id.to_string();
    }
    match header_of(item).and_then(Value::as_str) {
        Some(text) => format!("accordion-{}-{index}", dashed(text)),
        None => format!("accordion-item-{index}"),
    }
}

fn string_list(value: Option<&Value>) -> Option<Vec<String>> {
    value.and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect()
    })
}

#[allow(non_snake_case)]
fn AccordionBody(input: AccordionBodyProps) -> Element {
    let props = &input.props;
    let view = input.view.as_view();
    let registry = &input.registry;
    let items = get_array(props, "items");
    let ids: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(index, item)| item_id(item, index))
        .collect();
    let default_open: Vec<String> = if let Some(list) = string_list(props.get("defaultOpenItems")) {
        list
    } else if let Some(indices) = props.get("defaultOpen").and_then(Value::as_array) {
        indices
            .iter()
            .filter_map(Value::as_int)
            .filter(|index| *index >= 0 && (*index as usize) < ids.len())
            .map(|index| ids[index as usize].clone())
            .collect()
    } else {
        items
            .iter()
            .zip(ids.iter())
            .filter(|(item, _)| {
                item.get("defaultOpen")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .map(|(_, id)| id.clone())
            .collect()
    };
    let mut internal = use_signal(|| default_open);
    let controlled = string_list(props.get("openItems"));
    let is_controlled = controlled.is_some();
    let current: Vec<String> = controlled.unwrap_or_else(|| internal.read().clone());
    let multiple = get_bool_or(props, "multiple", false);
    let toggle_event = get_string(props, "toggleEvent").filter(|name| !name.is_empty());
    let sink = view.sink;
    let surface = if get_string_or(props, "surface", "auto") == "auto" {
        "surface-content overflow-hidden"
    } else {
        ""
    };
    let root_class = cn(&[
        "w-full divide-y divide-border",
        surface,
        &get_string_or(props, "className", ""),
    ]);
    let attrs = passthrough_attrs(props, &[]);
    let mut rows: Vec<Element> = Vec::new();
    for (item, id) in items.iter().zip(ids.iter()) {
        let is_open = current.contains(id);
        let disabled = item
            .get("disabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let button_class = cn(&[
            "w-full flex items-center justify-between px-4 py-3 hover:bg-muted transition-colors duration-fast focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset disabled:opacity-50 disabled:cursor-not-allowed",
            if is_open { "bg-muted font-bold" } else { "" },
        ]);
        let chevron = pattern_spec(
            "icon",
            [
                ("name", Value::string("chevron-down")),
                ("size", Value::string("sm")),
                (
                    "className",
                    Value::string(cn(&[
                        "transition-transform duration-fast",
                        if is_open { "transform rotate-180" } else { "" },
                    ])),
                ),
            ],
        );
        let header = match header_of(item) {
            Some(Value::String(text)) if trait_ref_name(text).is_none() => registry.render_spec(
                &pattern_spec(
                    "typography",
                    [
                        ("variant", Value::string("body")),
                        ("weight", Value::string("medium")),
                        ("content", Value::string(text.to_string())),
                    ],
                ),
                None,
                &view,
            ),
            Some(node) => view.render_node(registry, node),
            None => rsx! {},
        };
        let body = item
            .get("content")
            .map(|node| view.render_node(registry, node));
        let content_id = format!("accordion-content-{id}");
        let toggled_id = id.clone();
        let toggled_current = current.clone();
        let event = toggle_event.clone();
        let toggle = move |_| {
            if disabled {
                return;
            }
            let mut next = toggled_current.clone();
            if is_open {
                next.retain(|open| open != &toggled_id);
            } else {
                if !multiple {
                    next.clear();
                }
                next.push(toggled_id.clone());
            }
            if !is_controlled {
                internal.set(next);
            }
            if let (Some(sink), Some(event)) = (sink, &event) {
                sink.call((
                    event.clone(),
                    object_value([
                        ("itemId", Value::string(toggled_id.clone())),
                        ("isOpen", Value::Bool(!is_open)),
                    ]),
                ));
            }
        };
        rows.push(rsx! {
            div { key: "{id}",
                div { class: "overflow-hidden",
                    button {
                        r#type: "button",
                        class: "{button_class}",
                        disabled: disabled,
                        aria_expanded: is_open,
                        aria_controls: "{content_id}",
                        onclick: toggle,
                        div { class: "flex-1 text-left", {header} }
                        {registry.render_spec(&chevron, None, &view)}
                    }
                    if is_open {
                        div { id: "{content_id}", class: "px-4 py-3 border-t-heavy border-border",
                            if let Some(body) = body { {body} }
                        }
                    }
                }
            }
        });
    }
    rsx! {
        div { class: "{root_class}", ..attrs,
            for row in rows { {row} }
        }
    }
}
