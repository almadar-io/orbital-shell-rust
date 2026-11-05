use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_number_or, get_string_or, };
use orbital_core::{Value, ValueMap};

pub struct DialogueBubbleRenderer;

impl DioxusPatternRenderer for DialogueBubbleRenderer {
    fn pattern_name(&self) -> &str {
        "dialogue-bubble"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let class = merge_class_name("almadar-dialogue-bubble", props);
        let position = get_string_or(props, "position", "bottom");
        let mood = get_string_or(props, "mood", "neutral");
        let speaker = get_string_or(props, "speaker", "");
        let text = get_string_or(props, "text", "");
        let visible_text = if props
            .as_object()
            .and_then(|p| p.get("revealedChars"))
            .is_some()
        {
            let count = get_number_or(props, "revealedChars", 0.0).max(0.0) as usize;
            text.chars().take(count).collect::<String>()
        } else {
            text
        };
        let portrait = props
            .as_object()
            .and_then(|p| p.get("portrait"))
            .cloned()
            .map(|asset| {
                let mut spec = ValueMap::new();
                spec.insert("type".into(), Value::String("atlas-image".into()));
                spec.insert("asset".into(), asset);
                spec.insert("size".into(), Value::Number(48.0));
                Value::Object(spec.into())
            });
        let ring = match mood.as_str() {
            "happy" => "var(--color-success,#22c55e)",
            "concerned" => "var(--color-info,#3b82f6)",
            "angry" => "var(--color-error,#ef4444)",
            _ => "var(--color-warning,#f59e0b)",
        };
        let corner = if position == "top" {
            "border-bottom-left-radius:0;"
        } else {
            "border-top-left-radius:0;"
        };
        let bubble_style = format!("display:flex;align-items:flex-start;gap:12px;padding:12px 16px;border-radius:var(--radius-container,12px);background:var(--color-background,#fff);color:var(--color-foreground,#111);{corner}");
        let portrait_style = format!(
            "width:48px;height:48px;border-radius:50%;object-fit:cover;border:3px solid {ring};"
        );
        rsx! {
            div {
                class: "{class} almadar-dialogue-bubble--{position} almadar-dialogue-bubble--{mood}",
                style: "{bubble_style}",
                ..shared_root_attrs(props),
                if let Some(spec) = portrait {
                    div {
                        class: "almadar-dialogue-bubble__portrait",
                        style: "{portrait_style}overflow:hidden;flex-shrink:0;",
                        role: "img",
                        aria_label: "{speaker}",
                        {registry.render_spec(&spec, None, view)}
                    }
                }
                div {
                    class: "almadar-dialogue-bubble__copy",
                    style: "display:flex;flex-direction:column;gap:4px;min-width:0;",
                    if !speaker.is_empty() {
                        span { class: "almadar-dialogue-bubble__speaker", style: "font-weight:700;", "{speaker}" }
                    }
                    span { class: "almadar-dialogue-bubble__text", "{visible_text}" }
                }
            }
        }
    }
}
