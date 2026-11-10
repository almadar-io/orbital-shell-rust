//! Dioxus renderer for the `annotated-media` pattern, mirroring `AnnotatedMedia.tsx`.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    cn, number_value, object_value, DioxusPatternRenderer, DioxusView, EventSink, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct AnnotatedMediaRenderer;

impl AnnotatedMediaRenderer {
    const NAME: &str = "annotated-media";
}

static NEXT_VIDEO: AtomicUsize = AtomicUsize::new(0);

#[derive(Props, Clone, PartialEq)]
struct AnnotatedMediaProps {
    props: Value,
    view: OwnedDioxusView,
}

#[derive(Clone, PartialEq)]
struct PinSpec {
    id: String,
    x: Option<f64>,
    y: Option<f64>,
    time: Option<f64>,
    label: Option<String>,
    resolved: bool,
    number: usize,
}

fn round1(n: f64) -> f64 {
    (n * 10.0).round() / 10.0
}

fn clamp(n: f64) -> f64 {
    n.clamp(0.0, 100.0)
}

fn pin_class(pin: &PinSpec, active: Option<&str>) -> String {
    let tone = if pin.resolved {
        "bg-muted text-muted-foreground"
    } else {
        "bg-primary text-primary-foreground"
    };
    let ring = if active == Some(pin.id.as_str()) {
        "ring-2 ring-ring ring-offset-2 ring-offset-background"
    } else {
        ""
    };
    cn(&[
        "flex h-7 w-7 items-center justify-center rounded-full border-2 border-background text-xs font-semibold shadow-md focus:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        tone,
        ring,
    ])
}

async fn video_number(script: String) -> Option<f64> {
    let mut eval = document::eval(&script);
    match eval.recv::<f64>().await {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::error!(%error, "annotated-media video read");
            None
        }
    }
}

fn emit(sink: Option<EventSink>, name: &Option<String>, payload: Value) {
    if let (Some(sink), Some(name)) = (sink, name) {
        sink.call((name.clone(), payload));
    }
}

#[allow(non_snake_case)]
fn AnnotatedMedia(input: AnnotatedMediaProps) -> Element {
    let props = &input.props;
    let view = input.view.as_view();
    let sink = view.sink;
    let src = get_string_or(props, "src", "");
    let video = get_string_or(props, "mediaType", "image") == "video";
    let title = get_string_or(props, "title", "");
    let active = get_string(props, "activePinId");
    let placing = get_bool_or(props, "placing", true);
    let place_event = get_string(props, "placeEvent");
    let select_event = get_string(props, "pinSelectEvent");
    let class_name = get_string_or(props, "className", "");
    let pins: Vec<PinSpec> = get_array(props, "pins")
        .iter()
        .enumerate()
        .map(|(index, pin)| PinSpec {
            id: get_string_or(pin, "id", ""),
            x: get_number(pin, "x"),
            y: get_number(pin, "y"),
            time: get_number(pin, "time"),
            label: get_string(pin, "label"),
            resolved: get_bool_or(pin, "resolved", false),
            number: index + 1,
        })
        .collect();

    let video_id = use_hook(|| {
        format!(
            "annotated-media-video-{}",
            NEXT_VIDEO.fetch_add(1, Ordering::Relaxed)
        )
    });
    let mut frame = use_signal(|| None::<Rc<MountedData>>);
    let mut duration = use_signal(|| 0.0f64);

    let root = cn(&["flex w-full flex-col gap-2", class_name.as_str()]);
    let crosshair = if placing && place_event.is_some() {
        "cursor-crosshair"
    } else {
        ""
    };
    let frame_class = cn(&[
        "relative w-full overflow-hidden rounded-md bg-muted",
        crosshair,
    ]);
    let placed: Vec<PinSpec> = pins
        .iter()
        .filter(|pin| pin.x.is_some() && pin.y.is_some())
        .cloned()
        .collect();
    let timed: Vec<PinSpec> = if video {
        pins.iter()
            .filter(|pin| pin.time.is_some())
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    let total = duration();

    let place_video = video_id.clone();
    let metadata_video = video_id.clone();
    let place_name = place_event.clone();

    rsx! {
        div { class: "{root}", "data-pattern": "annotated-media",
            div {
                class: "{frame_class}",
                role: "region",
                "aria-label": "{title}",
                onmounted: move |event| frame.set(Some(event.data())),
                onclick: move |event| {
                    if !placing || place_name.is_none() {
                        return;
                    }
                    let Some(element) = frame.peek().clone() else { return; };
                    let point = event.client_coordinates();
                    let name = place_name.clone();
                    let video_id = place_video.clone();
                    spawn(async move {
                        let rect = match element.get_client_rect().await {
                            Ok(rect) => rect,
                            Err(error) => {
                                tracing::error!(%error, "annotated-media measure");
                                return;
                            }
                        };
                        if rect.size.width == 0.0 || rect.size.height == 0.0 {
                            return;
                        }
                        let time = if video {
                            let script = format!(
                                "var v=document.getElementById('{video_id}');if(v){{v.pause();dioxus.send(v.currentTime)}}else{{dioxus.send(0)}}"
                            );
                            video_number(script).await.map(round1).unwrap_or(0.0)
                        } else {
                            0.0
                        };
                        emit(
                            sink,
                            &name,
                            object_value([
                                ("x", number_value(round1(clamp((point.x - rect.origin.x) / rect.size.width * 100.0)))),
                                ("y", number_value(round1(clamp((point.y - rect.origin.y) / rect.size.height * 100.0)))),
                                ("time", number_value(time)),
                            ]),
                        );
                    });
                },
                if video {
                    video {
                        id: "{video_id}",
                        src: "{src}",
                        controls: true,
                        style: "display:block;width:100%",
                        onloadedmetadata: move |_| {
                            let script = format!("var v=document.getElementById('{metadata_video}');dioxus.send(v?v.duration:0)");
                            spawn(async move {
                                if let Some(seconds) = video_number(script).await {
                                    if seconds.is_finite() {
                                        duration.set(seconds);
                                    }
                                }
                            });
                        },
                    }
                } else {
                    img { src: "{src}", alt: "{title}", draggable: "false", style: "display:block;width:100%;user-select:none" }
                }
                for pin in placed.iter() {
                    {
                        let class = format!("absolute {}", pin_class(pin, active.as_deref()));
                        let style = format!(
                            "left:{}%;top:{}%;transform:translate(-50%,-50%)",
                            pin.x.unwrap_or(0.0),
                            pin.y.unwrap_or(0.0)
                        );
                        let label = pin.label.clone().unwrap_or_default();
                        let aria = pin.label.clone().unwrap_or_else(|| pin.number.to_string());
                        let pressed = active.as_deref() == Some(pin.id.as_str());
                        let select = select_event.clone();
                        let pin_id = pin.id.clone();
                        let pin_time = pin.time;
                        let seek_video = video_id.clone();
                        rsx! {
                            button {
                                key: "{pin.id}",
                                r#type: "button",
                                class: "{class}",
                                style: "{style}",
                                title: "{label}",
                                "aria-label": "{aria}",
                                "aria-pressed": "{pressed}",
                                onclick: move |event| {
                                    event.stop_propagation();
                                    if video {
                                        if let Some(time) = pin_time {
                                            let script = format!("var v=document.getElementById('{seek_video}');if(v){{v.currentTime={time};v.pause()}}");
                                            document::eval(&script);
                                        }
                                    }
                                    emit(sink, &select, object_value([("id", Value::string(pin_id.as_str()))]));
                                },
                                "{pin.number}"
                            }
                        }
                    }
                }
            }
            if !timed.is_empty() && total > 0.0 {
                div { class: "relative h-8 w-full rounded-md bg-muted", "aria-label": "Timeline",
                    for pin in timed.iter() {
                        {
                            let class = format!("absolute {}", pin_class(pin, active.as_deref()));
                            let time = pin.time.unwrap_or(0.0);
                            let style = format!(
                                "left:{}%;top:50%;transform:translate(-50%,-50%)",
                                clamp(time / total * 100.0)
                            );
                            let label = pin.label.clone().unwrap_or_default();
                            let aria = pin.label.clone().unwrap_or_else(|| pin.number.to_string());
                            let select = select_event.clone();
                            let pin_id = pin.id.clone();
                            let seek_video = video_id.clone();
                            rsx! {
                                button {
                                    key: "{pin.id}",
                                    r#type: "button",
                                    class: "{class}",
                                    style: "{style}",
                                    title: "{label}",
                                    "aria-label": "{aria}",
                                    onclick: move |event| {
                                        event.stop_propagation();
                                        let script = format!("var v=document.getElementById('{seek_video}');if(v){{v.currentTime={time};v.pause()}}");
                                        document::eval(&script);
                                        emit(sink, &select, object_value([("id", Value::string(pin_id.as_str()))]));
                                    },
                                    "{pin.number}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for AnnotatedMediaRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { AnnotatedMedia { props: props.clone(), view: view.to_owned() } }
    }
}
