//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use crate::styles::style_with_props;
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::runtime::theme::theme_keys;
use orbital_core::{ThemeMode, ThemePresets, Value};

fn theme_in_viewer_mode(key: &str, mode: Option<ThemeMode>) -> String {
    let Some(mode) = mode else {
        return key.to_string();
    };
    let base = key
        .strip_suffix("-light")
        .or_else(|| key.strip_suffix("-dark"));
    let Some(base) = base else {
        return key.to_string();
    };
    let Some(theme) = ThemePresets::installed().get(base) else {
        return key.to_string();
    };
    let keys = theme_keys(theme);
    if !keys.iter().any(|candidate| candidate == key) {
        return key.to_string();
    }
    let viewer_key = format!("{base}-{}", mode.as_str());
    if keys.contains(&viewer_key) {
        viewer_key
    } else {
        key.to_string()
    }
}

/// Dioxus renderer for the `box` pattern.
pub struct BoxRenderer;

impl BoxRenderer {
    const NAME: &str = "box";
}

impl DioxusPatternRenderer for BoxRenderer {
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
        let spacing = |key: &str, prefix: &str| -> String {
            let Some(value) = get_string(props, key) else {
                return String::new();
            };
            let step = match value.as_str() {
                "none" => "0",
                "xs" => "1",
                "sm" => "2",
                "md" => "4",
                "lg" => "6",
                "xl" => "8",
                "2xl" => "12",
                "auto" => "auto",
                _ => return String::new(),
            };
            format!("{prefix}-{step}")
        };
        let bg = match get_string_or(props, "bg", "transparent").as_str() {
            "primary" => "bg-primary text-primary-foreground",
            "secondary" => "bg-secondary text-secondary-foreground",
            "muted" => "bg-muted text-foreground",
            "accent" => "bg-accent text-accent-foreground",
            "surface" => "bg-card",
            "overlay" => "bg-card/80 backdrop-blur-sm",
            "gradient" => "band-gradient",
            "inverse" => "band-inverse",
            _ => "bg-transparent",
        };
        let rounded = match get_string_or(props, "rounded", "none").as_str() {
            "sm" => "rounded-sm",
            "md" => "rounded-md",
            "lg" => "rounded-lg",
            "xl" | "2xl" => "rounded-xl",
            "full" => "rounded-full",
            _ => "rounded-none",
        };
        let shadow = match get_string_or(props, "shadow", "none").as_str() {
            "sm" => "shadow-sm",
            "md" => "shadow",
            "lg" | "xl" => "shadow-lg",
            _ => "shadow-none",
        };
        let display = match get_string_or(props, "display", "").as_str() {
            "block" => "block",
            "inline" => "inline",
            "inline-block" => "inline-block",
            "flex" => "flex",
            "inline-flex" => "inline-flex",
            "grid" => "grid",
            _ => "",
        };
        let overflow = match get_string_or(props, "overflow", "").as_str() {
            "auto" => "overflow-auto",
            "hidden" => "overflow-hidden",
            "visible" => "overflow-visible",
            "scroll" => "overflow-scroll",
            _ => "",
        };
        let position = match get_string_or(props, "position", "").as_str() {
            "relative" => "relative",
            "absolute" => "absolute",
            "fixed" => "fixed",
            "sticky" => "sticky",
            _ => "",
        };
        let border = if get_bool_or(props, "border", false) {
            "border-[length:var(--border-width)] border-border"
        } else {
            ""
        };
        let width = if get_bool_or(props, "fullWidth", false) {
            "w-full"
        } else {
            ""
        };
        let height = if get_bool_or(props, "fullHeight", false) {
            "h-full"
        } else {
            ""
        };
        let edge = |key: &str| match get_string_or(props, key, "none").as_str() {
            "curve" => Some("curve"),
            "wave" => Some("wave"),
            "arc" => Some("arc"),
            "tilt" => Some("tilt"),
            "step" => Some("step"),
            "scallop" => Some("scallop"),
            _ => None,
        };
        let edge_top = edge("edgeTop");
        let edge_bottom = edge("edgeBottom");
        let edge_color = match get_string_or(props, "edgeColor", "background").as_str() {
            "muted" => "muted",
            "surface" => "surface",
            "gradient" => "gradient",
            "inverse" => "inverse",
            _ => "background",
        };
        let edge_flip = get_bool_or(props, "edgeFlip", false);
        let texture = get_bool_or(props, "texture", false);
        let is_band = texture || edge_top.is_some() || edge_bottom.is_some();
        let clickable = get_string(props, "action").is_some();
        let base = format!("{} {} {} {} {} {} {bg} {border} {rounded} {shadow} {display} {width} {height} {overflow} {position} {} {}",
            spacing("padding", "p"), spacing("paddingX", "px"), spacing("paddingY", "py"),
            spacing("margin", "m"), spacing("marginX", "mx"), spacing("marginY", "my"),
            if is_band { "band" } else { "" },
            if clickable { "cursor-pointer" } else { "" });
        let class = merge_class_name(&base, props);
        let max_width = get_string_or(props, "maxWidth", "");
        let mut base_style = if max_width.is_empty() {
            String::new()
        } else {
            format!("max-width:{max_width}")
        };
        let pad = get_string(props, "paddingY").or_else(|| get_string(props, "padding"));
        let pad_rem = match pad.as_deref() {
            Some("none") => Some("0rem"),
            Some("xs") => Some("0.25rem"),
            Some("sm") => Some("0.5rem"),
            Some("md") => Some("1rem"),
            Some("lg") => Some("1.5rem"),
            Some("xl") => Some("2rem"),
            Some("2xl") => Some("3rem"),
            _ => None,
        };
        if let Some(pad) = pad_rem {
            for (side, edge) in [("top", edge_top), ("bottom", edge_bottom)] {
                if let Some(edge) = edge {
                    let height = if edge == "curve" {
                        "var(--surface-edge-height, clamp(24px, 6vw, 96px))"
                    } else {
                        "clamp(24px, 6vw, 96px)"
                    };
                    if !base_style.is_empty() {
                        base_style.push(';');
                    }
                    base_style.push_str(&format!("padding-{side}:calc({pad} + {height})"));
                }
            }
        }
        let style = style_with_props(&base_style, props);
        let action = get_string(props, "action");
        let data_theme =
            get_string(props, "data-theme").map(|key| theme_in_viewer_mode(&key, view.theme_mode));
        let data_motion = get_bool_or(props, "animateChanges", false).then_some("presentation");
        let action_payload = props
            .as_object()
            .and_then(|fields| fields.get("actionPayload"))
            .cloned()
            .unwrap_or_else(|| Value::Object(orbital_core::ValueMap::new().into()));
        let hover_event = get_string(props, "hoverEvent");
        let hover_out = hover_event.clone();
        let sink = view.sink;
        let children = child_patterns(props, view.trait_content);
        let top_edge_class = edge_top.map(|edge| {
            format!(
                "band-edge band-edge-top band-edge-shape-{edge} band-edge-color-{edge_color} {}",
                if edge_flip { "band-edge-flip" } else { "" }
            )
        });
        let bottom_edge_class = edge_bottom.map(|edge| {
            format!(
                "band-edge band-edge-bottom band-edge-shape-{edge} band-edge-color-{edge_color} {}",
                if edge_flip { "band-edge-flip" } else { "" }
            )
        });
        rsx! { div { class: "{class}", style: "{style}", "data-theme": data_theme.as_deref(), "data-motion": data_motion,
            onclick: move |_| { if let (Some(action), Some(sink)) = (&action, sink) { sink.call((action.clone(), action_payload.clone())); } },
            onmouseenter: move |_| { if let (Some(event), Some(sink)) = (&hover_event, sink) { let mut payload = orbital_core::ValueMap::new(); payload.insert("hovered".into(), Value::Bool(true)); sink.call((event.clone(), Value::Object(payload.into()))); } },
            onmouseleave: move |_| { if let (Some(event), Some(sink)) = (&hover_out, sink) { let mut payload = orbital_core::ValueMap::new(); payload.insert("hovered".into(), Value::Bool(false)); sink.call((event.clone(), Value::Object(payload.into()))); } },
            ..shared_root_attrs(props),
            if texture { span { class: "band-texture", "data-band-layer": "", aria_hidden: "true" } }
            if let Some(edge_class) = top_edge_class { span { class: "{edge_class}", "data-band-layer": "", aria_hidden: "true" } }
            if let Some(edge_class) = bottom_edge_class { span { class: "{edge_class}", "data-band-layer": "", aria_hidden: "true" } }
            for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
        } }
    }
}
