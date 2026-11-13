use crate::{BaseDocument, Node as BlitzDomNode, local_name};
use accesskit::{
    Action, Node as AccessKitNode, NodeId, Rect, Role, Toggled, TreeId, TreeInfo, TreeUpdate,
};
use blitz_traits::canvas_access::{CanvasAccessAction, CanvasAccessNode, canvas_access_host};

/// ORBITAL PATCH: virtual node ids of a scene canvas live above every DOM node id:
/// `VIRTUAL_BASE | canvas dom id << 28 | index in the canvas's tree`.
const VIRTUAL_BASE: u64 = 1 << 62;
const VIRTUAL_INDEX_BITS: u32 = 28;

/// What a virtual accessibility node stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CanvasAccessTarget {
    pub(crate) canvas_dom_id: usize,
    pub(crate) scene_id: u64,
    pub(crate) target: String,
}

pub(crate) struct CanvasAccessTree {
    pub(crate) nodes: Vec<(NodeId, AccessKitNode)>,
    pub(crate) targets: Vec<(u64, CanvasAccessTarget)>,
    pub(crate) focus: Option<NodeId>,
}

fn virtual_id(canvas_dom_id: usize, index: usize) -> NodeId {
    NodeId(VIRTUAL_BASE | (canvas_dom_id as u64) << VIRTUAL_INDEX_BITS | index as u64)
}

/// The declared role table: a semantic role names one AccessKit role, an unlisted role is `Unknown`.
fn canvas_role(role: &str, multiline: bool) -> Role {
    match role {
        "button" => Role::Button,
        "textbox" if multiline => Role::MultilineTextInput,
        "textbox" => Role::TextInput,
        "combobox" => Role::ComboBox,
        "option" => Role::ListBoxOption,
        "listbox" => Role::ListBox,
        "checkbox" => Role::CheckBox,
        "switch" => Role::Switch,
        "radio" => Role::RadioButton,
        "slider" => Role::Slider,
        "spinbutton" => Role::SpinButton,
        "tab" => Role::Tab,
        "link" => Role::Link,
        "menuitem" => Role::MenuItem,
        "heading" => Role::Heading,
        "img" => Role::Image,
        "group" => Role::Group,
        "label" | "text" => Role::Label,
        _ => Role::Unknown,
    }
}

/// The AccessKit actions a virtual node of that role accepts.
fn canvas_actions(role: &str, disabled: bool) -> Vec<Action> {
    if disabled {
        return Vec::new();
    }
    let mut actions = vec![Action::Focus];
    if matches!(
        role,
        "button"
            | "combobox"
            | "option"
            | "checkbox"
            | "switch"
            | "radio"
            | "tab"
            | "link"
            | "menuitem"
    ) {
        actions.push(Action::Click);
    }
    if matches!(role, "slider" | "spinbutton") {
        actions.push(Action::Increment);
        actions.push(Action::Decrement);
    }
    actions
}

/// The AccessKit nodes of a canvas's semantic tree, laid under the canvas element at `origin`
/// (physical pixels) and `scale`, and the id each stands for.
pub(crate) fn canvas_access_tree(
    nodes: &[CanvasAccessNode],
    canvas_dom_id: usize,
    scene_id: u64,
    origin: (f64, f64),
    scale: f64,
) -> CanvasAccessTree {
    let mut tree = CanvasAccessTree {
        nodes: Vec::new(),
        targets: Vec::new(),
        focus: None,
    };
    for (index, node) in nodes.iter().enumerate() {
        let id = virtual_id(canvas_dom_id, index);
        let mut builder = AccessKitNode::new(canvas_role(&node.role, node.multiline));
        builder.set_label(node.name.clone());
        if let Some(value) = &node.value {
            builder.set_value(value.clone());
        }
        if let Some(checked) = node.checked {
            builder.set_toggled(checked.into());
        }
        if node.disabled {
            builder.set_disabled();
        }
        let x = origin.0 + node.x as f64 * scale;
        let y = origin.1 + node.y as f64 * scale;
        builder.set_bounds(Rect {
            x0: x,
            y0: y,
            x1: x + node.width as f64 * scale,
            y1: y + node.height as f64 * scale,
        });
        for action in canvas_actions(&node.role, node.disabled) {
            builder.add_action(action);
        }
        if node.focused && tree.focus.is_none() {
            tree.focus = Some(id);
        }
        tree.targets.push((
            id.0,
            CanvasAccessTarget {
                canvas_dom_id,
                scene_id,
                target: node.id.clone(),
            },
        ));
        tree.nodes.push((id, builder));
    }
    tree
}

/// The canvas action an AccessKit action asks for, if the contract has one.
pub fn canvas_access_action(action: Action) -> Option<CanvasAccessAction> {
    match action {
        Action::Focus => Some(CanvasAccessAction::Focus),
        Action::Click => Some(CanvasAccessAction::Activate),
        Action::Increment => Some(CanvasAccessAction::Increment),
        Action::Decrement => Some(CanvasAccessAction::Decrement),
        _ => None,
    }
}

impl BaseDocument {
    pub fn build_accessibility_tree(&self) -> TreeUpdate {
        let mut nodes = std::collections::HashMap::new();
        let mut window = AccessKitNode::new(Role::Window);
        let mut canvas_nodes = Vec::new();
        let mut canvas_targets = std::collections::HashMap::new();
        let mut canvas_focus = None;

        self.visit(|node_id, node| {
            let parent = node
                .parent
                .and_then(|parent_id| nodes.get_mut(&parent_id))
                .map(|(_, parent)| parent)
                .unwrap_or(&mut window);
            let (id, mut builder) = self.build_accessibility_node(node, parent);

            if let Some((canvas_id, scene_id)) = self.canvas_scene_of(node_id) {
                if let (Some(host), Some((x, y))) =
                    (canvas_access_host(scene_id), self.node_origin(node_id))
                {
                    let scale = self.viewport.scale() as f64;
                    let tree = canvas_access_tree(
                        &host.nodes(),
                        canvas_id,
                        scene_id,
                        (x as f64 * scale, y as f64 * scale),
                        scale,
                    );
                    for (child, _) in &tree.nodes {
                        builder.push_child(*child);
                    }
                    if self.focus_node_id == Some(node_id) && tree.focus.is_some() {
                        canvas_focus = tree.focus;
                    }
                    canvas_targets.extend(tree.targets);
                    canvas_nodes.extend(tree.nodes);
                }
            }

            nodes.insert(node_id, (id, builder));
        });

        *self
            .canvas_access_targets
            .lock()
            .expect("canvas access targets poisoned") = canvas_targets;

        let mut nodes: Vec<_> = nodes
            .into_iter()
            .map(|(_, (id, node))| (id, node))
            .collect();
        nodes.extend(canvas_nodes);
        nodes.push((NodeId(u64::MAX), window));

        let tree = TreeInfo::new(NodeId(u64::MAX));
        TreeUpdate {
            nodes,
            tree: Some(tree),
            tree_id: TreeId::ROOT,
            focus: canvas_focus.unwrap_or_else(|| {
                NodeId(self.focus_node_id.map(|id| id as u64).unwrap_or(u64::MAX))
            }),
        }
    }

    /// What a virtual accessibility node of the last tree stands for.
    pub fn canvas_access_target(&self, node: NodeId) -> Option<(usize, u64, String)> {
        let targets = self
            .canvas_access_targets
            .lock()
            .expect("canvas access targets poisoned");
        let found = targets.get(&node.0)?;
        Some((found.canvas_dom_id, found.scene_id, found.target.clone()))
    }

    fn canvas_scene_of(&self, node_id: usize) -> Option<(usize, u64)> {
        let element = self.get_node(node_id)?.element_data()?;
        if element.name.local != local_name!("canvas") {
            return None;
        }
        Some((node_id, element.attr(local_name!("src"))?.parse().ok()?))
    }

    fn build_accessibility_node(
        &self,
        node: &BlitzDomNode,
        parent: &mut AccessKitNode,
    ) -> (NodeId, AccessKitNode) {
        let id = NodeId(node.id as u64);

        let mut builder = AccessKitNode::default();
        let position = node.absolute_position(0.0, 0.0);
        let size = node.final_layout.size;
        if size.width > 0.0 && size.height > 0.0 {
            let scroll = self.viewport_scroll();
            let scale = self.viewport.scale() as f64;
            let x = (position.x as f64 - scroll.x) * scale;
            let y = (position.y as f64 - scroll.y) * scale;
            builder.set_bounds(Rect {
                x0: x,
                y0: y,
                x1: x + size.width as f64 * scale,
                y1: y + size.height as f64 * scale,
            });
        }
        if node.id == 0 {
            builder.set_role(Role::Window)
        } else if let Some(element_data) = node.element_data() {
            let name = element_data.name.local.to_string();

            // TODO match more roles
            let role = match element_data.attr(local_name!("role")).unwrap_or(&name) {
                "button" => Role::Button,
                "checkbox" => Role::CheckBox,
                "div" => Role::GenericContainer,
                "header" => Role::Header,
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Role::Heading,
                "p" => Role::Paragraph,
                "span" => Role::Paragraph,
                "section" => Role::Section,
                "input" if element_data.has_text_input_type() => {
                    let ty = element_data.attr(local_name!("type")).unwrap_or("text");
                    match ty {
                        "number" => Role::NumberInput,
                        _ => Role::TextInput,
                    }
                }
                "input" => Role::CheckBox,
                _ => Role::Unknown,
            };

            builder.set_role(role);
            if element_data.attr(local_name!("aria-hidden")) == Some("true") {
                builder.set_hidden();
            }
            builder.set_html_tag(name);
            if let Some(label) = element_data.attr(local_name!("aria-label")) {
                builder.set_label(label);
            } else if matches!(
                role,
                Role::Button | Role::CheckBox | Role::Heading | Role::Paragraph
            ) {
                builder.set_label(node.text_content());
            }
            if element_data.attr(local_name!("disabled")).is_some() {
                builder.set_disabled();
            }
            if role == Role::TextInput {
                if let Some(input) = element_data.text_input_data() {
                    builder.set_value(input.editor.raw_text());
                }
            }
            if role == Role::CheckBox {
                let toggled = match element_data.attr(local_name!("aria-checked")) {
                    Some("true") => Toggled::True,
                    Some("mixed") => Toggled::Mixed,
                    Some(_) => Toggled::False,
                    None => element_data
                        .checkbox_input_checked()
                        .unwrap_or_else(|| element_data.attr(local_name!("checked")).is_some())
                        .into(),
                };
                builder.set_toggled(toggled);
            }
            if matches!(role, Role::Button | Role::CheckBox) {
                builder.add_action(Action::Click);
                if element_data.attr(local_name!("disabled")).is_none() {
                    builder.add_action(Action::Focus);
                }
            } else if role == Role::TextInput {
                builder.add_action(Action::Focus);
            }
        } else if node.is_text_node() {
            builder.set_role(Role::TextRun);
            builder.set_value(node.text_content());
            parent.push_labelled_by(id)
        }

        parent.push_child(id);

        (id, builder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attribute, DocumentConfig, QualName, ns};

    fn semantic(id: &str, role: &str) -> CanvasAccessNode {
        CanvasAccessNode {
            id: id.into(),
            role: role.into(),
            name: format!("{id} name"),
            value: None,
            checked: None,
            disabled: false,
            focused: false,
            multiline: false,
            order: 0.0,
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 30.0,
        }
    }

    #[test]
    fn a_textbox_node_carries_role_name_value_bounds_and_focus_action() {
        let mut field = semantic("f", "textbox");
        field.value = Some("hello".into());
        field.focused = true;
        let tree = canvas_access_tree(&[field], 7, 3, (100.0, 200.0), 2.0);
        let (id, node) = &tree.nodes[0];
        assert_eq!(node.role(), Role::TextInput);
        assert_eq!(node.label(), Some("f name"));
        assert_eq!(node.value(), Some("hello"));
        assert_eq!(
            node.bounds(),
            Some(Rect {
                x0: 120.0,
                y0: 240.0,
                x1: 320.0,
                y1: 300.0
            })
        );
        assert!(node.supports_action(Action::Focus));
        assert!(!node.supports_action(Action::Click));
        assert_eq!(tree.focus, Some(*id));
        assert_eq!(
            tree.targets[0],
            (
                id.0,
                CanvasAccessTarget {
                    canvas_dom_id: 7,
                    scene_id: 3,
                    target: "f".into()
                }
            )
        );
    }

    #[test]
    fn roles_state_and_actions_follow_the_declared_table() {
        let mut check = semantic("c", "checkbox");
        check.checked = Some(true);
        let mut off = semantic("d", "button");
        off.disabled = true;
        let mut multi = semantic("m", "textbox");
        multi.multiline = true;
        let tree = canvas_access_tree(
            &[
                check,
                off,
                multi,
                semantic("s", "slider"),
                semantic("x", "no-such-role"),
            ],
            1,
            1,
            (0.0, 0.0),
            1.0,
        );
        let node = |i: usize| &tree.nodes[i].1;
        assert_eq!(node(0).toggled(), Some(Toggled::True));
        assert!(node(0).supports_action(Action::Click));
        assert!(node(1).is_disabled());
        assert!(!node(1).supports_action(Action::Focus));
        assert_eq!(node(2).role(), Role::MultilineTextInput);
        assert!(node(3).supports_action(Action::Increment));
        assert!(node(3).supports_action(Action::Decrement));
        assert_eq!(node(4).role(), Role::Unknown);
    }

    #[test]
    fn control_no_focused_node_leaves_the_focus_to_the_dom_and_ids_are_distinct_per_canvas() {
        let a = canvas_access_tree(&[semantic("a", "button")], 1, 1, (0.0, 0.0), 1.0);
        let b = canvas_access_tree(&[semantic("a", "button")], 2, 1, (0.0, 0.0), 1.0);
        assert_eq!(a.focus, None);
        assert_ne!(a.nodes[0].0, b.nodes[0].0);
        assert!(a.nodes[0].0.0 >= VIRTUAL_BASE);
    }

    #[test]
    fn access_actions_map_to_the_four_contract_actions_and_nothing_else() {
        assert_eq!(
            canvas_access_action(Action::Focus),
            Some(CanvasAccessAction::Focus)
        );
        assert_eq!(
            canvas_access_action(Action::Click),
            Some(CanvasAccessAction::Activate)
        );
        assert_eq!(
            canvas_access_action(Action::Increment),
            Some(CanvasAccessAction::Increment)
        );
        assert_eq!(
            canvas_access_action(Action::Decrement),
            Some(CanvasAccessAction::Decrement)
        );
        assert_eq!(canvas_access_action(Action::ScrollDown), None);
    }

    struct Fixed(Vec<CanvasAccessNode>);

    impl blitz_traits::canvas_access::CanvasAccessHost for Fixed {
        fn nodes(&self) -> Vec<CanvasAccessNode> {
            self.0.clone()
        }
        fn access_action(&self, _: &str, _: CanvasAccessAction) {}
        fn text_input(&self, _: &str, _: &str, _: bool) {}
        fn key_input(&self, _: &str, _: &str, _: blitz_traits::canvas_access::CanvasKeyMods) {}
    }

    #[test]
    fn a_registered_canvas_gets_its_nodes_under_the_canvas_element_and_its_actions_resolve() {
        let scene = 9_100_001;
        let mut field = semantic("f", "textbox");
        field.focused = true;
        blitz_traits::canvas_access::register_canvas_access(
            scene,
            std::sync::Arc::new(Fixed(vec![field])),
        );
        let mut doc = BaseDocument::new(DocumentConfig::default());
        let mut mutator = doc.mutate();
        let canvas = mutator.create_element(
            QualName::new(None, ns!(html), local_name!("canvas")),
            vec![Attribute {
                name: QualName::new(None, ns!(), local_name!("src")),
                value: scene.to_string().into(),
            }],
        );
        let plain = mutator.create_element(
            QualName::new(None, ns!(html), local_name!("canvas")),
            vec![],
        );
        mutator.append_children(0, &[canvas, plain]);
        drop(mutator);
        doc.set_focus_to(canvas);

        let tree = doc.build_accessibility_tree();
        blitz_traits::canvas_access::unregister_canvas_access(scene);
        let element = &tree
            .nodes
            .iter()
            .find(|(id, _)| *id == NodeId(canvas as u64))
            .unwrap()
            .1;
        assert_eq!(element.children().len(), 1);
        let child = element.children()[0];
        assert_eq!(tree.focus, child);
        assert_eq!(
            doc.canvas_access_target(child),
            Some((canvas, scene, "f".to_string()))
        );
        let bare = &tree
            .nodes
            .iter()
            .find(|(id, _)| *id == NodeId(plain as u64))
            .unwrap()
            .1;
        assert!(bare.children().is_empty());
        assert_eq!(doc.canvas_access_target(NodeId(canvas as u64)), None);
    }

    #[test]
    fn aria_hidden_marks_accessibility_subtree_hidden() {
        let mut doc = BaseDocument::new(DocumentConfig::default());
        let mut mutator = doc.mutate();
        let hidden = mutator.create_element(
            QualName::new(None, ns!(html), local_name!("div")),
            vec![Attribute {
                name: QualName::new(None, ns!(), local_name!("aria-hidden")),
                value: "true".into(),
            }],
        );
        let child = mutator.create_element(
            QualName::new(None, ns!(html), local_name!("button")),
            vec![],
        );
        mutator.append_children(0, &[hidden]);
        mutator.append_children(hidden, &[child]);
        drop(mutator);

        let tree = doc.build_accessibility_tree();
        let parent = tree
            .nodes
            .iter()
            .find(|(id, _)| *id == NodeId(hidden as u64))
            .unwrap();
        assert!(parent.1.is_hidden());
        assert!(parent.1.children().contains(&NodeId(child as u64)));
    }

    #[test]
    fn aria_checked_sets_checkbox_accessibility_state() {
        let mut doc = BaseDocument::new(DocumentConfig::default());
        let mut mutator = doc.mutate();
        let checkbox = mutator.create_element(
            QualName::new(None, ns!(html), local_name!("button")),
            vec![
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("role")),
                    value: "checkbox".into(),
                },
                Attribute {
                    name: QualName::new(None, ns!(), local_name!("aria-checked")),
                    value: "true".into(),
                },
            ],
        );
        mutator.append_children(0, &[checkbox]);
        drop(mutator);

        let tree = doc.build_accessibility_tree();
        let node = &tree
            .nodes
            .iter()
            .find(|(id, _)| *id == NodeId(checkbox as u64))
            .unwrap()
            .1;
        assert_eq!(node.toggled(), Some(Toggled::True));
    }
}
