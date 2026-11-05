use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, dom_passthrough, object_value, pattern_spec, typography_spec,
    DioxusPatternRenderer, DioxusView, EventSink, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `tag-input` pattern.
pub struct TagInputRenderer;

#[derive(Props, Clone, PartialEq)]
struct TagInputProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

const ENTER: &str = "__tag_input_enter";
const DRAFT: &str = "__tag_input_draft";
const REMOVE: &str = "__tag_input_remove:";

#[allow(non_snake_case)]
fn TagInput(input: TagInputProps) -> Element {
    let props = &input.props;
    let view = input.view.as_view();
    let tags = get_string_array(props, "value");
    let disabled = get_bool_or(props, "disabled", false);
    let unique = get_bool_or(props, "unique", true);
    let variant = get_string_or(props, "variant", "default");
    let placeholder = get_string_or(props, "placeholder", "Type and press Enter…");
    let helper = get_string(props, "helperText").filter(|text| !text.is_empty());
    let add_event = get_string(props, "addEvent").filter(|name| !name.is_empty());
    let remove_event = get_string(props, "removeEvent").filter(|name| !name.is_empty());
    let mut draft = use_signal(String::new);
    let sink = view.sink;

    let relay_tags = tags.clone();
    let relay: EventSink = Callback::new(move |(name, payload): (String, Value)| {
        if name == DRAFT {
            if let Some(value) = get_string(&payload, "value") {
                draft.set(value);
            }
        } else if name == ENTER {
            if disabled {
                return;
            }
            let Some(raw) = get_string(&payload, "value") else {
                return;
            };
            let tag = raw.trim().to_string();
            if tag.is_empty() {
                return;
            }
            if unique && relay_tags.contains(&tag) {
                draft.set(String::new());
                return;
            }
            let mut next = relay_tags.clone();
            next.push(tag.clone());
            if let (Some(sink), Some(event)) = (sink, &add_event) {
                sink.call((
                    event.clone(),
                    object_value([
                        ("tag", Value::string(tag)),
                        (
                            "value",
                            array_value(next.into_iter().map(Value::string).collect()),
                        ),
                    ]),
                ));
            }
            draft.set(String::new());
        } else if let Some(index) = name
            .strip_prefix(REMOVE)
            .and_then(|index| index.parse::<usize>().ok())
        {
            if disabled || index >= relay_tags.len() {
                return;
            }
            let mut next = relay_tags.clone();
            let tag = next.remove(index);
            if let (Some(sink), Some(event)) = (sink, &remove_event) {
                sink.call((
                    event.clone(),
                    object_value([
                        ("tag", Value::string(tag)),
                        ("index", Value::Int(index as i64)),
                        (
                            "value",
                            array_value(next.into_iter().map(Value::string).collect()),
                        ),
                    ]),
                ));
            }
        }
    });

    let chips = array_value(
        tags.iter()
            .enumerate()
            .map(|(index, tag)| {
                let mut fields = vec![
                    ("label", Value::string(tag.clone())),
                    ("variant", Value::string(variant.clone())),
                    ("size", Value::string("sm")),
                    ("removeLabel", Value::string(format!("Remove {tag}"))),
                ];
                if !disabled {
                    fields.push(("onRemove", Value::string(format!("{REMOVE}{index}"))));
                }
                pattern_spec("badge", fields)
            })
            .collect(),
    );
    let mut children = Vec::new();
    if !tags.is_empty() {
        children.push(pattern_spec(
            "hstack",
            [
                ("gap", Value::string("xs")),
                ("className", Value::string("flex-wrap")),
                ("children", chips),
            ],
        ));
    }
    children.push(pattern_spec(
        "input",
        [
            ("value", Value::string(draft())),
            ("placeholder", Value::string(placeholder)),
            ("disabled", Value::Bool(disabled)),
            ("onChange", Value::string(DRAFT)),
            ("action", Value::string(ENTER)),
        ],
    ));
    if let Some(helper) = helper {
        children.push(typography_spec("caption", Some("muted"), "", &helper));
    }
    let class = cn(&[
        "w-full",
        get_string(props, "className").as_deref().unwrap_or(""),
    ]);
    let mut root = vec![
        ("gap".to_string(), Value::string("xs")),
        ("className".to_string(), Value::string(class)),
        ("children".to_string(), array_value(children)),
    ];
    root.extend(dom_passthrough(props));
    let root = pattern_spec("vstack", root);
    let inner_view = view.for_source(view.source_trait.clone(), relay);
    input.registry.render_spec(&root, None, &inner_view)
}

impl DioxusPatternRenderer for TagInputRenderer {
    fn pattern_name(&self) -> &str {
        "tag-input"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { TagInput { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
