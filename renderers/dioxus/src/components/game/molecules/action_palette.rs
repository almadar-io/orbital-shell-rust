use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_array, get_bool_or, get_number, get_string, get_string_array, get_string_or,
    merge_class_name,
};
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
        let aria_label = get_string(props, "aria-label");
        let aria_labelledby = get_string(props, "aria-labelledby");
        let aria_describedby = get_string(props, "aria-describedby");
        let aria_live = get_string(props, "aria-live");
        let aria_current = get_string(props, "aria-current");
        let aria_pressed = get_string(props, "aria-pressed");
        let aria_expanded = get_bool_or(props, "aria-expanded", false);
        let aria_selected = get_bool_or(props, "aria-selected", false);
        let aria_hidden = get_bool_or(props, "aria-hidden", false);
        let aria_busy = get_bool_or(props, "aria-busy", false);
        let role = get_string(props, "role");
        let lang = get_string(props, "lang");
        let dir = get_string(props, "dir");
        let tab_index = get_number(props, "tabIndex").map(|index| index.to_string());
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
                aria_label: aria_label,
                aria_labelledby: aria_labelledby,
                aria_describedby: aria_describedby,
                aria_live: aria_live,
                aria_current: aria_current,
                aria_pressed: aria_pressed,
                aria_expanded: aria_expanded,
                aria_selected: aria_selected,
                aria_hidden: aria_hidden,
                aria_busy: aria_busy,
                role: role,
                lang: lang,
                dir: dir,
                tabindex: tab_index,
                style: "padding:var(--almadar-pad);border:1px solid var(--almadar-line);border-radius:var(--almadar-radius);background:var(--almadar-bg)",
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
