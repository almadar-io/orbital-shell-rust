//! A scene canvas's side of the accessibility and input-method channel (Rasm floor contract
//! sections 10 and 12): the [`SceneHost`]'s semantic tree out, AccessKit actions, committed and
//! preedit text and editing keys in. The window (`blitz-shell`) finds the host by the canvas's
//! paint-source id.

use blitz_traits::canvas_access::{
    CanvasAccessAction, CanvasAccessHost, CanvasAccessNode, CanvasKeyMods,
};
use orbital_canvas::mappers::scene::SemanticNode;
use orbital_canvas::{AccessAction, KeyMods, SceneHost};
use std::sync::{Arc, Mutex};

pub(crate) struct SceneAccess {
    host: Arc<Mutex<SceneHost>>,
}

impl SceneAccess {
    pub(crate) fn new(host: Arc<Mutex<SceneHost>>) -> Self {
        Self { host }
    }
}

fn access_node(node: &SemanticNode) -> CanvasAccessNode {
    CanvasAccessNode {
        id: node.id.clone(),
        role: node.role.clone(),
        name: node.name.clone(),
        value: node.value.clone(),
        checked: node.checked,
        disabled: node.disabled,
        focused: node.focused,
        multiline: node.multiline,
        order: node.order,
        x: node.rect.x,
        y: node.rect.y,
        width: node.rect.w,
        height: node.rect.h,
    }
}

fn access_action(action: CanvasAccessAction) -> AccessAction {
    match action {
        CanvasAccessAction::Focus => AccessAction::Focus,
        CanvasAccessAction::Activate => AccessAction::Activate,
        CanvasAccessAction::Increment => AccessAction::Increment,
        CanvasAccessAction::Decrement => AccessAction::Decrement,
    }
}

impl CanvasAccessHost for SceneAccess {
    fn nodes(&self) -> Vec<CanvasAccessNode> {
        let host = self.host.lock().expect("scene host poisoned");
        host.semantic_tree().iter().map(access_node).collect()
    }

    fn access_action(&self, target: &str, action: CanvasAccessAction) {
        let mut host = self.host.lock().expect("scene host poisoned");
        host.access_action(target, access_action(action));
    }

    fn text_input(&self, target: &str, text: &str, composing: bool) {
        let mut host = self.host.lock().expect("scene host poisoned");
        host.text_input(target, text, composing);
    }

    fn key_input(&self, target: &str, key: &str, mods: CanvasKeyMods) {
        let mut host = self.host.lock().expect("scene host poisoned");
        host.key_input(
            target,
            key,
            KeyMods {
                shift: mods.shift,
                alt: mods.alt,
                ctrl: mods.ctrl,
                meta: mods.meta,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbital_canvas::packet::Rect;

    fn semantic() -> SemanticNode {
        SemanticNode {
            id: "name".into(),
            role: "textbox".into(),
            name: "Name".into(),
            value: Some("Ada".into()),
            checked: None,
            disabled: false,
            focused: true,
            caret: Some(3),
            selection_start: None,
            selection_end: None,
            multiline: true,
            order: 2.0,
            rect: Rect {
                x: 4.0,
                y: 5.0,
                w: 60.0,
                h: 20.0,
            },
        }
    }

    #[test]
    fn a_semantic_node_maps_field_for_field() {
        let node = access_node(&semantic());
        assert_eq!(
            (node.id.as_str(), node.role.as_str(), node.name.as_str()),
            ("name", "textbox", "Name")
        );
        assert_eq!(node.value.as_deref(), Some("Ada"));
        assert!(node.focused && node.multiline && !node.disabled);
        assert_eq!(
            (node.x, node.y, node.width, node.height),
            (4.0, 5.0, 60.0, 20.0)
        );
        assert_eq!(node.order, 2.0);
    }

    #[test]
    fn control_an_unfocused_checked_node_keeps_its_state() {
        let mut source = semantic();
        source.focused = false;
        source.disabled = true;
        source.checked = Some(false);
        let node = access_node(&source);
        assert!(!node.focused && node.disabled);
        assert_eq!(node.checked, Some(false));
    }

    #[test]
    fn each_canvas_action_maps_to_its_scene_host_action() {
        assert_eq!(
            access_action(CanvasAccessAction::Focus),
            AccessAction::Focus
        );
        assert_eq!(
            access_action(CanvasAccessAction::Activate),
            AccessAction::Activate
        );
        assert_eq!(
            access_action(CanvasAccessAction::Increment),
            AccessAction::Increment
        );
        assert_eq!(
            access_action(CanvasAccessAction::Decrement),
            AccessAction::Decrement
        );
    }
}
