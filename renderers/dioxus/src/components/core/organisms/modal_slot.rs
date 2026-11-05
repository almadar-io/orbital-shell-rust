use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    fanout_sink, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_string, get_string_or};
use orbital_core::client::recurse::child_patterns;
use orbital_core::Value;

const SLOT_CLOSE: &str = "slot:close";

pub struct ModalSlotRenderer;

#[derive(Props, Clone, PartialEq)]
struct ModalSlotProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn ModalSlot(input: ModalSlotProps) -> Element {
    let close = use_hook(|| fanout_sink(input.view.sink, SLOT_CLOSE, &["CLOSE", "CANCEL"]));
    let view = input.view.as_view();
    let props = &input.props;
    let children = child_patterns(props, view.trait_content);
    if children.is_empty() {
        return rsx! {};
    }
    let title = get_string(props, "title")
        .filter(|title| !title.is_empty())
        .or_else(|| match children.as_slice() {
            [(only, _)] => get_string(only, "title"),
            _ => None,
        });
    let mut fields = vec![
        ("isOpen", Value::Bool(true)),
        ("closeEvent", Value::string(SLOT_CLOSE)),
        ("size", Value::string(get_string_or(props, "size", "md"))),
    ];
    if let Some(title) = title {
        fields.push(("title", Value::string(title)));
    }
    if let Some(class) = get_string(props, "className") {
        fields.push(("className", Value::string(class)));
    }
    if let Some(children) = props.get("children") {
        fields.push(("children", children.clone()));
    }
    let spec = pattern_spec("modal", fields);
    let slot_view = view.for_source(view.source_trait.clone(), close);
    input.registry.render_spec(&spec, None, &slot_view)
}

impl DioxusPatternRenderer for ModalSlotRenderer {
    fn pattern_name(&self) -> &str {
        "modal-slot"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { ModalSlot { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
