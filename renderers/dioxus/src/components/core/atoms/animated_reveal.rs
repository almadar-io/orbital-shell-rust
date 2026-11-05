use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use crate::visibility::{intersection, use_visibility};

pub struct AnimatedRevealRenderer;

#[component]
fn RevealFrame(
    trigger: String,
    animation: String,
    duration: f64,
    delay: f64,
    easing: String,
    threshold: f64,
    once: bool,
    animate: bool,
    reveal_event: Option<String>,
    enter_event: Option<String>,
    leave_event: Option<String>,
    sink: Option<EventSink>,
    class: String,
    children: Element,
) -> Element {
    let mut hovering = use_signal(|| false);
    let mut has_animated = use_signal(|| false);
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    let (visibility_id, visibility) = use_visibility();
    #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
    let visibility_id = 0_usize;
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    let hit = intersection(visibility(), [0.0; 4]);
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    let seen_enough = hit.visible
        && (hit.element_fraction >= threshold as f32
            || hit.viewport_height_fraction >= threshold as f32);
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    let intersects = hit.visible;
    #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
    let seen_enough = true;
    #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
    let intersects = true;
    let observation = use_hook(|| Rc::new(RefCell::new((false, false))));
    let effect_trigger = trigger.clone();
    let mut effect = use_effect(move || {
        if effect_trigger != "scroll" {
            return;
        }
        let mut previous = observation.borrow_mut();
        if hit_visible(
            intersects,
            seen_enough,
            &mut previous,
            &enter_event,
            &leave_event,
            &reveal_event,
            sink,
        ) {
            has_animated.set(true);
        }
    });
    effect.mark_dirty();
    let active = match trigger.as_str() {
        "manual" => animate,
        "hover" => hovering(),
        _ => seen_enough || (once && has_animated()),
    };
    let state = match (animation.as_str(), active) {
        ("fade-up", false) => "opacity:0;transform:translateY(24px)",
        ("fade-up", true) | ("fade-down", true) => "opacity:1;transform:translateY(0)",
        ("fade-down", false) => "opacity:0;transform:translateY(-24px)",
        ("fade-in", false) => "opacity:0",
        ("fade-in", true) => "opacity:1",
        ("fade-left", false) => "opacity:0;transform:translateX(24px)",
        ("fade-right", false) => "opacity:0;transform:translateX(-24px)",
        ("fade-left", true) | ("fade-right", true) => "opacity:1;transform:translateX(0)",
        ("scale", false) => "opacity:0;transform:scale(0.92)",
        ("scale", true) => "opacity:1;transform:scale(1)",
        ("scale-up", false) => "opacity:0;transform:scale(0.92) translateY(16px)",
        ("scale-up", true) => "opacity:1;transform:scale(1) translateY(0)",
        _ => "",
    };
    let style = format!("{state};transition-property:opacity,transform;transition-duration:{duration}ms;transition-delay:{delay}ms;transition-timing-function:{easing}");
    let is_hover = trigger == "hover";
    rsx! {
        div {
            class: "{class}",
            style: "{style}",
            "data-orbital-visibility": "{visibility_id}",
            onmouseenter: move |_| {
                if is_hover {
                    hovering.set(true);
                }
            },
            onmouseleave: move |_| {
                if is_hover && (!once || !has_animated()) {
                    has_animated.set(true);
                    hovering.set(false);
                }
            },
            {children}
        }
    }
}

fn hit_visible(
    intersects: bool,
    seen: bool,
    previous: &mut (bool, bool),
    enter: &Option<String>,
    leave: &Option<String>,
    reveal: &Option<String>,
    sink: Option<EventSink>,
) -> bool {
    if intersects != previous.0 {
        if let (Some(sink), Some(event)) = (sink, if intersects { enter } else { leave }) {
            sink.call((
                event.clone(),
                Value::Object(orbital_core::ValueMap::new().into()),
            ));
        }
        previous.0 = intersects;
    }
    if seen && !previous.1 {
        previous.1 = true;
        if let (Some(sink), Some(event)) = (sink, reveal) {
            sink.call((
                event.clone(),
                Value::Object(orbital_core::ValueMap::new().into()),
            ));
        }
        return true;
    }
    false
}

impl DioxusPatternRenderer for AnimatedRevealRenderer {
    fn pattern_name(&self) -> &str {
        "animated-reveal"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let trigger = get_string_or(props, "trigger", "scroll");
        let animation = get_string_or(props, "animation", "fade-up");
        let duration = get_number_or(props, "duration", 600.0).max(0.0);
        let delay = get_number_or(props, "delay", 0.0).max(0.0);
        let easing = get_string_or(props, "easing", "cubic-bezier(0.16, 1, 0.3, 1)");
        let threshold = get_number_or(props, "threshold", 0.15).clamp(0.0, 1.0);
        let once = get_bool_or(props, "once", true);
        let animate = get_bool_or(props, "animate", false);
        let reveal_event = get_string(props, "revealEvent");
        let enter_event = get_string(props, "enterEvent");
        let leave_event = get_string(props, "leaveEvent");
        let sink = view.sink;
        let class = merge_class_name("", props);
        let children = child_patterns(props, view.trait_content);
        let children = rsx! { for (child, _src) in children.iter() { {registry.render_spec(child, None, view)} } };
        rsx! { RevealFrame { trigger, animation, duration, delay, easing, threshold, once, animate, reveal_event, enter_event, leave_event, sink, class, children } }
    }
}
