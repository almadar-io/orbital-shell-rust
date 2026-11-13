use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs_with_defaults, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_bool_or, get_string, get_string_or, };
use orbital_core::client::recurse::child_patterns;
use orbital_core::{Value, ValueMap};

pub struct DrawerRenderer;

fn width_class(width: &str) -> &'static str {
    match width {
        "sm" => "w-full sm:w-80",
        "md" => "w-full sm:w-96",
        "lg" => "w-full sm:max-w-lg",
        "xl" => "w-full sm:max-w-2xl",
        "full" => "w-screen",
        _ => "",
    }
}

fn overlay_spec(contained: bool, close_event: Option<&str>, enabled: bool) -> Value {
    let mut fields = ValueMap::new();
    fields.insert("type".into(), Value::string("overlay"));
    fields.insert(
        "className".into(),
        Value::string(if contained {
            "almadar-drawer-overlay-contained"
        } else {
            "almadar-drawer-overlay-viewport"
        }),
    );
    if enabled {
        if let Some(event) = close_event {
            fields.insert("action".into(), Value::string(event));
        }
    }
    Value::Object(fields.into())
}

fn close_button_spec(close_event: Option<&str>, label: &str) -> Value {
    let mut fields = ValueMap::new();
    fields.insert("type".into(), Value::string("button"));
    fields.insert("label".into(), Value::string(""));
    fields.insert("icon".into(), Value::string("x"));
    fields.insert("size".into(), Value::string("sm"));
    fields.insert("variant".into(), Value::string("ghost"));
    fields.insert("aria-label".into(), Value::string(label));
    if let Some(event) = close_event {
        fields.insert("action".into(), Value::string(event));
    }
    Value::Object(fields.into())
}

impl DioxusPatternRenderer for DrawerRenderer {
    fn pattern_name(&self) -> &str {
        "drawer"
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
        let position = get_string_or(props, "position", "right");
        let position_class = if position == "left" {
            "left-0 border-r"
        } else {
            "right-0 border-l"
        };
        let width = get_string_or(props, "width", "md");
        let width_class = width_class(&width);
        let width_style = if width_class.is_empty() {
            format!("width: {width};")
        } else {
            String::new()
        };
        let frame_class = if contained {
            "absolute top-0 bottom-0 z-50 max-h-full"
        } else {
            "fixed top-0 bottom-0 z-[60] max-h-screen"
        };
        let class = merge_class_name(
            &format!("{frame_class} {position_class} {width_class} m-0 p-0 flex flex-col bg-surface surface-material border shadow-elevation-dialog"),
            props,
        );
        let close_event = get_string(props, "closeEvent");
        let escape_event = close_event.clone();
        let close_on_escape = get_bool_or(props, "closeOnEscape", true);
        let overlay = overlay_spec(
            contained,
            close_event.as_deref(),
            get_bool_or(props, "closeOnOverlayClick", true),
        );
        let show_close = get_bool_or(props, "showCloseButton", true);
        let close_label = get_string_or(
            props,
            "closeButtonLabel",
            &crate::i18n::t("aria.closeDrawer"),
        );
        let close_button = close_button_spec(close_event.as_deref(), &close_label);
        let title = props.as_object().and_then(|fields| fields.get("title"));
        let footer = props.as_object().and_then(|fields| fields.get("footer"));
        let children = child_patterns(props, view.trait_content);
        let sink = view.sink;
        let attrs = shared_root_attrs_with_defaults(props, &[("role", Value::string("dialog"))]);

        rsx! {
            {registry.render_spec(&overlay, None, view)}
            dialog {
                class: "{class}",
                style: "{width_style}",
                open: true,
                aria_modal: "true",
                onkeydown: move |event| {
                    if close_on_escape && event.key() == Key::Escape {
                        event.stop_propagation();
                        if let (Some(sink), Some(name)) = (sink, &escape_event) {
                            sink.call((name.clone(), Value::Object(ValueMap::new().into())));
                        }
                    }
                },
                ..attrs,
                if title.is_some() || show_close {
                    div { class: "px-6 py-4 flex items-center justify-between shrink-0 border-b-[length:var(--border-width)] border-border",
                        if let Some(title) = title { h2 { class: "text-lg font-semibold heading-voice", {view.render_node(registry, title)} } }
                        if show_close { {registry.render_spec(&close_button, None, view)} }
                    }
                }
                div { class: "flex-1 overflow-y-auto p-6",
                    for (child, source) in children.iter() {
                        {view.render_child(registry, child, source.as_deref())}
                    }
                }
                if let Some(footer) = footer {
                    div { class: "px-6 py-4 shrink-0 bg-muted border-t-[length:var(--border-width)] border-border",
                        {view.render_node(registry, footer)}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{close_button_spec, overlay_spec, width_class};
    use orbital_core::Value;

    #[test]
    fn width_presets_and_custom_width_are_distinct() {
        assert_eq!(width_class("md"), "w-full sm:w-96");
        assert_eq!(width_class("40rem"), "");
    }

    #[test]
    fn close_controls_only_bind_declared_event() {
        let active = overlay_spec(false, Some("CLOSE"), true);
        let inactive = overlay_spec(true, Some("CLOSE"), false);
        assert_eq!(
            active
                .as_object()
                .unwrap()
                .get("action")
                .and_then(Value::as_str),
            Some("CLOSE")
        );
        assert!(inactive.as_object().unwrap().get("action").is_none());
        let button = close_button_spec(Some("CLOSE"), "Dismiss panel");
        assert_eq!(
            button
                .as_object()
                .unwrap()
                .get("aria-label")
                .and_then(Value::as_str),
            Some("Dismiss panel")
        );
        assert_eq!(
            button
                .as_object()
                .unwrap()
                .get("action")
                .and_then(Value::as_str),
            Some("CLOSE")
        );
    }
}
