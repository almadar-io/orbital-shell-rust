use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::{
    get_array, get_bool_or, get_string, get_string_or, merge_class_name,
};
use orbital_core::{Value, ValueMap};

pub struct GameMenuRenderer;

impl DioxusPatternRenderer for GameMenuRenderer {
    fn pattern_name(&self) -> &str {
        "game-menu"
    }

    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let title = get_string_or(props, "title", "");
        let subtitle = get_string(props, "subtitle");
        let mut options = get_array(props, "options");
        if options.is_empty() {
            options = get_array(props, "menuItems");
        }
        let background = get_string(props, "background");
        let background_style = background
            .as_deref()
            .map(|value| format!("background:{value}"))
            .unwrap_or_default();
        let logo = props
            .as_object()
            .and_then(|value| value.get("logo"))
            .cloned()
            .map(|asset| {
                let mut spec = ValueMap::new();
                spec.insert("type".into(), Value::String("atlas-image".into()));
                spec.insert("asset".into(), asset);
                spec.insert("size".into(), Value::Number(96.0));
                registry.render_spec(&Value::Object(spec.into()), entity, view)
            });
        let class = merge_class_name("almadar-game-menu", props);
        let selected_event = get_string(props, "onSelect");
        let sink = view.sink;
        let navigate = view.navigate;
        rsx! {
            div { class: "{class}", style: "{background_style}",
                div { class: "almadar-game-menu__card",
                    if let Some(logo) = logo {
                        div { class: "almadar-game-menu__logo", {logo} }
                    }
                    h1 { class: "almadar-game-menu__title", "{title}" }
                    if let Some(subtitle) = subtitle {
                        p { class: "almadar-game-menu__subtitle", "{subtitle}" }
                    }
                    div { class: "almadar-game-menu__options",
                        for (index, option) in options.iter().enumerate() {
                            {
                                let label = get_string_or(option, "label", "");
                                let disabled = get_bool_or(option, "disabled", false);
                                let sub_label = get_string(option, "subLabel");
                                let item_event = get_string(option, "event");
                                let path = get_string(option, "navigatesTo");
                                let selected_event = selected_event.clone();
                                let option_value = option.clone();
                                let relay: EventSink = Callback::new(move |(name, _): (String, Value)| {
                                    if name != "__game_menu_select" {
                                        return;
                                    }
                                    if let Some(sink) = sink {
                                        if let Some(event) = &item_event {
                                            let mut payload = ValueMap::new();
                                            payload.insert("option".into(), option_value.clone());
                                            sink.call((event.clone(), Value::Object(payload.clone().into())));
                                        }
                                        if let Some(event) = &selected_event {
                                            let mut payload = ValueMap::new();
                                            payload.insert("option".into(), option_value.clone());
                                            sink.call((event.clone(), Value::Object(payload.into())));
                                        }
                                    }
                                    if let (Some(navigate), Some(path)) = (navigate, &path) {
                                        navigate.call(path.clone());
                                    }
                                });
                                let child_view = view.for_source(view.source_trait.clone(), relay);
                                let mut spec = ValueMap::new();
                                spec.insert("type".into(), Value::String("choice-button".into()));
                                spec.insert("text".into(), Value::String(label.into()));
                                spec.insert("index".into(), Value::Number((index + 1) as f64));
                                spec.insert("disabled".into(), Value::Bool(disabled));
                                spec.insert("onClick".into(), Value::String("__game_menu_select".into()));
                                rsx! {
                                    div { key: "{index}", class: "almadar-game-menu__option",
                                        {registry.render_spec(&Value::Object(spec.into()), entity, &child_view)}
                                        if let Some(sub_label) = sub_label {
                                            span { class: "almadar-game-menu__sub-label", "{sub_label}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
