//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::merge_class_name;
use crate::renderer::{DioxusPatternRenderer, DioxusView, OwnedDioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::Value;

/// The atom card root classes for `props`, shared with bodies that emit the card element around their own children.
pub(crate) fn card_class(props: &Value) -> String {
    let variant = get_string_or(props, "variant", "bordered");
    let padding = get_string_or(props, "padding", "md");
    let variant_class = match variant.as_str() {
            "elevated" => {
                "bg-card surface-material border-[length:var(--border-width)] border-border shadow transition-all duration-normal hover:shadow-elevation-dialog hover:translate-y-[var(--hover-translate-y)]"
            }
            "interactive" => {
                "bg-card surface-material border-[length:var(--border-width)] border-border shadow cursor-pointer transition-all duration-normal hover:shadow-elevation-dialog"
            }
            _ => "surface-content transition-all duration-normal hover:shadow-elevation-dialog hover:translate-y-[var(--hover-translate-y)]",
        };
    let padding_class = match padding.as_str() {
        "none" => "",
        "sm" => "p-card-sm",
        "lg" => "p-card-lg",
        _ => "p-card-md",
    };
    let look_class = match get_string_or(props, "look", "elevated").as_str() {
        "flat-bordered" => "shadow-none border-[length:var(--border-width)] border-border",
        "borderless-divider" => "shadow-none border-0 border-b border-border rounded-none",
        "ticket" => "shadow-none border-dashed border-[length:var(--border-width)] border-border",
        "invoice" => {
            "shadow-none border-[length:var(--border-width-thick)] border-border rounded-sm"
        }
        "chip" => "shadow-none rounded-pill border-[length:var(--border-width)] border-border",
        "tile-image-first" => "p-0 overflow-hidden",
        _ => "",
    };
    let shadow_class = match get_string(props, "shadow").as_deref() {
        Some("none") => "shadow-none",
        Some("sm") => "shadow-elevation-card",
        Some("md") => "shadow",
        Some("lg") => "shadow-elevation-dialog",
        _ => "",
    };
    let base = format!("rounded-container relative transition-all duration-normal chrome-panel {variant_class} {padding_class} {look_class} {shadow_class}");
    merge_class_name(&base, props)
}

/// Dioxus renderer for the `card` pattern.
pub struct CardRenderer;

impl CardRenderer {
    const NAME: &str = "card";
}

impl DioxusPatternRenderer for CardRenderer {
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
        rsx! { CardBody { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}

/// Context marker: a card already paints a surface, so content blocks inside stay flat (web `SurfaceBoundary`).
#[derive(Clone, Copy)]
pub(crate) struct OnSurface;

#[derive(Props, Clone, PartialEq)]
struct CardBodyProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn CardBody(input: CardBodyProps) -> Element {
    use_context_provider(|| OnSurface);
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let class = card_class(props);
    let title = get_string_or(props, "title", "");
    let subtitle = get_string_or(props, "subtitle", "");
    let loading = get_bool_or(props, "loading", false);
    let action = get_string(props, "action");
    let pressable = action.is_some();
    let sink = view.sink;
    let children = child_patterns(props, view.trait_content);
    rsx! {
        div { class: "{class}", role: pressable.then_some("button"), tabindex: pressable.then_some(0),
            onclick: move |_| {
                if let (Some(sink), Some(action)) = (sink, &action) {
                    sink.call((action.clone(), Value::Object(orbital_core::ValueMap::new().into())));
                }
            },
            ..shared_root_attrs(props),
            if loading { span { class: "almadar-spinner absolute inset-0" } }
            if !title.is_empty() || !subtitle.is_empty() {
                div { class: "mb-4",
                    if !title.is_empty() { h3 { class: "text-lg text-card-foreground heading-voice", "{title}" } }
                    if !subtitle.is_empty() { p { class: "text-sm text-muted-foreground mt-1", "{subtitle}" } }
                }
            }
            for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
        }
    }
}
