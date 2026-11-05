use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    object_value, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_bool_or, get_number_or, get_string, get_string_or, };
use orbital_core::Value;

/// Dioxus renderer for the `lightbox` pattern.
pub struct LightboxRenderer;

#[derive(Props, Clone, PartialEq)]
struct LightboxProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn overlay_button(icon: &str, position: &str, label: &str) -> Value {
    pattern_spec("button", [
        ("variant", Value::string("ghost")),
        ("icon", Value::string(icon)),
        ("label", Value::string("")),
        ("className", Value::String(format!("absolute z-10 rounded-full bg-card text-foreground hover:bg-muted {position}").into())),
        ("aria-label", Value::string(label)),
    ])
}

#[allow(non_snake_case)]
fn Lightbox(input: LightboxProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let images: Vec<Value> = props
        .as_object()
        .and_then(|fields| fields.get("images"))
        .and_then(Value::as_array)
        .map(|items| items.iter().cloned().collect())
        .unwrap_or_default();
    let current = get_number_or(props, "currentIndex", 0.0) as usize;
    let mut state = use_signal(|| (current, current));
    let (seen, stored) = state();
    let index = if seen == current { stored } else { current };
    if !get_bool_or(props, "isOpen", false) || images.is_empty() {
        return rsx! {};
    }
    let last = images.len() - 1;
    let index = index.min(last);
    let image = &images[index];
    let has_prev = index > 0;
    let has_next = index < last;
    let show_counter = get_bool_or(props, "showCounter", true);
    let src = get_string(image, "src").unwrap_or_default();
    let alt = get_string_or(image, "alt", "");
    let caption = get_string(image, "caption").filter(|caption| !caption.is_empty());
    let class = merge_class_name(
        "fixed inset-0 z-[1000] flex items-center justify-center bg-black/90",
        props,
    );
    let attrs: Vec<Attribute> = shared_root_attrs(props)
        .into_iter()
        .filter(|attr| attr.name != "role" && attr.name != "aria-label")
        .collect();
    let label = get_string(props, "aria-label").unwrap_or_else(|| {
        if alt.is_empty() {
            "Image viewer".to_string()
        } else {
            alt.clone()
        }
    });
    let close_event = get_string(props, "closeAction");
    let sink = view.sink;
    let key_event = close_event.clone();
    let close_btn_event = close_event.clone();
    let close = move |event: Option<&String>| {
        if let (Some(sink), Some(event)) = (sink, event) {
            sink.call((event.clone(), object_value::<&str>([])));
        }
    };
    let close_button = overlay_button("x", "top-4 end-4", "Close modal");
    let prev_button = overlay_button("chevron-left", "start-4", "Previous image");
    let next_button = overlay_button("chevron-right", "end-4", "Next image");
    let counter = pattern_spec(
        "typography",
        [
            ("variant", Value::string("small")),
            ("className", Value::string("text-white mb-1")),
            (
                "content",
                Value::String(format!("{} / {}", index + 1, images.len()).into()),
            ),
        ],
    );
    let caption_spec = caption.map(|caption| {
        pattern_spec(
            "typography",
            [
                ("variant", Value::string("small")),
                ("className", Value::string("text-white/80 px-8")),
                ("content", Value::String(caption.into())),
            ],
        )
    });
    let close_click = close.clone();
    let close_key = close.clone();

    rsx! {
        div {
            class: "{class}",
            role: "dialog",
            aria_modal: "true",
            "aria-label": "{label}",
            "data-pattern": "lightbox",
            tabindex: "0",
            onclick: move |_| close_click(close_event.as_ref()),
            onkeydown: move |event| match event.key() {
                Key::Escape => close_key(key_event.as_ref()),
                Key::ArrowRight if has_next => state.set((current, index + 1)),
                Key::ArrowLeft if has_prev => state.set((current, index - 1)),
                _ => {}
            },
            ..attrs,
            div { style: "display:contents",
                onclick: move |event| {
                    event.stop_propagation();
                    close(close_btn_event.as_ref());
                },
                {registry.render_spec(&close_button, None, &view)}
            }
            if has_prev {
                div { style: "display:contents",
                    onclick: move |event| {
                        event.stop_propagation();
                        state.set((current, index - 1));
                    },
                    {registry.render_spec(&prev_button, None, &view)}
                }
            }
            div { class: "flex items-center justify-center w-full h-full p-12",
                onclick: move |event| event.stop_propagation(),
                img { src: "{src}", alt: "{alt}", class: "max-w-full max-h-full object-contain select-none", draggable: false }
            }
            if has_next {
                div { style: "display:contents",
                    onclick: move |event| {
                        event.stop_propagation();
                        state.set((current, index + 1));
                    },
                    {registry.render_spec(&next_button, None, &view)}
                }
            }
            div { class: "absolute bottom-4 inset-x-0 text-center",
                if show_counter && images.len() > 1 { {registry.render_spec(&counter, None, &view)} }
                if let Some(spec) = caption_spec.as_ref() { {registry.render_spec(spec, None, &view)} }
            }
        }
    }
}

impl DioxusPatternRenderer for LightboxRenderer {
    fn pattern_name(&self) -> &str {
        "lightbox"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { Lightbox { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
