use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_bool_or, get_number, get_string, get_string_or, merge_class_name,
};
use orbital_core::{Value, ValueMap};

pub struct ActionTileRenderer;

impl DioxusPatternRenderer for ActionTileRenderer {
    fn pattern_name(&self) -> &str {
        "action-tile"
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
        let action = props
            .as_object()
            .and_then(|value| value.get("action"))
            .cloned()
            .unwrap_or(Value::Null);
        let name = get_string_or(&action, "name", "");
        let category = get_string_or(&action, "category", "");
        let disabled = get_bool_or(props, "disabled", false);
        let size = get_string_or(props, "size", "md");
        let padding = match size.as_str() {
            "sm" => "4px 8px",
            "lg" => "12px 16px",
            _ => "8px 12px",
        };
        let category_color = props
            .as_object()
            .and_then(|value| value.get("categoryColors"))
            .and_then(|colors| colors.as_object())
            .and_then(|colors| colors.get(category.as_str()));
        let background = if disabled {
            None
        } else {
            category_color.and_then(|color| get_string(color, "bg"))
        };
        let border = if disabled {
            None
        } else {
            category_color.and_then(|color| get_string(color, "border"))
        };
        let background = background.as_deref().unwrap_or("var(--almadar-bg)");
        let border = border.as_deref().unwrap_or("var(--almadar-line)");
        let opacity = if disabled { "0.4" } else { "1" };
        let style = format!(
            "display:flex;flex-direction:column;align-items:center;gap:4px;padding:{padding};border:2px solid {border};background:{background};border-radius:var(--almadar-radius);opacity:{opacity};touch-action:none"
        );
        let class = merge_class_name("almadar-action-tile", props);
        let icon_asset = action
            .as_object()
            .and_then(|value| value.get("iconUrl"))
            .cloned();
        let icon = if let Some(asset) = icon_asset {
            let mut spec = ValueMap::new();
            spec.insert("type".into(), Value::String("atlas-image".into()));
            spec.insert("asset".into(), asset);
            spec.insert("size".into(), Value::Number(32.0));
            registry.render_spec(&Value::Object(spec.into()), entity, view)
        } else {
            let glyph = get_string_or(&action, "iconEmoji", "✦");
            rsx! { span { class: "almadar-action-tile__icon", "{glyph}" } }
        };
        rsx! {
            div {
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
                style: "{style}",
                draggable: !disabled,
                "aria-disabled": disabled,
                ondragstart: move |event| {
                    if disabled {
                        event.prevent_default();
                        return;
                    }
                    let data = match serde_json::to_string(&action) {
                        Ok(data) => data,
                        Err(error) => {
                            tracing::error!(%error, "action-tile drag payload could not be serialized");
                            event.prevent_default();
                            return;
                        }
                    };
                    let transfer = event.data().data_transfer();
                    if let Err(error) = transfer.set_data("application/x-almadar-slot-item", &data) {
                        tracing::error!(%error, "action-tile drag payload could not be stored");
                        event.prevent_default();
                        return;
                    }
                    transfer.set_effect_allowed("copy");
                },
                {icon}
                span { class: "almadar-action-tile__name", "{name}" }
            }
        }
    }
}
