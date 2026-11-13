//! Native Dioxus renderer for the `doc-sidebar` pattern (web: DocSidebar.tsx).

use crate::registry::{shared_root_attrs_with_defaults, DioxusPatternRegistry};
use crate::renderer::{
    array_value, cn, pattern_spec, render_with_local_sink, DioxusPatternRenderer, DioxusView,
    OwnedDioxusView, DOC_NAV_ROW_ACTIVE, DOC_NAV_ROW_HOVER, DOC_NAV_ROW_IDLE,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `doc-sidebar` pattern.
pub struct DocSidebarRenderer;

impl DocSidebarRenderer {
    const NAME: &str = "doc-sidebar";
}

const STACK_NONE: &str = "flex flex-col gap-0 items-stretch justify-start";

fn child_items(item: &Value) -> Vec<Value> {
    item.get("items")
        .and_then(Value::as_array)
        .map(|items| items.to_vec())
        .unwrap_or_default()
}

fn has_active(item: &Value) -> bool {
    get_bool_or(item, "active", false) || child_items(item).iter().any(has_active)
}

#[derive(Props, Clone, PartialEq)]
struct CategoryProps {
    item: Value,
    depth: usize,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn DocSidebarCategory(input: CategoryProps) -> Element {
    let item = &input.item;
    let depth = input.depth;
    let view = input.view.as_view();
    let registry = &input.registry;
    let children = child_items(item);
    let initial = children.iter().any(has_active);
    let expanded = use_signal(|| initial);
    let label = get_string_or(item, "label", "");
    let active = get_bool_or(item, "active", false);

    if !children.is_empty() {
        let is_open = expanded();
        let header_class = cn(&[
            "h-auto w-full justify-start gap-2 rounded-interactive px-2 py-1.5 font-normal",
            DOC_NAV_ROW_HOVER,
            if depth > 0 { "pl-4" } else { "" },
        ]);
        let header_spec = pattern_spec(
            "button",
            [
                ("variant", Value::string("ghost")),
                ("aria-expanded", Value::Bool(is_open)),
                ("className", Value::string(header_class)),
                ("action", Value::string("__local_toggle_doc_category")),
                (
                    "children",
                    array_value(vec![
                        pattern_spec(
                            "icon",
                            [
                                (
                                    "name",
                                    Value::string(if is_open {
                                        "chevron-down"
                                    } else {
                                        "chevron-right"
                                    }),
                                ),
                                ("size", Value::string("xs")),
                                ("className", Value::string("text-muted-foreground shrink-0")),
                            ],
                        ),
                        pattern_spec(
                            "typography",
                            [
                                ("variant", Value::string("caption")),
                                (
                                    "className",
                                    Value::string("text-xs uppercase tracking-wider text-muted-foreground font-semibold"),
                                ),
                                ("as", Value::string("span")),
                                ("content", Value::string(label)),
                            ],
                        ),
                    ]),
                ),
            ],
        );
        let header = render_with_local_sink(&view, registry, &header_spec, move |event, _| {
            if event != "__local_toggle_doc_category" {
                return false;
            }
            let mut state = expanded;
            let next = !state();
            state.set(next);
            true
        });
        let owned = input.view.clone();
        return rsx! {
            div { class: "{STACK_NONE}", style: "height:auto",
                {header}
                if is_open {
                    div { class: "{STACK_NONE} pl-4", style: "height:auto",
                        for (index, child) in children.iter().enumerate() {
                            DocSidebarCategory { key: "{index}", item: child.clone(), depth: depth + 1, view: owned.clone(), registry: registry.clone() }
                        }
                    }
                }
            }
        };
    }

    let text = pattern_spec(
        "typography",
        [
            ("variant", Value::string("body2")),
            ("color", Value::string("inherit")),
            (
                "weight",
                Value::string(if active { "semibold" } else { "normal" }),
            ),
            ("as", Value::string("span")),
            ("content", Value::string(label)),
        ],
    );
    let href = get_string(item, "href").filter(|href| !href.is_empty());
    let Some(href) = href else {
        let class = cn(&[
            "rounded-interactive px-3 py-1.5 text-sm",
            DOC_NAV_ROW_IDLE,
            if depth > 0 { "ml-2" } else { "" },
        ]);
        return rsx! { div { class: "{class}", {registry.render_spec(&text, None, &view)} } };
    };
    let state_class = if active {
        DOC_NAV_ROW_ACTIVE.to_string()
    } else {
        cn(&[DOC_NAV_ROW_IDLE, DOC_NAV_ROW_HOVER])
    };
    let class = cn(&[
        "h-auto w-full justify-start rounded-interactive px-3 py-1.5 text-sm font-normal",
        if depth > 0 { "ml-2" } else { "" },
        &state_class,
    ]);
    let mut fields = vec![
        ("variant", Value::string("ghost")),
        ("href", Value::string(href)),
        ("className", Value::string(class)),
        ("children", array_value(vec![text])),
    ];
    if active {
        fields.push(("aria-current", Value::string("page")));
    }
    registry.render_spec(&pattern_spec("button", fields), None, &view)
}

impl DioxusPatternRenderer for DocSidebarRenderer {
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
        let items = get_array(props, "items");
        let class = cn(&["w-full", &get_string_or(props, "className", "")]);
        let attrs = shared_root_attrs_with_defaults(
            props,
            &[
                ("role", Value::string("navigation")),
                (
                    "aria-label",
                    Value::string(crate::i18n::t("aria.docsSidebar")),
                ),
            ],
        );
        let owned = view.to_owned();
        rsx! {
            div { class: "{class}", ..attrs,
                div { class: "flex flex-col gap-1 items-stretch justify-start", style: "height:auto",
                    for (index, item) in items.iter().enumerate() {
                        DocSidebarCategory { key: "{index}", item: item.clone(), depth: 0usize, view: owned.clone(), registry: registry.clone() }
                    }
                }
            }
        }
    }
}
