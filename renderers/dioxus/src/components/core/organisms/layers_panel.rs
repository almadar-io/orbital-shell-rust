use crate::registry::DioxusPatternRegistry;
use crate::renderer::{pattern_spec, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct LayersPanelRenderer;

impl LayersPanelRenderer {
    const NAME: &str = "layers-panel";
}

impl DioxusPatternRenderer for LayersPanelRenderer {
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
        let mut fields = vec![
            ("items", Value::Array(Vec::<Value>::new().into())),
            ("look", Value::string("nav")),
        ];
        if let Some(selected) = get_string(props, "selectedId") {
            fields.push(("selectedId", Value::string(selected)));
        }
        if let Some(event) = get_string(props, "onSelect") {
            fields.push(("onNodeSelect", Value::string(event)));
        }
        if let Some(event) = get_string(props, "onReorder") {
            fields.push(("onNodeReorder", Value::string(event)));
        }
        registry.render_spec(&pattern_spec("file-tree", fields), None, view)
    }
}
