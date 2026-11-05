//! Native Dioxus body for the `carousel` pattern, mirroring `Carousel.tsx`.

use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    number_value, object_value, pattern_spec, DioxusPatternRenderer, DioxusView, EventSink,
    OwnedDioxusView, BOX_ATOM_BASE,
};
use crate::timer::Delay;
use dioxus::html::ScrollBehavior;
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::{Value, ValueMap};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

pub struct CarouselRenderer;

#[derive(Props, Clone, PartialEq)]
struct CarouselProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn Carousel(input: CarouselProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let items = get_array(props, "items");
    let total = items.len();
    let vertical = get_string_or(props, "orientation", "horizontal") == "vertical";
    let full_height = get_bool_or(props, "fullHeight", false);
    let viewport_bound = vertical && full_height;
    let show_dots = get_bool_or(props, "showDots", true);
    let show_arrows = get_bool_or(props, "showArrows", true);
    let looping = get_bool_or(props, "loop", false);
    let auto_play = get_bool_or(props, "autoPlay", false);
    let interval = get_number_or(props, "autoPlayInterval", 5000.0).max(0.0) as u64;
    let change_event = get_string(props, "slideChangeEvent");
    let change_payload = props
        .get("slideChangePayload")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(ValueMap::new);
    let load_event = get_string(props, "loadMoreEvent");
    let load_ahead = get_number_or(props, "loadMoreAhead", 2.0) as i64;
    let has_more = get_bool_or(props, "hasMore", false);
    let loading_more = get_bool_or(props, "isLoadingMore", false);
    let sink = view.sink;

    let mut active = use_signal(|| 0i64);
    let mut announced = use_signal(|| 0i64);
    let mut requested_at = use_signal(|| None::<usize>);
    let mut top_offset = use_signal(|| 0.0f64);
    let mut slides = use_signal(HashMap::<usize, Rc<MountedData>>::new);
    let last = total as i64 - 1;

    let emit_change: Callback<i64> = Callback::new(move |index: i64| {
        announced.set(index);
        if let (Some(sink), Some(event)) = (sink, &change_event) {
            let mut fields = change_payload.clone();
            fields.insert("index".into(), number_value(index as f64));
            sink.call((event.clone(), Value::Object(fields.into())));
        }
    });

    let go: Callback<i64> = Callback::new(move |index: i64| {
        let target = if looping {
            if index < 0 {
                last
            } else if index > last {
                0
            } else {
                index
            }
        } else {
            index.clamp(0, last.max(0))
        };
        active.set(target);
        emit_change.call(target);
        if let Some(slide) = slides.peek().get(&(target.max(0) as usize)).cloned() {
            spawn(async move {
                if let Err(error) = slide.scroll_to(ScrollBehavior::Smooth).await {
                    tracing::error!(%error, "carousel scroll");
                }
            });
        }
    });

    use_future(move || async move {
        if !auto_play || total <= 1 {
            return;
        }
        loop {
            Delay::new(Duration::from_millis(interval)).await;
            let current = active();
            let next = if looping {
                (current + 1) % total as i64
            } else {
                (current + 1).min(last)
            };
            go.call(next);
        }
    });

    let load_name = load_event.clone();
    use_effect(move || {
        let Some(event) = &load_name else { return };
        let current = active();
        if !has_more || loading_more || total == 0 || current < last - load_ahead {
            return;
        }
        if *requested_at.peek() == Some(total) {
            return;
        }
        requested_at.set(Some(total));
        if let Some(sink) = sink {
            sink.call((
                event.clone(),
                object_value([("index", number_value(current as f64))]),
            ));
        }
    });

    if total == 0 {
        return rsx! {};
    }

    let lambda = props
        .get("renderItem")
        .and_then(render_item_lambda)
        .or_else(|| props.get("children").and_then(render_item_lambda));
    let slide_nodes: Vec<Option<Value>> = items
        .iter()
        .enumerate()
        .map(|(index, item)| lambda.as_ref().map(|lambda| lambda.apply(item, index)))
        .collect();

    let relay: EventSink = Callback::new(move |(_, payload): (String, Value)| {
        if let Some(delta) = payload.get("dir").and_then(Value::as_int) {
            go.call(active() + delta);
        }
    });
    let button_view = view.for_source(view.source_trait.clone(), relay);
    let current = active();
    let has_prev = looping || current > 0;
    let has_next = looping || current < last;

    let root_class = merge_class_name(
        &format!(
            "{BOX_ATOM_BASE} relative overflow-hidden w-full {}",
            if full_height && !viewport_bound {
                "h-full"
            } else {
                ""
            }
        ),
        props,
    );
    let root_style = if viewport_bound {
        format!("height:calc(100svh - {}px)", top_offset())
    } else {
        String::new()
    };
    let track_class = format!(
        "{BOX_ATOM_BASE} flex overflow-auto w-full scroll-smooth scrollbar-none {} [&::-webkit-scrollbar]:hidden [-ms-overflow-style:none]",
        if vertical { "[scroll-snap-type:y_mandatory]" } else { "[scroll-snap-type:x_mandatory]" }
    );
    let track_style = format!(
        "flex-direction:{};scroll-snap-type:{};{}",
        if vertical { "column" } else { "row" },
        if vertical {
            "y mandatory"
        } else {
            "x mandatory"
        },
        if full_height { "height:100%" } else { "" }
    );
    let slide_style = format!(
        "scroll-snap-align:start;{}{}",
        if vertical {
            "scroll-snap-stop:always;"
        } else {
            ""
        },
        if full_height { "height:100%" } else { "" }
    );
    let prev_pos = if vertical {
        "right-2 top-2 z-10"
    } else {
        "left-2 top-1/2 -translate-y-1/2 z-10"
    };
    let next_pos = if vertical {
        "right-2 bottom-2 z-10"
    } else {
        "right-2 top-1/2 -translate-y-1/2 z-10"
    };
    let arrow = |icon: &str, label: &str, dir: i64| {
        pattern_spec("button", [
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("icon", Value::string(icon)),
            ("label", Value::string("")),
            ("aria-label", Value::string(label)),
            ("action", Value::string("CAROUSEL_STEP")),
            ("actionPayload", object_value([("dir", Value::Int(dir))])),
            ("className", Value::string("rounded-full bg-surface/80 backdrop-blur-sm shadow-elevation-interactive hover:bg-surface")),
        ])
    };
    let prev_button = arrow(
        if vertical {
            "chevron-up"
        } else {
            "chevron-left"
        },
        "Previous slide",
        -1,
    );
    let next_button = arrow(
        if vertical {
            "chevron-down"
        } else {
            "chevron-right"
        },
        "Next slide",
        1,
    );
    let attrs: Vec<Attribute> = crate::registry::shared_root_attrs(props)
        .into_iter()
        .filter(|attr| !matches!(attr.name, "role" | "tabindex"))
        .collect();

    rsx! {
        div {
            class: "{root_class}",
            style: "{root_style}",
            role: "region",
            "aria-roledescription": "carousel",
            tabindex: "0",
            onmounted: move |event| {
                let element = event.data();
                spawn(async move {
                    match element.get_client_rect().await {
                        Ok(rect) => top_offset.set(rect.origin.y.max(0.0).round()),
                        Err(error) => tracing::error!(%error, "carousel measure"),
                    }
                });
            },
            onkeydown: move |event| {
                let (next, prev): (&[Key], &[Key]) = if vertical {
                    (&[Key::ArrowDown, Key::PageDown], &[Key::ArrowUp, Key::PageUp])
                } else {
                    (&[Key::ArrowRight], &[Key::ArrowLeft])
                };
                let key = event.key();
                if next.contains(&key) {
                    event.prevent_default();
                    go.call(active() + 1);
                } else if prev.contains(&key) {
                    event.prevent_default();
                    go.call(active() - 1);
                }
            },
            ..attrs,
            div {
                class: "{track_class}",
                style: "{track_style}",
                onscroll: move |event| {
                    let data = event.data();
                    let (size, position) = if vertical {
                        (data.client_height() as f64, data.scroll_top() as f64)
                    } else {
                        (data.client_width() as f64, data.scroll_left() as f64)
                    };
                    if size == 0.0 { return; }
                    let index = (position / size).round() as i64;
                    if index < 0 || index >= total as i64 { return; }
                    active.set(index);
                    if (position - index as f64 * size).abs() <= 1.0 && index != announced() {
                        emit_change.call(index);
                    }
                },
                for (index, node) in slide_nodes.iter().enumerate() {
                    div {
                        key: "{index}",
                        class: "{BOX_ATOM_BASE} flex-shrink-0 w-full [scroll-snap-align:start]",
                        style: "{slide_style}",
                        onmounted: move |event| { slides.write().insert(index, event.data()); },
                        if let Some(node) = node { {registry.render_spec(node, None, &view)} }
                    }
                }
            }
            if show_arrows && has_prev && total > 1 {
                div { class: "{BOX_ATOM_BASE} absolute {prev_pos}", {registry.render_spec(&prev_button, None, &button_view)} }
            }
            if show_arrows && has_next && total > 1 {
                div { class: "{BOX_ATOM_BASE} absolute {next_pos}", {registry.render_spec(&next_button, None, &button_view)} }
            }
            if show_dots && total > 1 {
                div { class: "{BOX_ATOM_BASE} absolute bottom-3 left-0 right-0 z-10",
                    div { class: "flex flex-row gap-1 items-center justify-center", role: "tablist", "aria-label": "Slides",
                        for index in 0..total {
                            {
                                let is_active = index as i64 == current;
                                let size = if is_active { 10 } else { 8 };
                                let color = if is_active { "var(--color-primary)" } else { "var(--color-muted, #d4d4d8)" };
                                let opacity = if is_active { "1" } else { "0.6" };
                                let number = index + 1;
                                rsx! {
                                    div {
                                        key: "{index}",
                                        class: "{BOX_ATOM_BASE} rounded-full cursor-pointer transition-all duration-fast",
                                        style: "width:{size}px;height:{size}px;background-color:{color};opacity:{opacity}",
                                        role: "tab",
                                        "aria-selected": "{is_active}",
                                        tabindex: if is_active { "0" } else { "-1" },
                                        "aria-label": "Go to slide {number}",
                                        onclick: move |_| go.call(index as i64),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl CarouselRenderer {
    const NAME: &str = "carousel";
}

impl DioxusPatternRenderer for CarouselRenderer {
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
        rsx! { Carousel { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
