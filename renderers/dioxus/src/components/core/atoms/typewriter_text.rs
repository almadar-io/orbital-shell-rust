use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{emit_empty_event, DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use crate::timer::{animate, Delay};
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use std::time::Duration;

pub struct TypewriterTextRenderer;

#[derive(Props, Clone, PartialEq)]
struct TypewriterProps {
    value: Value,
    sink: Option<EventSink>,
}

#[allow(non_snake_case)]
fn Typewriter(props: TypewriterProps) -> Element {
    let text = get_string_or(
        &props.value,
        "text",
        get_string_or(&props.value, "content", "").as_str(),
    );
    let speed = get_number_or(&props.value, "speed", 40.0).max(0.0) as u64;
    let start_delay = get_number_or(&props.value, "startDelay", 0.0).max(0.0) as u64;
    let chars = text.chars().count();
    let on_complete = get_string(&props.value, "onComplete");
    let sink = props.sink;
    let previous = use_hook(|| Rc::new(RefCell::new(None::<(String, u64, u64)>)));
    let mut visible = use_signal(|| 0_usize);
    let mut generation = use_signal(|| 0_u64);
    let key = (text.clone(), speed, start_delay);
    let changed = *previous.borrow() != Some(key.clone());
    if changed {
        *previous.borrow_mut() = Some(key);
        let next = generation.peek().saturating_add(1);
        generation.set(next);
        visible.set(0);
        #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
        spawn(async move {
            if start_delay > 0 {
                Delay::new(Duration::from_millis(start_delay)).await;
            }
            if *generation.peek() != next {
                return;
            }
            animate(
                Duration::from_millis(speed.saturating_mul(chars as u64)),
                move |progress| {
                    if *generation.peek() != next {
                        return false;
                    }
                    visible.set(((progress * chars as f64).floor() as usize).min(chars));
                    true
                },
            )
            .await;
            if *generation.peek() == next {
                emit_empty_event(sink, on_complete.as_deref());
            }
        });
        #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
        {
            visible.set(chars);
            spawn(async move {
                if *generation.peek() == next {
                    emit_empty_event(sink, on_complete.as_deref());
                }
            });
        }
    }
    let count = visible().min(chars);
    let displayed: String = text.chars().take(count).collect();
    let class = merge_class_name("almadar-typewriter-text", &props.value);
    rsx! {
        span { class: "{class}", aria_label: "{text}",
            "{displayed}"
            if count < chars { span { class: "almadar-typewriter-text__cursor", aria_hidden: "true" } }
        }
    }
}

impl DioxusPatternRenderer for TypewriterTextRenderer {
    fn pattern_name(&self) -> &str {
        "typewriter-text"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { Typewriter { value: props.clone(), sink: view.sink } }
    }
}
