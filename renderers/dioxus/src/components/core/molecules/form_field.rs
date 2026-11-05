//! Native Dioxus renderer for the `form-field` pattern, the web `FormField` molecule.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    cn, passthrough_attrs, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::child_patterns;
use orbital_core::Value;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIELD: AtomicU64 = AtomicU64::new(0);

/// Dioxus renderer for the `form-field` pattern.
pub struct FormFieldRenderer;

impl FormFieldRenderer {
    const NAME: &str = "form-field";
}

impl DioxusPatternRenderer for FormFieldRenderer {
    fn pattern_name(&self) -> &str {
        Self::NAME
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { FormFieldBody { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}

#[derive(Props, Clone, PartialEq)]
struct FormFieldBodyProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn FormFieldBody(input: FormFieldBodyProps) -> Element {
    let props = &input.props;
    let view = input.view.as_view();
    let registry = &input.registry;
    let generated =
        use_hook(|| format!("form-field-{}", NEXT_FIELD.fetch_add(1, Ordering::Relaxed)));
    let label = get_string_or(props, "label", "");
    let required = get_bool_or(props, "required", false);
    let error = get_string(props, "error").filter(|text| !text.is_empty());
    let hint = get_string(props, "hint").filter(|text| !text.is_empty());
    let children = child_patterns(props, view.trait_content);
    let field = match children.as_slice() {
        [(spec, None)] => spec.as_object().cloned(),
        _ => None,
    };
    let field_id = field.as_ref().map(|map| {
        map.get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| generated.clone())
    });
    let base_id = field_id.clone().unwrap_or_else(|| generated.clone());
    let hint_id = format!("{base_id}-hint");
    let error_id = format!("{base_id}-error");
    let message_id = if error.is_some() {
        Some(error_id.clone())
    } else if hint.is_some() {
        Some(hint_id.clone())
    } else {
        None
    };
    let field_node = match field {
        Some(mut map) => {
            let existing = map
                .get("aria-describedby")
                .and_then(Value::as_str)
                .map(str::to_string);
            let described: Vec<String> = existing.into_iter().chain(message_id).collect();
            if let Some(id) = field_id.as_ref() {
                map.insert("id".into(), Value::string(id.clone()));
            }
            if !described.is_empty() {
                map.insert(
                    "aria-describedby".into(),
                    Value::string(described.join(" ")),
                );
            }
            if error.is_some() {
                map.insert("aria-invalid".into(), Value::Bool(true));
            }
            registry.render_spec(&Value::Object(map.into()), None, &view)
        }
        None => rsx! {
            for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
        },
    };
    let label_spec = pattern_spec(
        "label",
        [
            ("text", Value::string(label)),
            (
                "htmlFor",
                Value::string(field_id.clone().unwrap_or_default()),
            ),
            ("required", Value::Bool(required)),
        ],
    );
    let error_spec = error.as_ref().map(|text| {
        pattern_spec(
            "typography",
            [
                ("variant", Value::string("caption")),
                ("color", Value::string("error")),
                ("id", Value::string(error_id.clone())),
                ("content", Value::string(text.clone())),
            ],
        )
    });
    let hint_spec = hint.as_ref().filter(|_| error.is_none()).map(|text| {
        pattern_spec(
            "typography",
            [
                ("variant", Value::string("caption")),
                ("color", Value::string("muted")),
                ("id", Value::string(hint_id.clone())),
                ("content", Value::string(text.clone())),
            ],
        )
    });
    let class = cn(&[
        "flex flex-col gap-1 items-stretch justify-start",
        &get_string_or(props, "className", ""),
    ]);
    let attrs = passthrough_attrs(props, &[]);
    rsx! {
        div { class: "{class}", style: "height:auto", ..attrs,
            {registry.render_spec(&label_spec, None, &view)}
            {field_node}
            if let Some(spec) = error_spec.as_ref() { {registry.render_spec(spec, None, &view)} }
            if let Some(spec) = hint_spec.as_ref() { {registry.render_spec(spec, None, &view)} }
        }
    }
}
