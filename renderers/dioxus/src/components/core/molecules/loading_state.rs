use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::{Value, ValueMap};

pub struct LoadingStateRenderer;

fn typography(variant: &str, content: &str, color: &str, class: &str) -> Value {
    let mut props = ValueMap::new();
    props.insert("type".into(), Value::string("typography"));
    props.insert("variant".into(), Value::string(variant));
    props.insert("content".into(), Value::string(content));
    props.insert("color".into(), Value::string(color));
    props.insert("className".into(), Value::string(class));
    Value::Object(props.into())
}

impl DioxusPatternRenderer for LoadingStateRenderer {
    fn pattern_name(&self) -> &str {
        "loading-state"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let title = get_string(props, "title").filter(|title| !title.is_empty());
        let message = get_string(props, "message").unwrap_or_else(|| "Loading...".to_string());
        let full_page = get_bool_or(props, "fullPage", false);
        let layout = if full_page {
            "fixed inset-0 z-[1000] py-12 bg-background/80 backdrop-blur-sm animate-overlay-in"
        } else {
            "py-12"
        };
        let class = merge_class_name(
            &format!("flex flex-col items-center justify-center {layout}"),
            props,
        );
        let mut spinner_props = ValueMap::new();
        spinner_props.insert("type".into(), Value::string("spinner"));
        spinner_props.insert("size".into(), Value::string("lg"));
        let spinner = Value::Object(spinner_props.into());
        let title_spec = title
            .as_deref()
            .map(|title| typography("h3", title, "primary", "mt-4 text-lg"));
        let message_class = if title.is_some() { "mt-2" } else { "mt-4" };
        let message_spec = typography("small", &message, "muted", message_class);
        rsx! {
            div { class: "contents", "data-loading-state": "", ..shared_root_attrs(props),
                div { class: "{class}",
                    {registry.render_spec(&spinner, None, view)}
                    if let Some(title) = title_spec { {registry.render_spec(&title, None, view)} }
                    {registry.render_spec(&message_spec, None, view)}
                }
            }
        }
    }
}
