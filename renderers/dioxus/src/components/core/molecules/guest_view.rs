//! Dioxus renderer for the `guest-view` pattern: paints the guest program's children only.
//! `guest`, `source` and `deliver` are routing data the kernel reads; the renderer never shows or emits them.
//! Each guest paint is scoped to the guest trait that drew it: a control inside dispatches `<Painter>.<EVENT>` to the
//! opener's trait, which the kernel delivers as the view's `deliver` event (`kernel/guest_controls.rs`).

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{pattern_spec, DioxusPatternRenderer, DioxusView, EventSink, SourceSinkFactory};
use dioxus::prelude::*;
use orbital_core::client::recurse::*;
use orbital_core::{slot_role, SlotRole, Value, ValueMap};

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

/// The guest's page-root traits that painted `children`, in the same order (`roots`): child N is scoped to `roots[N]`.
fn guest_roots(props: &Value) -> Vec<String> {
    props
        .get("roots")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|root| root.as_str().filter(|name| !name.is_empty()).map(str::to_string))
        .collect()
}

/// The trait child `index` is scoped to: the embed's own painter, else the page root that painted it, else `source`.
fn child_painter<'a>(painter: Option<&'a str>, roots: &'a [String], index: usize) -> Option<&'a str> {
    painter.or_else(|| roots.get(index).map(String::as_str))
}

/// Whether a paint is a notification (the toast slot): an embedded trait's render there still paints in the slot.
fn paints_in_slot(slot: &str, embedded: bool) -> bool {
    !embedded || slot == "toast"
}

/// The classes of the contained frame the web's `UISlotRenderer` gives `slot` inside a guest view, `None` for a slot
/// the view paints no frame for (the main slot is the view's children).
fn slot_frame(slot: &str) -> Option<&'static str> {
    match (slot_role(slot), slot) {
        (SlotRole::Modal, "modal") | (SlotRole::Drawer, "drawer") => Some("contents"),
        (SlotRole::Toast, _) => Some("absolute top-4 right-4 z-50"),
        (SlotRole::Overlay, _) => Some("absolute inset-0 z-50 flex items-center justify-center overflow-auto bg-scrim"),
        (SlotRole::HudTop, _) => Some("absolute top-0 left-0 right-0 z-40"),
        (SlotRole::HudBottom, _) => Some("absolute bottom-0 left-0 right-0 z-40"),
        _ => None,
    }
}

/// The pattern a slot paint is drawn as: a self-overlaying modal or confirm dialog brings its own chrome, any other
/// modal content sits in a contained modal, any other drawer content in a contained drawer.
fn slot_spec(slot: &str, content: &Value) -> Value {
    let kind = content.get("type").and_then(Value::as_str).unwrap_or_default();
    let wrapped = |name: &str, fields: Vec<(&str, Value)>| {
        let mut spec = vec![("isOpen", Value::Bool(true)), ("contained", Value::Bool(true)), ("children", Value::Array(vec![content.clone()].into()))];
        spec.extend(fields.into_iter().filter(|(_, value)| !value.is_null()));
        pattern_spec(name, spec)
    };
    let field = |name: &str| content.get(name).cloned().unwrap_or(Value::Null);
    match slot {
        "modal" if matches!(kind, "modal" | "confirm-dialog") => {
            let mut spec = content.clone();
            if let Value::Object(fields) = &mut spec {
                fields.insert("contained".into(), Value::Bool(true));
            }
            spec
        }
        "modal" => wrapped("modal", vec![("title", field("title")), ("size", field("size"))]),
        "drawer" => wrapped("drawer", vec![("title", field("title")), ("position", field("position")), ("width", field("width"))]),
        _ => content.clone(),
    }
}

/// The guest's paints outside the main slot, in the order the guest painted them: `(slot, painter, content)`.
fn slot_paints(props: &Value) -> Vec<(String, String, Value)> {
    props
        .get("paints")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|paint| {
            let slot = paint.get("slot").and_then(Value::as_str)?;
            let painter = paint.get("trait").and_then(Value::as_str)?;
            let content = paint.get("content").filter(|content| !content.is_null())?;
            let embedded = matches!(paint.get("embedded"), Some(Value::Bool(true)));
            (slot_frame(slot).is_some() && paints_in_slot(slot, embedded)).then(|| (slot.to_string(), painter.to_string(), content.clone()))
        })
        .collect()
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
        let roots = guest_roots(props);
        let slots = slot_paints(props);
        rsx! {
            div { class: "bg-transparent rounded-none shadow-none w-full h-full relative", "data-guest-frame": "",
                div { class: "w-full h-full relative overflow-auto",
                    for (index, (child, painter)) in children.iter().enumerate() { {guest.render_child(registry, child, child_painter(painter.as_deref(), &roots, index))} }
                }
                for (slot, painter, painted) in slots.iter() {
                    div { key: "{slot}:{painter}", id: "slot-{slot}", class: "ui-slot ui-slot-{slot} {slot_frame(slot).unwrap_or_default()}",
                        {match painted {
                            Value::String(_) => guest.for_trait_source(painter.clone()).render_node(registry, painted),
                            _ => registry.render_spec(&slot_spec(slot, painted), None, &guest.for_trait_source(painter.clone())),
                        }}
                    }
                }
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
    fn each_page_root_child_is_scoped_to_its_own_root() {
        let two: Value = serde_json::from_value(serde_json::json!({
            "type": "guest-view", "guest": "g", "source": "One", "roots": ["One", "Two"], "deliver": "D",
            "children": [{ "type": "button", "action": "A" }, { "type": "button", "action": "B" }]
        }))
        .unwrap();
        let roots = guest_roots(&two);
        assert_eq!(child_painter(None, &roots, 0), Some("One"));
        assert_eq!(child_painter(None, &roots, 1), Some("Two"), "the second root's controls dispatch under the second root");
        assert_eq!(child_painter(Some("Toolbar"), &roots, 1), Some("Toolbar"), "an embed keeps its own painter");
        assert_eq!(child_painter(None, &guest_roots(&props()), 0), None, "control: without `roots` the child falls to `source`");
    }

    #[test]
    fn control_a_view_without_a_source_or_paints_scopes_nothing() {
        let bare: Value = serde_json::from_value(serde_json::json!({ "type": "guest-view", "guest": "g", "deliver": "D", "children": [] })).unwrap();
        assert_eq!(guest_source(&bare), None);
        assert!(guest_trait_content(&bare).is_empty());
    }
}

#[cfg(all(test, feature = "native", not(target_arch = "wasm32")))]
mod dom_tests {
    use super::*;
    use blitz_dom::{Document as _, DocumentConfig};
    use blitz_traits::events::{BlitzMouseButtonEvent, MouseEventButton, MouseEventButtons, UiEvent};
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus_native::DioxusDocument;
    use keyboard_types::Modifiers;
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static EVENTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
        static DROPPED: std::cell::Cell<&'static [&'static str]> = const { std::cell::Cell::new(&[]) };
    }

    fn page() -> Value {
        let dropped = DROPPED.with(std::cell::Cell::get);
        let mut value = serde_json::json!({
            "type": "stack",
            "children": [
                { "type": "button", "label": "Host", "aria-label": "host-button", "action": "HOST" },
                {
                    "type": "guest-view", "guest": "guest-1", "source": "Home", "deliver": "GUEST_CLICKED",
                    "paints": [
                        { "slot": "main", "trait": "Home", "orbital": "Notes", "embedded": false, "content": { "type": "button", "label": "Add", "aria-label": "guest-add", "action": "ADD" } },
                        { "slot": "modal", "trait": "Confirm", "orbital": "Notes", "embedded": false, "content": { "type": "button", "label": "Sure", "aria-label": "guest-sure", "action": "SURE" } },
                        { "slot": "hud-top", "trait": "Hud", "orbital": "Notes", "embedded": false, "content": { "type": "button", "label": "Pause", "aria-label": "guest-pause", "action": "PAUSE" } },
                        { "slot": "toast", "trait": "Notify", "orbital": "Notes", "embedded": false, "content": { "type": "typography", "text": "Saved" } },
                        { "slot": "drawer", "trait": "Gone", "orbital": "Notes", "embedded": false, "content": null },
                        { "slot": "modal", "trait": "Inline", "orbital": "Notes", "embedded": true, "content": { "type": "typography", "text": "Inline only" } }
                    ],
                    "children": [{ "type": "button", "label": "Add", "aria-label": "guest-add", "action": "ADD" }]
                }
            ]
        });
        let paints = value.pointer_mut("/children/1/paints").and_then(serde_json::Value::as_array_mut).unwrap();
        paints.retain(|paint| !dropped.iter().any(|slot| paint["slot"] == *slot));
        serde_json::from_value(value).unwrap()
    }

    fn app() -> Element {
        let props = page();
        let entities = HashMap::new();
        let trait_content = ValueMap::new();
        let sink = Callback::new(|(event, _): (String, Value)| EVENTS.with(|events| events.borrow_mut().push(event)));
        let view = DioxusView {
            trait_name: "Opener",
            orbital_name: "Opener",
            current_state: "idle",
            entities: &entities,
            trait_content: &trait_content,
            source_trait: None,
            sink: Some(sink),
            source_sink: Callback::new(move |_| sink),
            navigate: None,
            dock: None,
            viewport_width: None,
            canvas_theme: None,
            theme_mode: None,
            toggle_theme: None,
        };
        crate::registry::default_registry().render_spec(&props, None, &view)
    }

    fn settle(doc: &mut DioxusDocument) {
        for _ in 0..4 {
            if !doc.poll(None) {
                break;
            }
        }
        doc.resolve(0.0);
    }

    fn document() -> DioxusDocument {
        EVENTS.with(|events| events.borrow_mut().clear());
        let mut doc = DioxusDocument::new(
            dioxus_core::VirtualDom::new(app),
            DocumentConfig { viewport: Some(Viewport::new(800, 600, 1.0, ColorScheme::Light)), ..Default::default() },
        );
        for css in crate::styles::stylesheets() {
            doc.add_user_agent_stylesheet(&css);
        }
        doc.initial_build();
        settle(&mut doc);
        doc
    }

    fn mouse(x: f32, y: f32, buttons: MouseEventButtons) -> BlitzMouseButtonEvent {
        BlitzMouseButtonEvent { x, y, button: MouseEventButton::Main, buttons, mods: Modifiers::default() }
    }

    fn click(doc: &mut DioxusDocument, selector: &str) -> Vec<String> {
        EVENTS.with(|events| events.borrow_mut().clear());
        let id = doc.query_selector(selector).unwrap().unwrap_or_else(|| panic!("{selector} is painted"));
        let at = doc.get_node(id).unwrap().synthetic_click_event_data(Modifiers::default());
        doc.handle_ui_event(UiEvent::MouseMove(mouse(at.x, at.y, MouseEventButtons::None)));
        doc.handle_ui_event(UiEvent::MouseDown(mouse(at.x, at.y, MouseEventButtons::Primary)));
        doc.handle_ui_event(UiEvent::MouseUp(mouse(at.x, at.y, MouseEventButtons::None)));
        settle(doc);
        EVENTS.with(|events| events.borrow().clone())
    }

    fn painted(doc: &DioxusDocument, selector: &str) -> bool {
        doc.query_selector(selector).unwrap().is_some()
    }

    #[test]
    fn the_guests_modal_and_toast_paint_inside_the_view() {
        let _guard = crate::EVENT_CONVERTER_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let doc = document();
        assert!(painted(&doc, "[data-guest-frame] #slot-modal [aria-label=\"guest-sure\"]"), "the guest's modal paint sits inside the view");
        assert!(painted(&doc, "[data-guest-frame] #slot-toast"), "the guest's toast paint sits inside the view");
        assert!(!painted(&doc, "#slot-drawer"), "control: a slot the guest cleared paints nothing");
        assert!(!doc_text_has(&doc, "Inline only"), "control: an embedded trait's modal paint shows only where its `@trait` embed names it");
    }

    fn doc_text_has(doc: &DioxusDocument, needle: &str) -> bool {
        doc.root_element().text_content().contains(needle)
    }

    #[test]
    fn a_click_on_a_guest_control_reaches_the_opener_scoped_to_its_painter() {
        let _guard = crate::EVENT_CONVERTER_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        DROPPED.with(|dropped| dropped.set(&["modal", "hud-top"]));
        let mut closed = document();
        assert_eq!(click(&mut closed, "[data-guest-frame] [aria-label=\"guest-add\"]"), vec!["Home.ADD".to_string()]);
        assert_eq!(click(&mut closed, "[aria-label=\"host-button\"]"), vec!["HOST".to_string()], "control: a host button outside the view dispatches the host's own event");
        DROPPED.with(|dropped| dropped.set(&["modal"]));
        let mut hud = document();
        assert_eq!(click(&mut hud, "#slot-hud-top [aria-label=\"guest-pause\"]"), vec!["Hud.PAUSE".to_string()], "a slot paint is scoped to the trait that painted it");
    }
}
