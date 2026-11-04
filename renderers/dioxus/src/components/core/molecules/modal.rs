use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_bool_or, get_string, get_string_or, merge_class_name};
use orbital_core::client::recurse::child_patterns;
use orbital_core::{Value, ValueMap};

pub struct ModalRenderer;

impl DioxusPatternRenderer for ModalRenderer {
    fn pattern_name(&self) -> &str {
        "modal"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        if !get_bool_or(props, "isOpen", true) {
            return rsx! {};
        }
        let contained = get_bool_or(props, "contained", false);
        let overlay_class = if contained {
            "absolute inset-0 z-50 bg-scrim flex items-start justify-center px-4 pb-4 pt-[10%] max-sm:items-end max-sm:p-0 max-sm:pt-0"
        } else {
            "fixed inset-0 z-[1000] bg-scrim flex items-start justify-center px-4 pb-4 pt-[10vh] max-sm:items-end max-sm:p-0 max-sm:pt-0"
        };
        let size = match get_string_or(props, "size", "md").as_str() {
            "sm" => "max-w-md min-w-[min(theme(minWidth.modal-sm),100%)] max-sm:min-w-0",
            "lg" => "max-w-3xl min-w-[min(theme(minWidth.modal-lg),100%)] max-sm:min-w-0",
            "xl" => "max-w-5xl min-w-[min(theme(minWidth.modal-xl),100%)] max-sm:min-w-0",
            "full" => "max-w-full mx-4 min-w-0",
            _ => "max-w-2xl min-w-[min(theme(minWidth.modal-md),100%)] max-sm:min-w-0",
        };
        let look = match get_string_or(props, "look", "centered-card").as_str() {
            "top-sheet" => "top-0 rounded-t-none rounded-b-container max-w-full w-full",
            "side-drawer" => "right-0 top-0 bottom-0 h-full rounded-l-container rounded-r-none w-[400px] max-w-full",
            "full-screen" => "inset-0 rounded-none w-full h-full max-w-full",
            _ => "",
        };
        let frame = "static m-0 p-0 border-0 bg-transparent pointer-events-auto w-full flex flex-col bg-surface surface-material border shadow-elevation-dialog rounded-container max-sm:max-w-none max-sm:w-full max-sm:max-h-[90vh] max-sm:rounded-b-none max-sm:pb-[env(safe-area-inset-bottom)]";
        let height = if contained {
            "max-h-[80%]"
        } else {
            "max-h-[80vh]"
        };
        let class = merge_class_name(&format!("{frame} {size} {height} {look}"), props);
        let title = get_string(props, "title");
        let footer = props.as_object().and_then(|fields| fields.get("footer"));
        let close_event = get_string(props, "closeEvent");
        let overlay_event = close_event.clone();
        let escape_event = close_event.clone();
        let sink = view.sink;
        let close_on_overlay = get_bool_or(props, "closeOnOverlayClick", true);
        let close_on_escape = get_bool_or(props, "closeOnEscape", true);
        let show_close = get_bool_or(props, "showCloseButton", true);
        let children = child_patterns(props, view.trait_content);
        let mut close = ValueMap::new();
        close.insert("type".into(), Value::string("button"));
        close.insert("label".into(), Value::string(""));
        close.insert("icon".into(), Value::string("x"));
        close.insert("size".into(), Value::string("sm"));
        close.insert("variant".into(), Value::string("ghost"));
        close.insert("aria-label".into(), Value::string("Close modal"));
        if let Some(event) = close_event {
            close.insert("action".into(), Value::String(event.into()));
        }
        let close = Value::Object(close.into());
        rsx! {
            div { class: "{overlay_class}",
                onclick: move |_| {
                    if close_on_overlay {
                        if let (Some(sink), Some(event)) = (sink, &overlay_event) {
                            sink.call((event.clone(), Value::Object(ValueMap::new().into())));
                        }
                    }
                },
                dialog { class: "{class}", open: true, aria_modal: "true",
                    onclick: move |event| event.stop_propagation(),
                    onkeydown: move |event| {
                        if close_on_escape && event.key() == Key::Escape {
                            event.stop_propagation();
                            if let (Some(sink), Some(name)) = (sink, &escape_event) {
                                sink.call((name.clone(), Value::Object(ValueMap::new().into())));
                            }
                        }
                    },
                    ..shared_root_attrs(props),
                    if title.is_some() || show_close {
                        div { class: "px-6 py-4 flex items-center justify-between border-b-[length:var(--border-width)] border-border",
                            if let Some(title) = title.as_ref() { h2 { class: "text-lg font-semibold heading-voice", "{title}" } }
                            if show_close { {registry.render_spec(&close, None, view)} }
                        }
                    }
                    div { class: "flex-1 overflow-y-auto p-6",
                        for (child, source) in children.iter() {
                            {view.render_child(registry, child, source.as_deref())}
                        }
                    }
                    if let Some(footer) = footer {
                        div { class: "px-6 py-4 bg-muted border-t-[length:var(--border-width)] border-border",
                            {view.render_node(registry, footer)}
                        }
                    }
                }
            }
        }
    }
}
