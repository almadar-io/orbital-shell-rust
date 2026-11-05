//! Native body of the `doc-search` pattern, mirroring the web `DocSearch` molecule.
//!
//! `onSearch` is a render callback (`nonAuthorable` in the pattern contract), so a program never supplies
//! results and the web dropdown never opens; the field and its clear control are the whole surface.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    pattern_spec, DioxusPatternRenderer, DioxusView, EventSink, OwnedDioxusView, BOX_ATOM_BASE,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `doc-search` pattern.
pub struct DocSearchRenderer;

impl DocSearchRenderer {
    const NAME: &str = "doc-search";
}

#[derive(Props, Clone, PartialEq)]
struct SearchProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn Search(input: SearchProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let mut query = use_signal(String::new);
    let relay: EventSink =
        Callback::new(
            move |(event, payload): (String, Value)| match event.as_str() {
                "__ds:query" => query.set(get_string_or(&payload, "value", "")),
                "__ds:clear" => query.set(String::new()),
                _ => {}
            },
        );
    let local = view.for_source(view.source_trait.clone(), relay);
    let current = query();
    let field = pattern_spec(
        "input",
        [
            ("inputType", Value::string("search")),
            (
                "placeholder",
                Value::string(get_string_or(
                    props,
                    "placeholder",
                    "Search documentation...",
                )),
            ),
            ("value", Value::string(current.to_string())),
            ("onChange", Value::string("__ds:query")),
            ("leftIcon", Value::string("search")),
            ("clearable", Value::Bool(!current.is_empty())),
            ("onClear", Value::string("__ds:clear")),
        ],
    );
    let class = format!(
        "{BOX_ATOM_BASE} relative w-full {}",
        get_string_or(props, "className", "")
    );
    rsx! {
        div { class: "{class}", ..shared_root_attrs(props),
            div { class: "{BOX_ATOM_BASE} relative",
                {input.registry.render_spec(&field, None, &local)}
            }
        }
    }
}

impl DioxusPatternRenderer for DocSearchRenderer {
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
        rsx! { Search { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
