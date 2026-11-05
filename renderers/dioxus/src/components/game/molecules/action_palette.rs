use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_array, get_bool_or, get_string, get_string_array, get_string_or, };
use orbital_core::{Value, ValueMap};

pub struct ActionPaletteRenderer;

impl DioxusPatternRenderer for ActionPaletteRenderer {
    fn pattern_name(&self) -> &str {
        "action-palette"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let root_attrs = shared_root_attrs(props);
        let class = merge_class_name("almadar-action-palette", props);
        let actions = get_array(props, "actions");
        let used = get_string_array(props, "usedActionIds");
        let allow_duplicates = get_bool_or(props, "allowDuplicates", true);
        let label = get_string(props, "label");
        let size = get_string(props, "size");
        let category_colors = props
            .as_object()
            .and_then(|value| value.get("categoryColors"))
            .cloned();
        rsx! {
            section {
                class: "{class}",
                style: "padding:var(--almadar-pad);border:1px solid var(--almadar-line);border-radius:var(--almadar-radius);background:var(--almadar-bg)",
                ..root_attrs,
                if let Some(label) = &label {
                    div { class: "almadar-action-palette__label", "{label}" }
                }
                div { class: "almadar-action-palette__items", style: "display:flex;flex-wrap:wrap;gap:var(--almadar-gap)",
                    for action in actions.iter() {
                        {
                            let id = get_string_or(action, "id", "");
                            let disabled = !allow_duplicates && used.contains(&id);
                            let mut spec = ValueMap::new();
                            spec.insert("type".into(), Value::String("action-tile".into()));
                            spec.insert("action".into(), action.clone());
                            spec.insert("disabled".into(), Value::Bool(disabled));
                            if let Some(size) = &size {
                                spec.insert("size".into(), Value::String(size.clone().into()));
                            }
                            if let Some(colors) = &category_colors {
                                spec.insert("categoryColors".into(), colors.clone());
                            }
                            rsx! {
                                div { key: "{id}", {registry.render_spec(&Value::Object(spec.into()), entity, view)} }
                            }
                        }
                    }
                }
            }
        }
    }
}
