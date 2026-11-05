//! Dioxus renderer for the shared `navigation` pattern.

use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::{Value, ValueMap};

pub struct NavigationRenderer;

fn button_spec(item: &Value) -> Value {
    let mut fields = ValueMap::new();
    fields.insert("type".into(), Value::String("button".into()));
    fields.insert(
        "label".into(),
        Value::String(get_string_or(item, "label", "").into()),
    );
    fields.insert(
        "variant".into(),
        Value::String(if get_bool_or(item, "isActive", false) {
            "primary".into()
        } else {
            "ghost".into()
        }),
    );
    fields.insert("size".into(), Value::String("sm".into()));
    fields.insert(
        "disabled".into(),
        Value::Bool(get_bool_or(item, "disabled", false)),
    );
    fields.insert("className".into(), Value::String("relative".into()));
    if let Some(icon) = get_string(item, "icon") {
        fields.insert("icon".into(), Value::String(icon.into()));
    }
    if let Some(href) = get_string(item, "href").filter(|href| !href.is_empty()) {
        fields.insert("href".into(), Value::String(href.into()));
    }
    if get_bool_or(item, "isActive", false) {
        fields.insert("aria-current".into(), Value::String("page".into()));
    }
    Value::Object(fields.into())
}

fn badge_spec(item: &Value) -> Option<Value> {
    let badge = item.as_object()?.get("badge")?.clone();
    let mut fields = ValueMap::new();
    fields.insert("type".into(), Value::String("badge".into()));
    fields.insert("variant".into(), Value::String("danger".into()));
    fields.insert("size".into(), Value::String("sm".into()));
    fields.insert(
        "className".into(),
        Value::String("absolute -top-1 -right-1".into()),
    );
    fields.insert("label".into(), badge);
    Some(Value::Object(fields.into()))
}

impl DioxusPatternRenderer for NavigationRenderer {
    fn pattern_name(&self) -> &str {
        "navigation"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let items = get_array(props, "items");
        let orientation = get_string_or(props, "orientation", "horizontal");
        let base = if orientation == "vertical" {
            "flex flex-col gap-1"
        } else {
            "flex flex-row items-center gap-1"
        };
        let class = merge_class_name(base, props);
        rsx! {
            nav { class: "{class}", ..shared_root_attrs(props),
                for item in items.iter() {
                    {let button = button_spec(item);
                    let badge = badge_spec(item);
                    let id = get_string_or(item, "id", "");
                    rsx! { span { key: "{id}", class: "relative inline-flex",
                        {registry.render_spec(&button, None, view)}
                        if let Some(badge) = badge.as_ref() { {registry.render_spec(badge, None, view)} }
                    } }}
                }
            }
        }
    }
}
