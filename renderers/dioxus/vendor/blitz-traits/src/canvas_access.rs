//! ORBITAL PATCH: the side channel through which a scene canvas contributes accessibility nodes and
//! receives assistive-technology actions, input-method text and editing keys (Rasm floor contract
//! sections 10 and 12). A canvas registers its host under its paint-source id, the `src` of its
//! `canvas` element.

use crate::events::{BlitzImeEvent, BlitzKeyEvent, KeyState};
use keyboard_types::{Key, Modifiers};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasAccessAction {
    Focus,
    Activate,
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CanvasKeyMods {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub meta: bool,
}

/// One node of a canvas's semantic tree. `x`, `y`, `width`, `height` are the node's rect in the
/// canvas element's CSS pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasAccessNode {
    pub id: String,
    pub role: String,
    pub name: String,
    pub value: Option<String>,
    pub checked: Option<bool>,
    pub disabled: bool,
    pub focused: bool,
    pub multiline: bool,
    pub order: f32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub trait CanvasAccessHost: Send + Sync {
    fn nodes(&self) -> Vec<CanvasAccessNode>;
    fn access_action(&self, target: &str, action: CanvasAccessAction);
    fn text_input(&self, target: &str, text: &str, composing: bool);
    fn key_input(&self, target: &str, key: &str, mods: CanvasKeyMods);
}

type Registry = Mutex<HashMap<u64, Arc<dyn CanvasAccessHost>>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}

pub fn register_canvas_access(scene_id: u64, host: Arc<dyn CanvasAccessHost>) {
    registry()
        .lock()
        .expect("canvas access registry poisoned")
        .insert(scene_id, host);
}

pub fn unregister_canvas_access(scene_id: u64) {
    registry()
        .lock()
        .expect("canvas access registry poisoned")
        .remove(&scene_id);
}

pub fn canvas_access_host(scene_id: u64) -> Option<Arc<dyn CanvasAccessHost>> {
    registry()
        .lock()
        .expect("canvas access registry poisoned")
        .get(&scene_id)
        .cloned()
}

/// What the focused semantic node takes from the keyboard: a `textbox` takes input-method text and
/// editing keys, a `combobox` takes every key but Tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasFocusKind {
    Textbox,
    Combobox,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasFocus {
    pub target: String,
    pub kind: CanvasFocusKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// The program's focus in the tree: the first focused node that takes keyboard input.
pub fn canvas_focus(nodes: &[CanvasAccessNode]) -> Option<CanvasFocus> {
    let node = nodes.iter().find(|node| node.focused && !node.disabled)?;
    let kind = match node.role.as_str() {
        "textbox" => CanvasFocusKind::Textbox,
        "combobox" => CanvasFocusKind::Combobox,
        _ => return None,
    };
    Some(CanvasFocus {
        target: node.id.clone(),
        kind,
        x: node.x,
        y: node.y,
        width: node.width,
        height: node.height,
    })
}

/// Input-method events for the focused text box. Returns whether the canvas took the event.
pub fn route_ime(host: &dyn CanvasAccessHost, focus: &CanvasFocus, event: &BlitzImeEvent) -> bool {
    if focus.kind != CanvasFocusKind::Textbox {
        return false;
    }
    match event {
        BlitzImeEvent::Preedit(text, _) => host.text_input(&focus.target, text, true),
        BlitzImeEvent::Commit(text) => host.text_input(&focus.target, text, false),
        BlitzImeEvent::Enabled | BlitzImeEvent::Disabled | BlitzImeEvent::TextState { .. } => {}
    }
    true
}

/// A key press for the focused node. A text box gets named keys and shortcuts (typed text arrives as
/// input-method commits); a combobox gets named keys except Tab and printable characters. Returns
/// whether the canvas took the event.
pub fn route_key(host: &dyn CanvasAccessHost, focus: &CanvasFocus, event: &BlitzKeyEvent) -> bool {
    if event.state != KeyState::Pressed || event.is_composing {
        return false;
    }
    let shortcut = event
        .modifiers
        .intersects(Modifiers::CONTROL | Modifiers::META);
    let forward = match (&event.key, focus.kind) {
        (
            Key::Process
            | Key::Dead
            | Key::Unidentified
            | Key::Alt
            | Key::AltGraph
            | Key::CapsLock
            | Key::Control
            | Key::Fn
            | Key::FnLock
            | Key::Meta
            | Key::NumLock
            | Key::ScrollLock
            | Key::Shift
            | Key::Super
            | Key::Hyper
            | Key::Symbol
            | Key::SymbolLock,
            _,
        ) => false,
        (Key::Tab, CanvasFocusKind::Combobox) => false,
        (Key::Character(_), CanvasFocusKind::Textbox) => shortcut,
        _ => true,
    };
    if !forward {
        return false;
    }
    let mods = CanvasKeyMods {
        shift: event.modifiers.contains(Modifiers::SHIFT),
        alt: event.modifiers.contains(Modifiers::ALT),
        ctrl: event.modifiers.contains(Modifiers::CONTROL),
        meta: event.modifiers.contains(Modifiers::META),
    };
    host.key_input(&focus.target, &event.key.to_string(), mods);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyboard_types::{Code, Location};

    #[derive(Default)]
    struct Recorder(Mutex<Vec<String>>);

    impl CanvasAccessHost for Recorder {
        fn nodes(&self) -> Vec<CanvasAccessNode> {
            Vec::new()
        }
        fn access_action(&self, target: &str, action: CanvasAccessAction) {
            self.0
                .lock()
                .unwrap()
                .push(format!("action {target} {action:?}"));
        }
        fn text_input(&self, target: &str, text: &str, composing: bool) {
            self.0
                .lock()
                .unwrap()
                .push(format!("text {target} {text:?} {composing}"));
        }
        fn key_input(&self, target: &str, key: &str, mods: CanvasKeyMods) {
            self.0
                .lock()
                .unwrap()
                .push(format!("key {target} {key} {mods:?}"));
        }
    }

    fn node(id: &str, role: &str, focused: bool) -> CanvasAccessNode {
        CanvasAccessNode {
            id: id.into(),
            role: role.into(),
            name: id.into(),
            value: None,
            checked: None,
            disabled: false,
            focused,
            multiline: false,
            order: 0.0,
            x: 1.0,
            y: 2.0,
            width: 30.0,
            height: 10.0,
        }
    }

    fn focus(kind: CanvasFocusKind) -> CanvasFocus {
        CanvasFocus {
            target: "f".into(),
            kind,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }

    fn key(key: Key, modifiers: Modifiers) -> BlitzKeyEvent {
        BlitzKeyEvent {
            key,
            code: Code::Unidentified,
            modifiers,
            location: Location::Standard,
            is_auto_repeating: false,
            is_composing: false,
            state: KeyState::Pressed,
            text: None,
        }
    }

    #[test]
    fn a_focused_textbox_is_the_canvas_focus() {
        let nodes = [node("a", "button", false), node("f", "textbox", true)];
        let found = canvas_focus(&nodes).unwrap();
        assert_eq!(
            (found.target.as_str(), found.kind),
            ("f", CanvasFocusKind::Textbox)
        );
        assert_eq!(
            (found.x, found.y, found.width, found.height),
            (1.0, 2.0, 30.0, 10.0)
        );
    }

    #[test]
    fn control_a_focused_button_or_an_unfocused_textbox_takes_no_keyboard() {
        assert!(canvas_focus(&[node("b", "button", true)]).is_none());
        assert!(canvas_focus(&[node("f", "textbox", false)]).is_none());
        let mut disabled = node("f", "textbox", true);
        disabled.disabled = true;
        assert!(canvas_focus(&[disabled]).is_none());
    }

    #[test]
    fn preedit_and_commit_reach_the_text_box_as_composing_and_committed_text() {
        let host = Recorder::default();
        let f = focus(CanvasFocusKind::Textbox);
        assert!(route_ime(
            &host,
            &f,
            &BlitzImeEvent::Preedit("に".into(), Some((0, 3)))
        ));
        assert!(route_ime(
            &host,
            &f,
            &BlitzImeEvent::Preedit(String::new(), None)
        ));
        assert!(route_ime(&host, &f, &BlitzImeEvent::Commit("日".into())));
        assert_eq!(
            *host.0.lock().unwrap(),
            vec![
                "text f \"に\" true",
                "text f \"\" true",
                "text f \"日\" false"
            ]
        );
    }

    #[test]
    fn control_ime_enable_and_disable_are_taken_without_a_call_and_a_combobox_leaves_ime_alone() {
        let host = Recorder::default();
        assert!(route_ime(
            &host,
            &focus(CanvasFocusKind::Textbox),
            &BlitzImeEvent::Enabled
        ));
        assert!(route_ime(
            &host,
            &focus(CanvasFocusKind::Textbox),
            &BlitzImeEvent::Disabled
        ));
        assert!(!route_ime(
            &host,
            &focus(CanvasFocusKind::Combobox),
            &BlitzImeEvent::Commit("x".into())
        ));
        assert!(host.0.lock().unwrap().is_empty());
    }

    #[test]
    fn a_textbox_gets_named_keys_and_shortcuts_with_their_modifiers() {
        let host = Recorder::default();
        let f = focus(CanvasFocusKind::Textbox);
        assert!(route_key(
            &host,
            &f,
            &key(Key::Backspace, Modifiers::empty())
        ));
        assert!(route_key(&host, &f, &key(Key::Tab, Modifiers::SHIFT)));
        assert!(route_key(
            &host,
            &f,
            &key(Key::Character("a".into()), Modifiers::META)
        ));
        assert_eq!(
            *host.0.lock().unwrap(),
            vec![
                "key f Backspace CanvasKeyMods { shift: false, alt: false, ctrl: false, meta: false }",
                "key f Tab CanvasKeyMods { shift: true, alt: false, ctrl: false, meta: false }",
                "key f a CanvasKeyMods { shift: false, alt: false, ctrl: false, meta: true }",
            ]
        );
    }

    #[test]
    fn control_a_textbox_never_gets_typed_characters_composition_or_releases() {
        let host = Recorder::default();
        let f = focus(CanvasFocusKind::Textbox);
        assert!(!route_key(
            &host,
            &f,
            &key(Key::Character("a".into()), Modifiers::SHIFT)
        ));
        assert!(!route_key(
            &host,
            &f,
            &key(Key::Process, Modifiers::empty())
        ));
        let mut composing = key(Key::Backspace, Modifiers::empty());
        composing.is_composing = true;
        assert!(!route_key(&host, &f, &composing));
        let mut released = key(Key::Backspace, Modifiers::empty());
        released.state = KeyState::Released;
        assert!(!route_key(&host, &f, &released));
        assert!(host.0.lock().unwrap().is_empty());
    }

    #[test]
    fn a_combobox_gets_printable_and_named_keys_but_not_tab() {
        let host = Recorder::default();
        let f = focus(CanvasFocusKind::Combobox);
        assert!(route_key(
            &host,
            &f,
            &key(Key::Character("b".into()), Modifiers::empty())
        ));
        assert!(route_key(
            &host,
            &f,
            &key(Key::ArrowDown, Modifiers::empty())
        ));
        assert!(!route_key(&host, &f, &key(Key::Tab, Modifiers::empty())));
        assert_eq!(host.0.lock().unwrap().len(), 2);
    }

    #[test]
    fn the_registry_hands_back_the_host_registered_under_the_scene_id() {
        register_canvas_access(9_000_001, Arc::new(Recorder::default()));
        assert!(canvas_access_host(9_000_001).is_some());
        assert!(canvas_access_host(9_000_002).is_none());
        unregister_canvas_access(9_000_001);
        assert!(canvas_access_host(9_000_001).is_none());
    }
}
