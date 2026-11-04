use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

pub struct AnimatedGraphicRenderer;

impl DioxusPatternRenderer for AnimatedGraphicRenderer {
    fn pattern_name(&self) -> &str {
        "animated-graphic"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let class = merge_class_name(
            "almadar-animated-graphic inline-flex items-center justify-center",
            props,
        );
        let svg = get_string(props, "svgContent").filter(|value| !value.is_empty());
        let src = get_string(props, "src").filter(|value| !value.is_empty());
        let alt = get_string(props, "alt").filter(|value| !value.is_empty());
        let animation = get_string_or(props, "animation", "draw");
        let animate = get_bool_or(props, "animate", false);
        let duration = get_number_or(props, "duration", 1200.0).max(0.0);
        let delay = get_number_or(props, "delay", 0.0).max(0.0);
        let easing = get_string_or(props, "easing", "cubic-bezier(0.16, 1, 0.3, 1)");
        let dim = |key: &str| -> Option<String> {
            match props.as_object().and_then(|fields| fields.get(key)) {
                Some(value @ (Value::Number(_) | Value::Int(_))) => {
                    Some(format!("{}px", value.to_string_value()))
                }
                Some(Value::String(value)) if !value.is_empty() => Some(value.clone().into_inner()),
                _ => None,
            }
        };
        let mut style = format!("--almadar-graphic-duration:{duration}ms;--almadar-graphic-delay:{delay}ms;--almadar-graphic-easing:{easing};");
        if let Some(width) = dim("width") {
            style.push_str(&format!("width:{width};"));
        }
        if let Some(height) = dim("height") {
            style.push_str(&format!("height:{height};"));
        }
        if let Some(stroke) = get_string(props, "strokeColor") {
            style.push_str(&format!("--almadar-graphic-stroke:{stroke};"));
        }
        if let Some(fill) = get_string(props, "fillColor") {
            style.push_str(&format!("--almadar-graphic-fill:{fill};"));
        }
        let role = alt.as_ref().map(|_| "img");
        let children = child_patterns(props, view.trait_content);
        rsx! {
            div {
                class: "{class}",
                style: "{style}",
                role: role,
                aria_label: alt.clone(),
                "data-orbital-animation": "{animation}",
                "data-orbital-animate": "{animate}",
                if let Some(markup) = svg {
                    div { class: "w-full h-full [&>svg]:w-full [&>svg]:h-full", aria_hidden: "true", dangerous_inner_html: "{markup}" }
                } else if let Some(url) = src {
                    img { class: "w-full h-full", src: "{url}", alt: "" }
                } else {
                    for (child, source) in children.iter() { {view.render_child(registry, child, source.as_deref())} }
                }
            }
        }
    }
}
