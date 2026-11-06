use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use dioxus_primitives::tabs::{TabContent, TabList, TabTrigger, Tabs};
use orbital_core::client::helpers::{get_bool_or, get_string, get_string_or, };
use orbital_core::{Value, ValueMap};

pub struct TabsRenderer;

fn tab_id(item: &Value) -> String {
    get_string(item, "id")
        .or_else(|| get_string(item, "value"))
        .unwrap_or_default()
}

impl DioxusPatternRenderer for TabsRenderer {
    fn pattern_name(&self) -> &str {
        "tabs"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let fields = props.as_object();
        let items = fields
            .and_then(|fields| fields.get("items").or_else(|| fields.get("tabs")))
            .and_then(Value::as_array);
        let Some(items) = items else {
            return rsx! { div { class: "w-full", ..shared_root_attrs(props) } };
        };
        if items.is_empty() {
            return rsx! { div { class: "w-full", ..shared_root_attrs(props) } };
        }

        let active = get_string(props, "activeTab");
        let default = get_string(props, "defaultActiveTab")
            .filter(|id| !id.is_empty())
            .or_else(|| {
                items
                    .iter()
                    .find(|item| get_bool_or(item, "active", false))
                    .map(tab_id)
            })
            .unwrap_or_else(|| tab_id(&items[0]));
        let orientation = get_string_or(props, "orientation", "horizontal");
        let horizontal = orientation != "vertical";
        let variant = get_string_or(props, "variant", "default");
        let root_class = merge_class_name(
            if horizontal {
                "w-full"
            } else {
                "w-full flex flex-row"
            },
            props,
        );
        let list_class = if horizontal {
            if variant == "pills" {
                "flex flex-row overflow-x-auto snap-x snap-mandatory gap-1 p-1 bg-muted rounded-interactive"
            } else {
                "flex flex-row border-b-[length:var(--border-width)] border-border overflow-x-auto snap-x snap-mandatory"
            }
        } else if variant == "pills" {
            "flex flex-col gap-1 p-1 bg-muted rounded-interactive"
        } else {
            "flex flex-col border-r-[length:var(--border-width)] border-border"
        };
        let variant_class = match variant.as_str() {
            "pills" => "rounded-interactive data-[state=active]:bg-primary data-[state=active]:text-primary-foreground",
            "underline" => "border-b-[length:var(--border-width)] border-transparent data-[state=active]:border-primary",
            _ => "border-b-[length:var(--border-width)] border-transparent hover:border-muted-foreground data-[state=active]:border-primary",
        };
        let trigger_class = format!(
            "flex items-center gap-2 px-4 py-2 text-sm font-medium transition-colors whitespace-nowrap focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed text-muted-foreground hover:text-foreground data-[state=active]:text-foreground {variant_class} {}",
            if horizontal { "snap-start shrink-0" } else { "" }
        );
        let event = get_string(props, "tabChangeEvent");
        let item_events: Vec<(String, String)> = items
            .iter()
            .filter_map(|item| get_string(item, "event").map(|name| (tab_id(item), name)))
            .collect();
        let sink = view.sink;

        rsx! {
            Tabs {
                class: "{root_class}",
                value: active,
                default_value: default,
                horizontal: horizontal,
                activate_on_focus: true,
                notify_on_active_click: true,
                on_value_change: move |tab_id: String| {
                    if let (Some(sink), Some(event)) = (sink, &event) {
                        let mut payload = ValueMap::new();
                        payload.insert("tabId".into(), Value::String(tab_id.into()));
                        let payload = Value::Object(payload.into());
                        sink.call((event.clone(), payload));
                    }
                },
                on_trigger_click: move |tab_id: String| {
                    if let Some(sink) = sink {
                        if let Some((_, item_event)) = item_events.iter().find(|(id, _)| id == &tab_id) {
                            let mut payload = ValueMap::new();
                            payload.insert("tabId".into(), Value::String(tab_id.into()));
                            let payload = Value::Object(payload.into());
                            sink.call((item_event.clone(), payload));
                        }
                    }
                },
                attributes: shared_root_attrs(props),
                TabList { class: "{list_class}", aria_orientation: "{orientation}",
                    for (index, item) in items.iter().enumerate() {
                        {
                            let id = tab_id(item);
                            let label = get_string_or(item, "label", "");
                            let disabled = get_bool_or(item, "disabled", false);
                            let icon_asset = item.get("iconAsset");
                            let mut icon_spec = ValueMap::new();
                            icon_spec.insert("type".into(), Value::string("icon"));
                            if let Some(icon) = get_string(item, "icon") { icon_spec.insert("name".into(), Value::String(icon.into())); }
                            icon_spec.insert("size".into(), Value::string("sm"));
                            let icon_spec = Value::Object(icon_spec.into());
                            let mut badge_spec = ValueMap::new();
                            badge_spec.insert("type".into(), Value::string("badge"));
                            if let Some(badge) = item.get("badge") { badge_spec.insert("label".into(), badge.clone()); }
                            badge_spec.insert("size".into(), Value::string("sm"));
                            let badge_spec = Value::Object(badge_spec.into());
                            let mut image_spec = ValueMap::new();
                            image_spec.insert("type".into(), Value::string("atlas-image"));
                            if let Some(asset) = icon_asset { image_spec.insert("asset".into(), asset.clone()); }
                            image_spec.insert("size".into(), Value::Int(16));
                            image_spec.insert("fit".into(), Value::string("contain"));
                            let image_spec = Value::Object(image_spec.into());
                            rsx! {
                                TabTrigger { key: "{id}", value: id, index: index, disabled: disabled,
                                    class: "{trigger_class}", "data-testid": "tab-{tab_id(item)}",
                                    if icon_asset.is_some() { {registry.render_spec(&image_spec, None, view)} }
                                    else if item.get("icon").is_some() { {registry.render_spec(&icon_spec, None, view)} }
                                    span { class: "!text-inherit", "{label}" }
                                    if item.get("badge").is_some() { {registry.render_spec(&badge_spec, None, view)} }
                                }
                            }
                        }
                    }
                }
                for (index, item) in items.iter().enumerate() {
                    if let Some(content) = item.get("content").filter(|value| !matches!(value, Value::Null)) {
                        TabContent {
                            key: "{tab_id(item)}-content", value: tab_id(item), index: index,
                            class: if horizontal { "mt-4" } else { "flex-1 min-w-0 ps-4" },
                            {view.render_node(registry, content)}
                        }
                    }
                }
            }
        }
    }
}

#[cfg(all(test, feature = "native", not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use blitz_dom::{Document as _, DocumentConfig};
    use blitz_traits::events::{BlitzKeyEvent, KeyState, MouseEventButtons, UiEvent};
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus_native::DioxusDocument;
    use keyboard_types::{Code, Key, Location, Modifiers};
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static EVENTS: RefCell<Vec<(String, Value)>> = const { RefCell::new(Vec::new()) };
    }

    fn tabs_app() -> Element {
        let props: Value = serde_json::from_value(serde_json::json!({
            "type": "tabs",
            "activeTab": "first",
            "tabChangeEvent": "TAB_CHANGED",
            "items": [
                {"id": "first", "label": "First", "content": "First panel", "event": "FIRST_PICKED"},
                {"id": "disabled", "label": "Disabled", "disabled": true, "content": "Disabled panel", "event": "DISABLED_PICKED"},
                {"id": "third", "label": "Third", "content": "Third panel", "event": "THIRD_PICKED"}
            ]
        })).unwrap();
        let entities = HashMap::new();
        let trait_content = ValueMap::new();
        let sink = Callback::new(|event| EVENTS.with(|events| events.borrow_mut().push(event)));
        let view = DioxusView {
            trait_name: "Tabs",
            orbital_name: "Tabs",
            current_state: "idle",
            entities: &entities,
            trait_content: &trait_content,
            source_trait: None,
            sink: Some(sink),
            source_sink: Callback::new(move |_| sink),
            navigate: None,
            dock: None,
            viewport_width: None,
            canvas_theme: None,
            theme_mode: None,
            toggle_theme: None,
        };
        let mut registry = DioxusPatternRegistry::new();
        registry.register(Box::new(TabsRenderer));
        registry.render_spec(&props, None, &view)
    }

    fn settle(doc: &mut DioxusDocument) {
        for _ in 0..4 {
            if !doc.poll(None) {
                break;
            }
        }
        doc.resolve(0.0);
    }

    fn click(doc: &mut DioxusDocument, selector: &str) {
        let id = doc.query_selector(selector).unwrap().unwrap();
        let mut mouse = doc
            .get_node(id)
            .unwrap()
            .synthetic_click_event_data(Modifiers::default());
        let hit = doc
            .hit(mouse.x, mouse.y)
            .expect("tab must be inside the viewport");
        let mut ancestor = Some(hit.node_id);
        let mut hits_tab = false;
        while let Some(node_id) = ancestor {
            if node_id == id {
                hits_tab = true;
                break;
            }
            ancestor = doc.get_node(node_id).and_then(|node| node.parent);
        }
        assert!(hits_tab, "{selector} center hit another element");
        mouse.buttons = MouseEventButtons::None;
        doc.handle_ui_event(UiEvent::MouseMove(mouse.clone()));
        mouse.buttons = MouseEventButtons::Primary;
        doc.handle_ui_event(UiEvent::MouseDown(mouse.clone()));
        mouse.buttons = MouseEventButtons::None;
        doc.handle_ui_event(UiEvent::MouseUp(mouse));
        settle(doc);
    }

    #[test]
    fn native_tabs_preserve_controlled_panel_and_declared_click_events() {
        let _converter_guard = crate::EVENT_CONVERTER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        EVENTS.with(|events| events.borrow_mut().clear());
        let mut doc = DioxusDocument::new(
            dioxus_core::VirtualDom::new(tabs_app),
            DocumentConfig {
                viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Light)),
                ..Default::default()
            },
        );
        doc.initial_build();
        settle(&mut doc);
        assert!(doc
            .query_selector("[data-testid=tab-first][data-state=active]")
            .unwrap()
            .is_some());
        assert!(doc
            .query_selector("[data-testid=tab-disabled][data-disabled=true]")
            .unwrap()
            .is_some());
        assert!(doc
            .query_selector("[role=tabpanel][data-state=active]")
            .unwrap()
            .is_some());

        click(&mut doc, "[data-testid=tab-disabled]");
        assert!(EVENTS.with(|events| events.borrow().is_empty()));
        assert!(doc
            .query_selector("[data-testid=tab-first][data-state=active]")
            .unwrap()
            .is_some());

        click(&mut doc, "[data-testid=tab-first]");
        EVENTS.with(|events| {
            let events = events.borrow();
            assert_eq!(events.len(), 2);
            assert_eq!(events[0].0, "TAB_CHANGED");
            assert_eq!(events[1].0, "FIRST_PICKED");
            assert_eq!(
                events[0].1.get("tabId").and_then(Value::as_str),
                Some("first")
            );
            assert_eq!(
                events[1].1.get("tabId").and_then(Value::as_str),
                Some("first")
            );
        });
        assert!(doc
            .query_selector("[data-testid=tab-first][data-state=active]")
            .unwrap()
            .is_some());

        EVENTS.with(|events| events.borrow_mut().clear());
        let first_id = doc
            .query_selector("[data-testid=tab-first]")
            .unwrap()
            .unwrap();
        doc.set_focus_to(first_id);
        doc.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
            key: Key::ArrowRight,
            code: Code::ArrowRight,
            modifiers: Modifiers::default(),
            location: Location::Standard,
            is_auto_repeating: false,
            is_composing: false,
            state: KeyState::Pressed,
            text: None,
        }));
        settle(&mut doc);
        EVENTS.with(|events| {
            let events = events.borrow();
            assert_eq!(
                events.len(),
                1,
                "ArrowRight must activate the next enabled tab"
            );
            assert_eq!(events[0].0, "TAB_CHANGED");
            assert_eq!(
                events[0].1.get("tabId").and_then(Value::as_str),
                Some("third")
            );
        });
        assert!(doc
            .query_selector("[data-testid=tab-first][data-state=active]")
            .unwrap()
            .is_some());
    }
}
