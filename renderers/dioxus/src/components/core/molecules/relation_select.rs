use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, dom_passthrough, object_value, pattern_spec, DioxusPatternRenderer,
    DioxusView, EventSink, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `relation-select` pattern.
pub struct RelationSelectRenderer;

#[derive(Props, Clone, PartialEq)]
struct RelationSelectProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

const TOGGLE: &str = "__relation_select_toggle";
const CLEAR: &str = "__relation_select_clear";
const SEARCH: &str = "__relation_select_search";
const ENTER: &str = "__relation_select_enter";
const PICK: &str = "__relation_select_pick:";

#[derive(Clone)]
struct Choice {
    value: String,
    label: String,
    description: Option<String>,
    disabled: bool,
}

fn choices(props: &Value) -> Vec<Choice> {
    props
        .as_object()
        .and_then(|fields| fields.get("options"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| Choice {
                    value: get_string_or(item, "value", ""),
                    label: get_string_or(item, "label", ""),
                    description: get_string(item, "description").filter(|text| !text.is_empty()),
                    disabled: get_bool_or(item, "disabled", false),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn filtered(options: &[Choice], query: &str) -> Vec<Choice> {
    if query.trim().is_empty() {
        return options.to_vec();
    }
    let query = query.to_lowercase();
    options
        .iter()
        .filter(|option| {
            option.label.to_lowercase().contains(&query)
                || option
                    .description
                    .as_ref()
                    .is_some_and(|text| text.to_lowercase().contains(&query))
        })
        .cloned()
        .collect()
}

#[allow(non_snake_case)]
fn RelationSelect(input: RelationSelectProps) -> Element {
    let props = &input.props;
    let view = input.view.as_view();
    let options = choices(props);
    let value = get_string(props, "value");
    let disabled = get_bool_or(props, "disabled", false);
    let is_loading = get_bool_or(props, "isLoading", false);
    let clearable = get_bool_or(props, "clearable", true);
    let error = get_string(props, "error").filter(|text| !text.is_empty());
    let on_change = get_string(props, "onChange").filter(|name| !name.is_empty());
    let placeholder = get_string(props, "placeholder").unwrap_or_else(|| "Select...".to_string());
    let search_placeholder =
        get_string(props, "searchPlaceholder").unwrap_or_else(|| "Search".to_string());
    let empty_message =
        get_string(props, "emptyMessage").unwrap_or_else(|| "No options found".to_string());
    let mut open = use_signal(|| false);
    let mut query = use_signal(String::new);
    let sink = view.sink;

    let selected = value
        .as_ref()
        .and_then(|value| options.iter().find(|option| &option.value == value))
        .cloned();
    let visible = filtered(&options, &query());
    let show_clear = clearable && selected.is_some() && !disabled;

    let relay_options = options.clone();
    let relay: EventSink = Callback::new(move |(name, payload): (String, Value)| {
        let emit = |next: Value| {
            if let (Some(sink), Some(event)) = (sink, &on_change) {
                sink.call((event.clone(), object_value([("value", next)])));
            }
        };
        let mut pick = move |option: &Choice| {
            if option.disabled {
                return;
            }
            emit(Value::string(option.value.clone()));
            open.set(false);
            query.set(String::new());
        };
        if name == TOGGLE {
            if !disabled {
                let next = !*open.peek();
                open.set(next);
                if !next {
                    return;
                }
                query.set(String::new());
            }
        } else if name == CLEAR {
            emit(Value::Null);
        } else if name == SEARCH {
            if let Some(text) = get_string(&payload, "value") {
                query.set(text);
            }
        } else if name == ENTER {
            let list = filtered(&relay_options, &query.peek());
            if list.len() == 1 {
                pick(&list[0]);
            }
        } else if let Some(index) = name
            .strip_prefix(PICK)
            .and_then(|index| index.parse::<usize>().ok())
        {
            let list = filtered(&relay_options, &query.peek());
            if let Some(option) = list.get(index) {
                pick(option);
            }
        }
    });

    let is_open = open();
    let trigger_label = if is_loading {
        "Loading...".to_string()
    } else if let Some(option) = &selected {
        option.label.clone()
    } else {
        placeholder
    };
    let trigger_class = cn(&[
        "w-full justify-between font-normal",
        if error.is_some() {
            "border-error/50 focus:border-error focus:ring-error"
        } else {
            ""
        },
        if is_open {
            "ring-2 ring-primary border-primary"
        } else {
            ""
        },
        if show_clear { "pe-12" } else { "" },
    ]);
    let mut children = vec![pattern_spec(
        "button",
        [
            ("variant", Value::string("secondary")),
            ("label", Value::string(trigger_label)),
            ("rightIcon", Value::string("chevron-down")),
            ("className", Value::string(trigger_class)),
            ("disabled", Value::Bool(disabled)),
            ("action", Value::string(TOGGLE)),
            ("aria-haspopup", Value::string("listbox")),
            ("aria-expanded", Value::Bool(is_open)),
        ],
    )];
    if show_clear {
        children.push(pattern_spec(
            "button",
            [
                ("variant", Value::string("ghost")),
                ("size", Value::string("sm")),
                ("leftIcon", Value::string("x")),
                ("label", Value::string("")),
                ("action", Value::string(CLEAR)),
                ("aria-label", Value::string("Clear")),
                (
                    "className",
                    Value::string(
                        "absolute top-1/2 -translate-y-1/2 end-8 h-6 w-6 p-0 text-muted-foreground",
                    ),
                ),
            ],
        ));
    }
    if is_open {
        let search = pattern_spec(
            "box",
            [
                ("padding", Value::string("sm")),
                ("className", Value::string("border-b border-border")),
                (
                    "children",
                    array_value(vec![pattern_spec(
                        "input",
                        [
                            ("inputType", Value::string("text")),
                            ("value", Value::string(query())),
                            ("onChange", Value::string(SEARCH)),
                            ("action", Value::string(ENTER)),
                            ("role", Value::string("combobox")),
                            ("aria-expanded", Value::Bool(true)),
                            ("aria-autocomplete", Value::string("list")),
                            ("aria-label", Value::string(search_placeholder.clone())),
                            ("placeholder", Value::string(search_placeholder.clone())),
                            ("leftIcon", Value::string("search")),
                            ("className", Value::string("text-sm")),
                        ],
                    )]),
                ),
            ],
        );
        let body = if is_loading {
            pattern_spec(
                "box",
                [
                    ("padding", Value::string("md")),
                    ("display", Value::string("flex")),
                    ("className", Value::string("justify-center")),
                    (
                        "children",
                        array_value(vec![pattern_spec(
                            "spinner",
                            [
                                ("size", Value::string("md")),
                                ("color", Value::string("primary")),
                            ],
                        )]),
                    ),
                ],
            )
        } else if visible.is_empty() {
            pattern_spec(
                "box",
                [
                    ("padding", Value::string("md")),
                    (
                        "children",
                        array_value(vec![pattern_spec(
                            "typography",
                            [
                                ("variant", Value::string("body")),
                                ("color", Value::string("muted")),
                                ("className", Value::string("text-center")),
                                ("content", Value::string(empty_message)),
                            ],
                        )]),
                    ),
                ],
            )
        } else {
            let rows = visible
                .iter()
                .enumerate()
                .map(|(index, option)| {
                    let is_selected = value.as_deref() == Some(option.value.as_str());
                    let class = cn(&[
                        "text-start text-sm cursor-pointer hover:bg-muted",
                        if is_selected {
                            "bg-primary/10 text-foreground"
                        } else {
                            ""
                        },
                        if option.disabled {
                            "opacity-50 cursor-not-allowed"
                        } else {
                            ""
                        },
                    ]);
                    let mut fields = vec![
                        ("role".to_string(), Value::string("option")),
                        ("aria-selected".to_string(), Value::Bool(is_selected)),
                        ("fullWidth".to_string(), Value::Bool(true)),
                        ("paddingX".to_string(), Value::string("sm")),
                        ("paddingY".to_string(), Value::string("sm")),
                        ("className".to_string(), Value::string(class)),
                    ];
                    if option.disabled {
                        fields.push(("aria-disabled".to_string(), Value::Bool(true)));
                    } else {
                        fields.push((
                            "action".to_string(),
                            Value::string(format!("{PICK}{index}")),
                        ));
                    }
                    let mut texts = vec![pattern_spec(
                        "typography",
                        [
                            ("variant", Value::string("body")),
                            ("className", Value::string("font-medium")),
                            ("content", Value::string(option.label.clone())),
                        ],
                    )];
                    if let Some(description) = &option.description {
                        texts.push(pattern_spec(
                            "typography",
                            [
                                ("variant", Value::string("caption")),
                                ("color", Value::string("muted")),
                                ("content", Value::string(description.clone())),
                            ],
                        ));
                    }
                    fields.push(("children".to_string(), array_value(texts)));
                    pattern_spec("box", fields)
                })
                .collect();
            pattern_spec(
                "vstack",
                [
                    ("gap", Value::string("none")),
                    ("children", array_value(rows)),
                ],
            )
        };
        let list = pattern_spec(
            "box",
            [
                ("overflow", Value::string("auto")),
                ("className", Value::string("max-h-60")),
                ("role", Value::string("listbox")),
                ("children", array_value(vec![body])),
            ],
        );
        children.push(pattern_spec(
            "box",
            [
                ("position", Value::string("absolute")),
                ("bg", Value::string("surface")),
                ("border", Value::Bool(true)),
                ("rounded", Value::string("md")),
                (
                    "className",
                    Value::string("shadow-elevation-popover surface-material z-50 w-full mt-1"),
                ),
                ("children", array_value(vec![search, list])),
            ],
        ));
    }
    if let Some(error) = error {
        children.push(pattern_spec(
            "typography",
            [
                ("variant", Value::string("caption")),
                ("color", Value::string("error")),
                ("className", Value::string("mt-1")),
                ("content", Value::string(error)),
            ],
        ));
    }
    let mut root = vec![
        (
            "className".to_string(),
            Value::string(cn(&[
                "relative",
                get_string(props, "className").as_deref().unwrap_or(""),
            ])),
        ),
        ("children".to_string(), array_value(children)),
    ];
    root.extend(dom_passthrough(props));
    let inner_view = view.for_source(view.source_trait.clone(), relay);
    input
        .registry
        .render_spec(&pattern_spec("box", root), None, &inner_view)
}

impl DioxusPatternRenderer for RelationSelectRenderer {
    fn pattern_name(&self) -> &str {
        "relation-select"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { RelationSelect { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
