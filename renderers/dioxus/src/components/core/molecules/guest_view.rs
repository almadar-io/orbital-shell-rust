//! Dioxus renderer for the `guest-view` pattern: paints the guest program's children only.
//! `guest`, `source` and `deliver` are routing data the kernel reads; the renderer never shows or emits them.
//! Each guest paint is scoped to the guest trait that drew it: a control inside dispatches `<Painter>.<EVENT>` to the
//! opener's trait, which the kernel delivers as the view's `deliver` event (`kernel/guest_controls.rs`).

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink, SourceSinkFactory};
use dioxus::prelude::*;
use orbital_core::client::recurse::*;
use orbital_core::{Value, ValueMap};

/// Dioxus renderer for the `guest-view` pattern.
pub struct GuestViewRenderer;

impl GuestViewRenderer {
    const NAME: &str = "guest-view";
}

/// The event a control painted by the guest trait `painter` dispatches to the opener.
fn guest_event(painter: &str, event: &str) -> String {
    format!("{painter}.{event}")
}

/// The guest's own paints by trait (`paints[].trait` → `paints[].content`): what an `@trait.X` inside the guest's
/// content resolves to, never the host's traits.
fn guest_trait_content(props: &Value) -> ValueMap {
    let mut content = ValueMap::default();
    for paint in props.get("paints").and_then(Value::as_array).into_iter().flatten() {
        if let (Some(Value::String(name)), Some(painted)) = (paint.get("trait"), paint.get("content")) {
            if !painted.is_null() {
                content.insert(name.as_str().into(), painted.clone());
            }
        }
    }
    content
}

/// The guest's page-root trait (`source`), whose paint is `children`.
fn guest_source(props: &Value) -> Option<String> {
    match props.get("source") {
        Some(Value::String(source)) if !source.as_str().is_empty() => Some(source.as_str().to_string()),
        _ => None,
    }
}

impl DioxusPatternRenderer for GuestViewRenderer {
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
        let content = guest_trait_content(props);
        let source = guest_source(props);
        let opener = view.sink;
        let scoped = move |painter: String| -> EventSink {
            Callback::new(move |(event, payload): (String, Value)| {
                if let Some(opener) = opener {
                    opener.call((guest_event(&painter, &event), payload));
                }
            })
        };
        let root = source.clone();
        let by_painter: SourceSinkFactory = Callback::new(move |painter: Option<String>| match painter.or_else(|| root.clone()) {
            Some(painter) => scoped(painter),
            None => Callback::new(|_: (String, Value)| {}),
        });
        let guest = DioxusView {
            trait_name: view.trait_name,
            orbital_name: view.orbital_name,
            current_state: view.current_state,
            entities: view.entities,
            trait_content: &content,
            source_trait: source.clone(),
            sink: Some(by_painter.call(source.clone())),
            source_sink: by_painter,
            navigate: view.navigate,
            dock: view.dock.clone(),
            viewport_width: view.viewport_width,
            canvas_theme: view.canvas_theme.clone(),
            theme_mode: view.theme_mode,
            toggle_theme: view.toggle_theme,
        };
        let children = child_patterns(props, &content);
        rsx! {
            div { class: "bg-transparent rounded-none shadow-none w-full h-full relative overflow-auto",
                for (child, painter) in children.iter() { {guest.render_child(registry, child, painter.as_deref())} }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props() -> Value {
        serde_json::from_value(serde_json::json!({
            "type": "guest-view", "guest": "guest-1", "source": "Home", "deliver": "GUEST_CLICKED",
            "paints": [
                { "slot": "main", "trait": "Home", "orbital": "Notes", "embedded": false, "content": { "type": "stack", "children": ["@trait.Toolbar"] } },
                { "slot": "main", "trait": "Toolbar", "orbital": "Notes", "embedded": true, "content": { "type": "button", "label": "Add", "action": "ADD" } },
                { "slot": "modal", "trait": "Gone", "orbital": "Notes", "embedded": false, "content": null }
            ],
            "children": [{ "type": "stack", "children": ["@trait.Toolbar"] }]
        }))
        .unwrap()
    }

    #[test]
    fn a_guest_control_dispatches_under_the_trait_that_painted_it() {
        assert_eq!(guest_event("Toolbar", "ADD"), "Toolbar.ADD");
        assert_eq!(guest_source(&props()).as_deref(), Some("Home"), "the page-root paint is scoped to `source`");
    }

    #[test]
    fn an_embedded_trait_resolves_against_the_guests_paints_and_a_cleared_paint_is_absent() {
        let content = guest_trait_content(&props());
        assert_eq!(content.get("Toolbar").and_then(|c| c.get("action")).and_then(Value::as_str), Some("ADD"));
        assert!(content.get("Home").is_some());
        assert!(content.get("Gone").is_none(), "control: a paint the guest cleared holds no content");
        let nested = child_patterns(content.get("Home").unwrap(), &content);
        assert_eq!(nested.len(), 1);
        assert_eq!(nested[0].1.as_deref(), Some("Toolbar"), "the embed names its painter");
    }

    #[test]
    fn control_a_view_without_a_source_or_paints_scopes_nothing() {
        let bare: Value = serde_json::from_value(serde_json::json!({ "type": "guest-view", "guest": "g", "deliver": "D", "children": [] })).unwrap();
        assert_eq!(guest_source(&bare), None);
        assert!(guest_trait_content(&bare).is_empty());
    }
}
