use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{emit_empty_event, DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use crate::timer::Delay;
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use std::time::Duration;

pub struct PresenceRenderer;

fn finish_exit(mut mounted: Signal<bool>, sink: Option<EventSink>, on_exited: Option<&str>) {
    if *mounted.peek() {
        mounted.set(false);
        emit_empty_event(sink, on_exited);
    }
}

#[component]
fn PresenceFrame(
    show: bool,
    animate: bool,
    animation: String,
    class: String,
    on_exited: Option<String>,
    sink: Option<EventSink>,
    children: Element,
) -> Element {
    let previous = use_hook(|| Rc::new(RefCell::new(show)));
    let mut mounted = use_signal(|| show);
    let mut exit_generation = use_signal(|| 0_u64);
    let changed = *previous.borrow() != show;
    if changed {
        *previous.borrow_mut() = show;
        let next = exit_generation.peek().saturating_add(1);
        exit_generation.set(next);
        if show {
            mounted.set(true);
        } else if !animate {
            let on_exited = on_exited.clone();
            spawn(async move { finish_exit(mounted, sink, on_exited.as_deref()) });
        } else {
            #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
            let timeout_event = on_exited.clone();
            #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
            spawn(async move {
                Delay::new(Duration::from_millis(1000)).await;
                if *exit_generation.peek() == next {
                    finish_exit(mounted, sink, timeout_event.as_deref());
                }
            });
            #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
            {
                let immediate_event = on_exited.clone();
                spawn(async move { finish_exit(mounted, sink, immediate_event.as_deref()) });
            }
        }
    }
    if !mounted() {
        return rsx! {};
    }
    let motion_class = if !animate {
        String::new()
    } else if show {
        format!("animate-{animation}-in")
    } else {
        format!("animate-{animation}-out")
    };
    rsx! {
        div {
            class: "{class} {motion_class}",
            onanimationend: move |_| {
                if !show { finish_exit(mounted, sink, on_exited.as_deref()); }
            },
            {children}
        }
    }
}

impl DioxusPatternRenderer for PresenceRenderer {
    fn pattern_name(&self) -> &str {
        "presence"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let show = get_bool_or(props, "show", true);
        let animate = get_bool_or(props, "animate", true);
        let animation = get_string_or(props, "animation", "fade");
        let class = merge_class_name("almadar-presence", props);
        let on_exited = get_string(props, "onExited");
        let sink = view.sink;
        let children = child_patterns(props, view.trait_content);
        let children = rsx! { for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} } };
        rsx! { PresenceFrame { show, animate, animation, class, on_exited, sink, children } }
    }
}
