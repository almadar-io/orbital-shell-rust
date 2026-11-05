//! Native Dioxus body for the `star-rating` pattern, mirroring `StarRating.tsx`.

use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    number_value, object_value, pattern_spec, DioxusPatternRenderer, DioxusView, EventSink,
    OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::{Value, ValueMap};

pub struct StarRatingRenderer;

#[derive(Props, Clone, PartialEq)]
struct StarProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn js_number(number: f64) -> String {
    if number.fract() == 0.0 {
        format!("{}", number as i64)
    } else {
        number.to_string()
    }
}

fn emit_change(sink: Option<EventSink>, action: &Option<String>, payload: &Value, value: f64) {
    let (Some(sink), Some(action)) = (sink, action) else {
        return;
    };
    let mut fields = payload.as_object().cloned().unwrap_or_else(ValueMap::new);
    fields.insert("value".into(), number_value(value));
    sink.call((action.clone(), Value::Object(fields.into())));
}

#[allow(non_snake_case)]
fn Stars(input: StarProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let mut hover = use_signal(|| None::<f64>);
    let value = get_number_or(props, "value", 0.0);
    let max = get_number_or(props, "max", 5.0).max(0.0) as usize;
    let max_value = max as f64;
    let read_only = get_bool_or(props, "readOnly", false);
    let half = get_string_or(props, "precision", "full") == "half";
    let size = get_string_or(props, "size", "md");
    let (star_px, star_size, gap) = match size.as_str() {
        "sm" => (16.0, "sm", "gap-0.5"),
        "lg" => (32.0, "xl", "gap-1.5"),
        _ => (24.0, "lg", "gap-1"),
    };
    let class = merge_class_name(
        &format!(
            "inline-flex items-center {gap} {}",
            if read_only { "" } else { "cursor-pointer" }
        ),
        props,
    );
    let ratio = format!(
        "Rating: {} out of {}",
        js_number(value),
        js_number(max_value)
    );
    let label = get_string(props, "aria-label")
        .or_else(|| get_string(props, "label"))
        .unwrap_or_else(|| ratio.clone());
    let action = get_string(props, "action");
    let payload = props
        .as_object()
        .and_then(|fields| fields.get("actionPayload"))
        .cloned()
        .unwrap_or_else(|| object_value::<&str>([]));
    let sink = view.sink;
    let display = hover().unwrap_or(value);
    let attrs: Vec<Attribute> = crate::registry::shared_root_attrs(props)
        .into_iter()
        .filter(|attr| !matches!(attr.name, "role" | "aria-label" | "tabindex"))
        .collect();
    let key_action = action.clone();
    let key_payload = payload.clone();
    rsx! {
        div {
            class: "{class}",
            role: if read_only { "img" } else { "slider" },
            "aria-label": "{label}",
            "aria-valuemin": if read_only { None } else { Some("0".to_string()) },
            "aria-valuemax": if read_only { None } else { Some(js_number(max_value)) },
            "aria-valuenow": if read_only { None } else { Some(js_number(value)) },
            "aria-valuetext": if read_only { None } else { Some(ratio.clone()) },
            tabindex: if read_only { None } else { Some("0".to_string()) },
            onkeydown: move |event| {
                if read_only { return; }
                let step = if half { 0.5 } else { 1.0 };
                let next = match event.key() {
                    Key::ArrowRight | Key::ArrowUp => Some((value + step).min(max_value)),
                    Key::ArrowLeft | Key::ArrowDown => Some((value - step).max(0.0)),
                    Key::Home => Some(0.0),
                    Key::End => Some(max_value),
                    _ => None,
                };
                if let Some(next) = next {
                    event.prevent_default();
                    emit_change(sink, &key_action, &key_payload, next);
                }
            },
            onmouseleave: move |_| hover.set(None),
            ..attrs,
            for i in 0..max {
                {
                    let fill = (display - i as f64).clamp(0.0, 1.0);
                    let is_full = fill >= 1.0;
                    let is_half = fill >= 0.5 && fill < 1.0;
                    let click_action = action.clone();
                    let click_payload = payload.clone();
                    let half_action = action.clone();
                    let half_payload = payload.clone();
                    let empty_star = pattern_spec("icon", [
                        ("name", Value::string("star")),
                        ("size", Value::string(star_size)),
                        ("color", Value::string("muted")),
                        ("className", Value::string("transition-colors duration-instant")),
                        ("strokeWidth", number_value(1.5)),
                    ]);
                    let mut filled_fields = vec![
                        ("name", Value::string("star")),
                        ("size", Value::string(star_size)),
                        ("color", Value::string("warning")),
                        ("className", Value::string("absolute inset-0 fill-warning transition-colors duration-instant")),
                        ("strokeWidth", number_value(1.5)),
                    ];
                    if is_half {
                        filled_fields.push(("style", object_value([("clipPath", Value::string("inset(0 50% 0 0)"))])));
                    }
                    let filled_star = pattern_spec("icon", filled_fields);
                    rsx! {
                        span {
                            key: "{i}",
                            "aria-hidden": "true",
                            class: "relative inline-block",
                            onclick: move |_| {
                                if read_only { return; }
                                emit_change(sink, &click_action, &click_payload, i as f64 + 1.0);
                            },
                            onmousemove: move |event| {
                                if read_only { return; }
                                let left = event.element_coordinates().x < star_px / 2.0;
                                hover.set(Some(if left && half { i as f64 + 0.5 } else { i as f64 + 1.0 }));
                            },
                            {registry.render_spec(&empty_star, None, &view)}
                            if is_full || is_half { {registry.render_spec(&filled_star, None, &view)} }
                            if !read_only && half {
                                span {
                                    class: "absolute inset-0 w-1/2",
                                    onclick: move |event| {
                                        event.stop_propagation();
                                        emit_change(sink, &half_action, &half_payload, i as f64 + 0.5);
                                    },
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl StarRatingRenderer {
    const NAME: &str = "star-rating";
}

impl DioxusPatternRenderer for StarRatingRenderer {
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
        rsx! { Stars { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
