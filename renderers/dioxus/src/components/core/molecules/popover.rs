use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_bool_or, get_string_or, };
use orbital_core::client::recurse::{child_patterns, trait_ref_name};
use orbital_core::Value;

/// Dioxus renderer for the `popover` pattern.
pub struct PopoverRenderer;

#[derive(Props, Clone, PartialEq)]
struct PopoverProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn Popover(input: PopoverProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let position = get_string_or(props, "position", "bottom");
    let trigger = get_string_or(props, "trigger", "click");
    let show_arrow = get_bool_or(props, "showArrow", true);
    let controlled = props
        .as_object()
        .and_then(|fields| fields.get("open"))
        .and_then(Value::as_bool);
    let mut local_open = use_signal(|| false);
    let is_open = controlled.unwrap_or_else(|| local_open());
    let click = trigger == "click";
    let content = props.as_object().and_then(|fields| fields.get("content"));
    let content_text = content
        .and_then(Value::as_str)
        .filter(|text| trait_ref_name(text).is_none())
        .map(str::to_string);
    let children = child_patterns(props, view.trait_content);

    let side_class = match position.as_str() {
        "top" => "bottom-full left-1/2 -translate-x-1/2 mb-2",
        "left" => "right-full top-1/2 -translate-y-1/2 me-2",
        "right" => "left-full top-1/2 -translate-y-1/2 ms-2",
        _ => "top-full left-1/2 -translate-x-1/2 mt-2",
    };
    let arrow_side = match position.as_str() {
        "top" => "top-full left-1/2 -translate-x-1/2 border-t-white border-l-transparent border-r-transparent border-b-transparent",
        "left" => "left-full top-1/2 -translate-y-1/2 border-l-white border-t-transparent border-b-transparent border-r-transparent",
        "right" => "right-full top-1/2 -translate-y-1/2 border-r-white border-t-transparent border-b-transparent border-l-transparent",
        _ => "bottom-full left-1/2 -translate-x-1/2 border-b-white border-l-transparent border-r-transparent border-t-transparent",
    };
    let panel_class = merge_class_name(
        &format!("absolute z-50 p-4 bg-card surface-material rounded-container border-heavy border-border shadow-elevation-popover {side_class}"),
        props,
    );
    let arrow_class = format!("absolute w-0 h-0 border-heavy {arrow_side}");
    let mut panel_attrs: Vec<Attribute> = shared_root_attrs(props)
        .into_iter()
        .filter(|attr| attr.name != "role" && attr.name != "aria-label")
        .collect();
    let label = props
        .as_object()
        .and_then(|fields| fields.get("aria-label"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| content_text.clone());
    if let Some(label) = label {
        panel_attrs.push(Attribute::new("aria-label", label, None, false));
    }
    let content_node = match (content_text, content) {
        (Some(text), _) => view.render_node(
            &input.registry,
            &pattern_spec(
                "typography",
                [
                    ("variant", Value::string("body")),
                    ("content", Value::String(text.into())),
                ],
            ),
        ),
        (None, Some(node)) => view.render_node(&input.registry, node),
        (None, None) => rsx! {},
    };

    rsx! {
        span { class: "relative inline-flex",
            if click {
                span {
                    role: "button",
                    tabindex: "0",
                    "aria-haspopup": "dialog",
                    "aria-expanded": "{is_open}",
                    onclick: move |_| {
                        if controlled.is_none() {
                            local_open.set(!local_open());
                        }
                    },
                    onkeydown: move |event| {
                        if controlled.is_none()
                            && (event.key() == Key::Enter || event.key() == Key::Character(" ".to_string()))
                        {
                            event.prevent_default();
                            local_open.set(!local_open());
                        }
                    },
                    for (child, source) in children.iter() {
                        {view.render_child(&input.registry, child, source.as_deref())}
                    }
                }
            } else {
                span {
                    "data-popover-open": "{is_open}",
                    onmouseenter: move |_| {
                        if controlled.is_none() { local_open.set(true); }
                    },
                    onmouseleave: move |_| {
                        if controlled.is_none() { local_open.set(false); }
                    },
                    onfocusin: move |_| {
                        if controlled.is_none() { local_open.set(true); }
                    },
                    onfocusout: move |_| {
                        if controlled.is_none() { local_open.set(false); }
                    },
                    for (child, source) in children.iter() {
                        {view.render_child(&input.registry, child, source.as_deref())}
                    }
                }
            }
            if is_open {
                div {
                    class: "{panel_class}",
                    style: "width:max-content;max-width:calc(100vw - 16px)",
                    role: "dialog",
                    onkeydown: move |event| {
                        if event.key() == Key::Escape && controlled.is_none() {
                            local_open.set(false);
                        }
                    },
                    ..panel_attrs,
                    {content_node}
                    if show_arrow { div { class: "{arrow_class}" } }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for PopoverRenderer {
    fn pattern_name(&self) -> &str {
        "popover"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { Popover { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
