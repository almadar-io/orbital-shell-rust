use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_bool_or, get_number_or, get_string_or, };
use orbital_core::client::recurse::child_patterns;
use orbital_core::Value;
use std::time::Duration;

use crate::timer::Delay;

pub struct TooltipRenderer;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum TooltipTone {
    Primary,
    Foreground,
}

pub(crate) fn position_classes(position: &str) -> &'static str {
    match position {
        "bottom" => "top-full left-1/2 -translate-x-1/2 mt-2",
        "left" => "right-full top-1/2 -translate-y-1/2 mr-2",
        "right" => "left-full top-1/2 -translate-y-1/2 ml-2",
        _ => "bottom-full left-1/2 -translate-x-1/2 mb-2",
    }
}

pub(crate) fn arrow_classes(position: &str, tone: TooltipTone) -> &'static str {
    match (position, tone) {
        ("bottom", TooltipTone::Primary) => "bottom-full left-1/2 -translate-x-1/2 border-b-primary border-l-transparent border-r-transparent border-t-transparent",
        ("left", TooltipTone::Primary) => "left-full top-1/2 -translate-y-1/2 border-l-primary border-t-transparent border-b-transparent border-r-transparent",
        ("right", TooltipTone::Primary) => "right-full top-1/2 -translate-y-1/2 border-r-primary border-t-transparent border-b-transparent border-l-transparent",
        (_, TooltipTone::Primary) => "top-full left-1/2 -translate-x-1/2 border-t-primary border-l-transparent border-r-transparent border-b-transparent",
        ("bottom", TooltipTone::Foreground) => "bottom-full left-1/2 -translate-x-1/2 border-b-foreground border-l-transparent border-r-transparent border-t-transparent",
        ("left", TooltipTone::Foreground) => "left-full top-1/2 -translate-y-1/2 border-l-foreground border-t-transparent border-b-transparent border-r-transparent",
        ("right", TooltipTone::Foreground) => "right-full top-1/2 -translate-y-1/2 border-r-foreground border-t-transparent border-b-transparent border-l-transparent",
        (_, TooltipTone::Foreground) => "top-full left-1/2 -translate-x-1/2 border-t-foreground border-l-transparent border-r-transparent border-b-transparent",
    }
}

#[component]
pub(crate) fn TooltipFrame(
    trigger: Element,
    content: Element,
    root_class: String,
    panel_class: String,
    position: String,
    delay_ms: u64,
    hide_delay_ms: u64,
    show_arrow: bool,
    arrow_tone: TooltipTone,
) -> Element {
    let mut hovering = use_signal(|| false);
    let mut hover_open = use_signal(|| false);
    let mut tap_open = use_signal(|| false);
    let mut generation = use_signal(|| 0_u64);
    let open = hover_open() || tap_open();
    let position_class = position_classes(&position);
    let arrow_class = arrow_classes(&position, arrow_tone);
    let trigger_class = if tap_open() {
        format!("{root_class} z-50")
    } else {
        root_class
    };

    rsx! {
        if tap_open() {
            div { class: "fixed inset-0 z-40", onclick: move |_| tap_open.set(false) }
        }
        span {
            class: "{trigger_class}",
            onmousemove: move |_| {
                if !*hovering.peek() {
                    hovering.set(true);
                    let next = generation.peek().wrapping_add(1);
                    generation.set(next);
                    spawn(async move {
                        Delay::new(Duration::from_millis(delay_ms)).await;
                        if *generation.peek() == next && *hovering.peek() {
                            hover_open.set(true);
                        }
                    });
                }
            },
            onmouseleave: move |_| {
                hovering.set(false);
                let next = generation.peek().wrapping_add(1);
                generation.set(next);
                if hide_delay_ms == 0 {
                    hover_open.set(false);
                } else {
                    spawn(async move {
                        Delay::new(Duration::from_millis(hide_delay_ms)).await;
                        if *generation.peek() == next {
                            hover_open.set(false);
                        }
                    });
                }
            },
            onclick: move |_| tap_open.set(!tap_open()),
            {trigger}
            if open {
                span {
                    class: "absolute z-50 {position_class} {panel_class}",
                    role: "tooltip",
                    onclick: move |event| event.stop_propagation(),
                    {content}
                    if show_arrow {
                        span { class: "absolute w-0 h-0 border-heavy {arrow_class}" }
                    }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for TooltipRenderer {
    fn pattern_name(&self) -> &str {
        "tooltip"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let trigger_children = child_patterns(props, view.trait_content);
        let trigger = rsx! {
            for (child, source) in trigger_children.iter() {
                {view.render_child(registry, child, source.as_deref())}
            }
        };
        let content = props.as_object().and_then(|object| object.get("content"));
        let content = match content {
            Some(Value::String(text)) => rsx! { span { "{text}" } },
            Some(spec) => registry.render_spec(spec, None, view),
            None => rsx! {},
        };
        let position = get_string_or(props, "position", "top");
        let delay_ms = get_number_or(props, "delay", 200.0).max(0.0) as u64;
        let hide_delay_ms = get_number_or(props, "hideDelay", 0.0).max(0.0) as u64;
        let show_arrow = get_bool_or(props, "showArrow", true);
        let root_class = "relative inline-block".to_string();
        let panel_class = merge_class_name(
            "px-3 py-2 max-w-xs bg-primary text-primary-foreground shadow-elevation-popover rounded-container text-sm break-words whitespace-normal",
            props,
        );
        rsx! {
            span { ..shared_root_attrs(props),
                TooltipFrame { trigger, content, root_class, panel_class, position,
                    delay_ms, hide_delay_ms, show_arrow, arrow_tone: TooltipTone::Primary }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_position_has_distinct_panel_and_arrow_placement() {
        let positions = ["top", "bottom", "left", "right"];
        for position in positions {
            assert!(!position_classes(position).is_empty());
            assert!(arrow_classes(position, TooltipTone::Primary).contains("primary"));
            assert!(arrow_classes(position, TooltipTone::Foreground).contains("foreground"));
        }
        assert_ne!(position_classes("top"), position_classes("bottom"));
        assert_ne!(position_classes("left"), position_classes("right"));
    }
}
