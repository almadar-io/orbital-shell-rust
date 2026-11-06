use crate::registry::{shared_root_attrs_with_defaults, DioxusPatternRegistry};
use crate::renderer::{cn, emit_empty_event, DioxusPatternRenderer, DioxusView, BOX_ATOM_BASE};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `flip-card` pattern, mirroring the web `FlipCard` over `FlipContainer`.
pub struct FlipCardRenderer;

impl FlipCardRenderer {
    const NAME: &str = "flip-card";
}

const FACE: &str = "absolute inset-0 w-full h-full rounded-container shadow-elevation-dialog flex items-center justify-center p-6";

impl DioxusPatternRenderer for FlipCardRenderer {
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
        let height = get_string_or(props, "height", "h-64");
        let extra = get_string_or(props, "className", "");
        let card_class = cn(&[&height, extra.trim()]);
        let class = cn(&["relative w-full cursor-pointer", &card_class]);
        let face_class = cn(&[BOX_ATOM_BASE, FACE]);
        let flipped = get_bool_or(props, "flipped", false);
        let rotation = if flipped { "180deg" } else { "0deg" };
        let style = format!("transform-style:preserve-3d;transform:rotateY({rotation})");
        let on_flip = get_string(props, "onFlip").filter(|name| !name.is_empty());
        let interactive = on_flip.is_some();
        let defaults = if interactive {
            vec![
                ("role", Value::string("button")),
                ("tabIndex", Value::Int(0)),
            ]
        } else {
            Vec::new()
        };
        let sink = view.sink;
        let key_event = on_flip.clone();
        let fields = props.as_object();
        let front_node = match fields.and_then(|fields| fields.get("front")) {
            Some(node) => view.render_node(registry, node),
            None => rsx! {},
        };
        let back_node = match fields.and_then(|fields| fields.get("back")) {
            Some(node) => view.render_node(registry, node),
            None => rsx! {},
        };
        rsx! {
            div { class: "{class}", style: "perspective:1000px",
                onclick: move |_| emit_empty_event(sink, on_flip.as_deref()),
                onkeydown: move |event| {
                    if !interactive { return; }
                    let key = event.key();
                    if key == Key::Enter || key.to_string() == " " {
                        event.prevent_default();
                        emit_empty_event(sink, key_event.as_deref());
                    }
                },
                ..shared_root_attrs_with_defaults(props, &defaults),
                div { class: "relative w-full h-full transition-transform duration-slow", style: "{style}",
                    div { class: "{face_class}", style: "backface-visibility:hidden;transform:rotateY(0deg)", {front_node} }
                    div { class: "{face_class}", style: "backface-visibility:hidden;transform:rotateY(180deg)", {back_node} }
                }
            }
        }
    }
}
