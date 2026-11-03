use crate::{local_name, BaseDocument, Node as BlitzDomNode};
use accesskit::{Action, Node as AccessKitNode, NodeId, Rect, Role, TreeId, TreeInfo, TreeUpdate};

impl BaseDocument {
    pub fn build_accessibility_tree(&self) -> TreeUpdate {
        let mut nodes = std::collections::HashMap::new();
        let mut window = AccessKitNode::new(Role::Window);

        self.visit(|node_id, node| {
            let parent = node
                .parent
                .and_then(|parent_id| nodes.get_mut(&parent_id))
                .map(|(_, parent)| parent)
                .unwrap_or(&mut window);
            let (id, builder) = self.build_accessibility_node(node, parent);

            nodes.insert(node_id, (id, builder));
        });

        let mut nodes: Vec<_> = nodes
            .into_iter()
            .map(|(_, (id, node))| (id, node))
            .collect();
        nodes.push((NodeId(u64::MAX), window));

        let tree = TreeInfo::new(NodeId(u64::MAX));
        TreeUpdate {
            nodes,
            tree: Some(tree),
            tree_id: TreeId::ROOT,
            focus: NodeId(self.focus_node_id.map(|id| id as u64).unwrap_or(u64::MAX)),
        }
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
                builder.set_toggled(element_data.attr(local_name!("checked")).is_some().into());
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
    use crate::{ns, Attribute, DocumentConfig, QualName};

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
}
