use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView};
use dioxus::prelude::*;
use dioxus_primitives::dropdown_menu::{
    DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
};
use orbital_core::client::helpers::{get_bool_or, get_string, get_string_or, };
use orbital_core::client::recurse::trait_ref_name as trigger_ref_name;
use orbital_core::{Value, ValueMap};

pub struct MenuRenderer;

#[derive(Props, Clone, PartialEq)]
struct MenuProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn Menu(props: MenuProps) -> Element {
    let view = props.view.as_view();
    let fields = props.props.as_object();
    let trigger = fields.and_then(|fields| fields.get("trigger"));
    let items = fields
        .and_then(|fields| fields.get("items"))
        .and_then(Value::as_array);
    let header = fields.and_then(|fields| fields.get("header"));
    let footer = fields.and_then(|fields| fields.get("footer"));
    let position = get_string_or(&props.props, "position", "bottom-left");
    let side = if position.starts_with("top") {
        "bottom-full mb-1"
    } else {
        "top-full mt-1"
    };
    let alignment = if position.ends_with("right") || position.ends_with("end") {
        "right-0"
    } else {
        "left-0"
    };
    let class = merge_class_name(
        &format!("absolute z-50 bg-card border-[length:var(--border-width)] border-border shadow-elevation-popover rounded-container min-w-0 sm:min-w-[200px] max-w-[calc(100vw-1rem)] py-1 {side} {alignment}"),
        &props.props,
    );
    let sink = view.sink;
    let navigate = view.navigate;
    let trigger_renderer: Option<Callback<Vec<Attribute>, Element>> = trigger.map(|trigger| {
        let trigger = match trigger.as_str() {
            Some(text) if trigger_ref_name(text).is_none() => {
                let mut spec = ValueMap::new();
                spec.insert("type".into(), Value::string("typography"));
                spec.insert("variant".into(), Value::string("small"));
                spec.insert("content".into(), trigger.clone());
                Value::Object(spec.into())
            }
            _ => trigger.clone(),
        };
        let registry = props.registry.clone();
        let owned_view = props.view.clone();
        Callback::new(move |attrs: Vec<Attribute>| {
            rsx! { span { role: "button", tabindex: "0", class: "inline-flex", ..attrs,
                {owned_view.as_view().render_node(&registry, &trigger)}
            } }
        })
    });
    rsx! {
        DropdownMenu { default_open: false, class: "relative inline-flex",
            if let Some(trigger_renderer) = trigger_renderer {
                DropdownMenuTrigger { r#as: trigger_renderer, "" }
            }
            DropdownMenuContent { class: "{class}", role: "menu", instant_close: true,
                if let Some(header) = header {
                    div { class: "px-4 py-2 border-b border-border", {view.render_node(&props.registry, header)} }
                }
                if let Some(items) = items {
                    for (index, item) in items.iter().enumerate() {
                        {
                            if get_string(item, "type").as_deref() == Some("divider") {
                                rsx! { div { key: "divider-{index}", role: "separator", aria_orientation: "horizontal", class: "w-full h-0 border-t border-border my-1 border-solid" } }
                            } else {
                                let label = get_string_or(item, "label", "");
                                let id = get_string(item, "id").unwrap_or_else(|| {
                                    let slug = label.to_lowercase().split_whitespace().collect::<Vec<_>>().join("-");
                                    format!("item-{slug}-{index}")
                                });
                                let title = get_string(item, "title");
                                let icon = get_string(item, "icon");
                                let badge = item.get("badge").cloned();
                                let disabled = get_bool_or(item, "disabled", false);
                                let danger = get_string(item, "variant").as_deref() == Some("danger");
                                let event = get_string(item, "event");
                                let href = get_string(item, "href");
                                let has_submenu = item.get("subMenu").and_then(Value::as_array).is_some_and(|items| !items.is_empty());
                                let row_class = format!(
                                    "w-full flex items-center justify-between gap-3 px-4 py-2 text-start text-sm transition-colors hover:bg-muted focus:outline-none focus:bg-muted disabled:opacity-50 disabled:cursor-not-allowed {} {}",
                                    if disabled { "cursor-not-allowed" } else { "" },
                                    if danger { "text-foreground hover:bg-error/10 border-s-heavy border-error" } else { "" },
                                );
                                let icon_class = if danger { "flex-shrink-0 text-error" } else { "flex-shrink-0" };
                                let icon_spec = pattern_spec("icon", [
                                    ("name", icon.map(|icon| Value::String(icon.into())).unwrap_or(Value::Null)),
                                    ("size", Value::string("sm")),
                                    ("className", Value::string(icon_class)),
                                ]);
                                let label_spec = pattern_spec("typography", [
                                    ("variant", Value::string("small")),
                                    ("className", Value::string("flex-1")),
                                    ("content", Value::String(label.clone().into())),
                                ]);
                                let badge_spec = pattern_spec("badge", [
                                    ("variant", Value::string("default")),
                                    ("size", Value::string("sm")),
                                    ("label", badge.unwrap_or(Value::Null)),
                                ]);
                                let chevron_spec = pattern_spec("icon", [
                                    ("name", Value::string("chevron-right")),
                                    ("size", Value::string("sm")),
                                    ("className", Value::string("flex-shrink-0")),
                                ]);
                                let action_label = label.clone();
                                rsx! {
                                    DropdownMenuItem::<String> { key: "{id}", value: id.clone(), index: index, disabled: disabled,
                                        class: "{row_class}", role: "menuitem", title: title,
                                        on_select: move |item_id: String| {
                                            if has_submenu { return; }
                                            if let (Some(sink), Some(event)) = (sink, &event) {
                                                let mut payload = ValueMap::new();
                                                payload.insert("itemId".into(), Value::String(item_id.into()));
                                                payload.insert("label".into(), Value::String(action_label.clone().into()));
                                                sink.call((event.clone(), Value::Object(payload.into())));
                                            }
                                            if let (Some(navigate), Some(href)) = (navigate, &href) {
                                                navigate.call(href.clone());
                                            }
                                        },
                                        div { class: "flex items-center gap-3 flex-1 min-w-0",
                                            if item.get("icon").is_some() { {props.registry.render_spec(&icon_spec, None, &view)} }
                                            {props.registry.render_spec(&label_spec, None, &view)}
                                            if item.get("badge").is_some() { {props.registry.render_spec(&badge_spec, None, &view)} }
                                            if has_submenu { {props.registry.render_spec(&chevron_spec, None, &view)} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(footer) = footer {
                    div { class: "px-4 py-2 border-t border-border", {view.render_node(&props.registry, footer)} }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for MenuRenderer {
    fn pattern_name(&self) -> &str {
        "menu"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { Menu { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}

#[cfg(all(test, feature = "native", not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use blitz_dom::{Document as _, DocumentConfig};
    use dioxus_native::DioxusDocument;
    use std::cell::RefCell;

    thread_local! {
        static OPEN: RefCell<Option<Signal<bool>>> = const { RefCell::new(None) };
    }

    fn menu_app() -> Element {
        let open = use_signal(|| true);
        OPEN.with(|slot| *slot.borrow_mut() = Some(open));
        rsx! {
            DropdownMenu { open: Some(open()),
                DropdownMenuTrigger { "Menu" }
                DropdownMenuContent { instant_close: true, "data-menu-probe": "open",
                    DropdownMenuItem::<String> { value: "one".to_string(), index: 0usize, "One" }
                }
            }
        }
    }

    #[test]
    fn native_menu_content_unmounts_when_closed_without_a_browser_animation() {
        let _converter_guard = crate::EVENT_CONVERTER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut doc = DioxusDocument::new(
            dioxus_core::VirtualDom::new(menu_app),
            DocumentConfig::default(),
        );
        doc.initial_build();
        doc.poll(None);
        assert!(doc.query_selector("[data-menu-probe]").unwrap().is_some());

        OPEN.with(|slot| {
            let mut open = (*slot.borrow()).expect("menu signal mounted");
            open.set(false);
        });
        assert!(doc.poll(None));
        assert!(doc.query_selector("[data-menu-probe]").unwrap().is_none());
    }
}
