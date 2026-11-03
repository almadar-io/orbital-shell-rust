use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_bool_or, get_string, get_string_or, icon_name_to_glyph, merge_class_name,
};
use orbital_core::Value;

pub struct ControlButtonRenderer;

#[derive(Props, Clone, PartialEq)]
struct ControlButtonProps {
    class: String,
    label: String,
    content: Element,
    style: String,
    pressed: bool,
    disabled: bool,
    press_event: Option<String>,
    release_event: Option<String>,
    on_press: Option<String>,
    on_release: Option<String>,
    sink: Option<EventSink>,
}

#[allow(non_snake_case)]
fn ControlButton(props: ControlButtonProps) -> Element {
    let mut held = use_signal(|| false);
    let mut pointer_cycle = use_signal(|| false);
    let class = if props.pressed || held() {
        format!("{} almadar-control-button--pressed", props.class)
    } else {
        props.class.clone()
    };
    let press = props.press_event.clone();
    let release = props.release_event.clone();
    let on_press = props.on_press.clone();
    let on_release = props.on_release.clone();
    let press_on_click = props.press_event.clone();
    let release_on_click = props.release_event.clone();
    let on_press_on_click = props.on_press.clone();
    let on_release_on_click = props.on_release.clone();
    let release_on_leave = props.release_event.clone();
    let on_release_on_leave = props.on_release.clone();
    let sink = props.sink;
    let disabled = props.disabled;
    rsx! {
        button {
            class: "{class}",
            style: "{props.style}",
            disabled: disabled,
            "aria-label": "{props.label}",
            "aria-pressed": props.pressed || held(),
            onmousedown: move |_| {
                if !disabled && !held() {
                    held.set(true);
                    pointer_cycle.set(true);
                    if let (Some(sink), Some(event)) = (sink, &press) {
                        sink.call((event.clone(), Value::Null));
                    }
                    if let (Some(sink), Some(event)) = (sink, &on_press) {
                        sink.call((event.clone(), Value::Null));
                    }
                }
            },
            onmouseup: move |_| {
                if held() {
                    held.set(false);
                    if let (Some(sink), Some(event)) = (sink, &release) {
                        sink.call((event.clone(), Value::Null));
                    }
                    if let (Some(sink), Some(event)) = (sink, &on_release) {
                        sink.call((event.clone(), Value::Null));
                    }
                }
            },
            onmouseleave: move |_| {
                if held() {
                    held.set(false);
                    if let (Some(sink), Some(event)) = (sink, &release_on_leave) {
                        sink.call((event.clone(), Value::Null));
                    }
                    if let (Some(sink), Some(event)) = (sink, &on_release_on_leave) {
                        sink.call((event.clone(), Value::Null));
                    }
                }
            },
            onclick: move |_| {
                if pointer_cycle() {
                    pointer_cycle.set(false);
                } else if !disabled {
                    if let Some(sink) = sink {
                        if let Some(event) = &press_on_click {
                            sink.call((event.clone(), Value::Null));
                        }
                        if let Some(event) = &on_press_on_click {
                            sink.call((event.clone(), Value::Null));
                        }
                        if let Some(event) = &release_on_click {
                            sink.call((event.clone(), Value::Null));
                        }
                        if let Some(event) = &on_release_on_click {
                            sink.call((event.clone(), Value::Null));
                        }
                    }
                }
            },
            {props.content}
        }
    }
}

impl DioxusPatternRenderer for ControlButtonRenderer {
    fn pattern_name(&self) -> &str {
        "control-button"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let size = match get_string_or(props, "size", "md").as_str() {
            "sm" => 40,
            "lg" => 72,
            "xl" => 96,
            _ => 56,
        };
        let radius = match get_string_or(props, "shape", "circle").as_str() {
            "circle" => "50%",
            "square" => "0",
            _ => "8px",
        };
        let variant = match get_string_or(props, "variant", "secondary").as_str() {
            "primary" => "almadar-control-button--primary",
            "ghost" => "almadar-control-button--ghost",
            _ => "almadar-control-button--secondary",
        };
        let style =
            format!("width:{size}px;height:{size}px;border-radius:{radius};touch-action:none");
        let asset = props
            .as_object()
            .and_then(|value| value.get("assetUrl"))
            .cloned();
        let glyph = get_string(props, "icon").map(|name| icon_name_to_glyph(&name).to_string());
        let label = get_string_or(props, "label", "");
        let content = if let Some(asset) = asset {
            let mut spec = orbital_core::ValueMap::new();
            spec.insert("type".into(), Value::String("atlas-image".into()));
            spec.insert("asset".into(), asset);
            spec.insert("size".into(), Value::Number(24.0));
            registry.render_spec(&Value::Object(spec.into()), entity, view)
        } else if let Some(glyph) = glyph {
            rsx! { span { "{glyph}" } }
        } else {
            rsx! { span { "{label}" } }
        };
        rsx! {
            ControlButton {
                class: format!("{} {variant}", merge_class_name("almadar-button almadar-control-button", props)),
                label,
                content,
                style,
                pressed: get_bool_or(props, "pressed", false),
                disabled: get_bool_or(props, "disabled", false),
                press_event: get_string(props, "pressEvent"),
                release_event: get_string(props, "releaseEvent"),
                on_press: get_string(props, "onPress"),
                on_release: get_string(props, "onRelease"),
                sink: view.sink,
            }
        }
    }
}
