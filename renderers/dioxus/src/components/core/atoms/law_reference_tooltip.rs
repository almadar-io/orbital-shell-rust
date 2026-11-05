use crate::renderer::merge_class_name;
use crate::components::core::molecules::tooltip::{TooltipFrame, TooltipTone};
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_string, get_string_or, };
use orbital_core::client::recurse::child_patterns;
use orbital_core::Value;

pub struct LawReferenceTooltipRenderer;

impl DioxusPatternRenderer for LawReferenceTooltipRenderer {
    fn pattern_name(&self) -> &str {
        "law-reference-tooltip"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let reference = props.as_object().and_then(|object| object.get("reference"));
        let law = reference
            .and_then(|value| get_string(value, "law"))
            .unwrap_or_default();
        let article = reference
            .and_then(|value| get_string(value, "article"))
            .unwrap_or_default();
        let law_name = reference.and_then(|value| get_string(value, "lawName"));
        let clause = reference.and_then(|value| get_string(value, "clause"));
        let link = reference.and_then(|value| get_string(value, "link"));
        let position = get_string_or(props, "position", "top");
        let root_class = merge_class_name("relative inline-block", props);
        let trigger_children = child_patterns(props, view.trait_content);
        let trigger = rsx! {
            for (child, source) in trigger_children.iter() {
                {view.render_child(registry, child, source.as_deref())}
            }
        };
        let content = rsx! {
            div { class: "flex flex-col gap-1",
                span { class: "text-sm font-semibold text-warning", "{law} {article}" }
                if let Some(law_name) = law_name {
                    span { class: "text-xs text-muted-foreground", "{law_name}" }
                }
                if let Some(clause) = clause {
                    hr { class: "border-border" }
                    span { class: "text-xs text-background leading-relaxed", "{clause}" }
                }
                if let Some(link) = link {
                    a {
                        href: "{link}", target: "_blank", rel: "noopener noreferrer",
                        class: "text-xs text-info hover:text-info/80 underline cursor-pointer",
                        onclick: move |event| event.stop_propagation(),
                        "View full law text"
                    }
                }
            }
        };
        rsx! {
            span { ..shared_root_attrs(props),
                TooltipFrame {
                    trigger,
                    content,
                    root_class,
                    panel_class: "p-2 rounded-lg w-64 bg-foreground text-background shadow-elevation-popover".to_string(),
                    position,
                    delay_ms: 200,
                    hide_delay_ms: 0,
                    show_arrow: true,
                    arrow_tone: TooltipTone::Foreground,
                }
            }
        }
    }
}
