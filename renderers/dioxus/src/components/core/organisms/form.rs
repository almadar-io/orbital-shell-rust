use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    object_value, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::client::recurse::*;
use orbital_core::{Value, ValueMap};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_FORM: AtomicUsize = AtomicUsize::new(0);

const CHANGE: &str = "form:change:";
const TOGGLE: &str = "form:toggle:";
const SUBMIT: &str = "form:submit";
const CANCEL: &str = "form:cancel";

/// Dioxus renderer for the `form` pattern.
pub struct FormRenderer;

impl FormRenderer {
    const NAME: &str = "form";
}

impl DioxusPatternRenderer for FormRenderer {
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
        rsx! { FormBody { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}

#[derive(Props, Clone, PartialEq)]
struct FormBodyProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn node(name: &str, fields: Vec<(&str, Value)>) -> Value {
    pattern_spec(name, fields)
}

fn list(items: Vec<Value>) -> Value {
    Value::Array(items.into())
}

fn text(value: impl AsRef<str>) -> Value {
    Value::string(value.as_ref().to_string())
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => *number != 0.0 && !number.is_nan(),
        Value::Int(number) => *number != 0,
        Value::String(string) => !string.is_empty(),
        _ => true,
    }
}

fn js_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(flag) => flag.to_string(),
        Value::Int(number) => number.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(string) => string.to_string(),
        Value::Array(items) => items.iter().map(js_string).collect::<Vec<_>>().join(","),
        _ => "[object Object]".to_string(),
    }
}

#[derive(Clone)]
struct Relation {
    entity: Option<String>,
    cardinality: Option<String>,
}

#[derive(Clone)]
pub(crate) struct SchemaField {
    name: String,
    label: Option<String>,
    ty: Option<String>,
    input_type: Option<String>,
    placeholder: Option<String>,
    required: bool,
    default: Option<Value>,
    options: Vec<Value>,
    values: Option<Vec<String>>,
    labels: Vec<(String, String)>,
    validation_enum: Vec<String>,
    relation: Option<Relation>,
    max: Option<f64>,
    pattern: Option<String>,
    hint: Option<String>,
}

fn string_items(value: Option<&Value>) -> Option<Vec<String>> {
    value.and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect()
    })
}

impl SchemaField {
    fn parse(value: &Value) -> Option<Self> {
        let name = nonempty(get_string(value, "name")).or(nonempty(get_string(value, "field")))?;
        let labels = value
            .get("labels")
            .and_then(Value::as_object)
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|(key, label)| {
                        label
                            .as_str()
                            .map(|label| (key.to_string(), label.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let relation = value
            .get("relation")
            .filter(|relation| relation.is_object())
            .map(|relation| Relation {
                entity: get_string(relation, "entity"),
                cardinality: get_string(relation, "cardinality"),
            });
        Some(Self {
            name,
            label: get_string(value, "label"),
            ty: get_string(value, "type"),
            input_type: get_string(value, "inputType"),
            placeholder: get_string(value, "placeholder"),
            required: get_bool_or(value, "required", false),
            default: value
                .get("defaultValue")
                .or_else(|| value.get("default"))
                .filter(|default| !default.is_null())
                .cloned(),
            options: get_array(value, "options"),
            values: string_items(value.get("values")),
            labels,
            validation_enum: string_items(
                value
                    .get("validation")
                    .and_then(|validation| validation.get("enum")),
            )
            .unwrap_or_default(),
            relation,
            max: get_number(value, "max"),
            pattern: get_string(value, "pattern"),
            hint: get_string(value, "hint"),
        })
    }

    fn named(name: &str) -> Self {
        Self {
            name: name.to_string(),
            label: None,
            ty: Some("string".to_string()),
            input_type: None,
            placeholder: None,
            required: false,
            default: None,
            options: Vec::new(),
            values: None,
            labels: Vec::new(),
            validation_enum: Vec::new(),
            relation: None,
            max: None,
            pattern: None,
            hint: None,
        }
    }

    fn label_of(&self, value: &str) -> String {
        self.labels
            .iter()
            .find(|(key, _)| key == value)
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| value.to_string())
    }

    fn enum_options(&self) -> Vec<Value> {
        if !self.options.is_empty() {
            return self.options.clone();
        }
        let values = match &self.values {
            Some(values) if !values.is_empty() => values,
            _ => &self.validation_enum,
        };
        values
            .iter()
            .map(|value| {
                object_value([
                    ("value", text(value)),
                    ("label", text(self.label_of(value))),
                ])
            })
            .collect()
    }

    fn input_type(&self) -> String {
        if let Some(input_type) = nonempty(self.input_type.clone()) {
            return input_type;
        }
        let ty = self.ty.as_deref();
        if ty == Some("relation") || self.relation.is_some() {
            return "relation".into();
        }
        if ty == Some("array") {
            return "array".into();
        }
        if ty == Some("enum") || self.values.is_some() || !self.enum_options().is_empty() {
            return "select".into();
        }
        match ty.map(str::to_lowercase).as_deref() {
            Some("email") => "email",
            Some("password") => "password",
            Some("url") => "url",
            Some("file") => "file",
            Some("image") => "image",
            Some("money") => "currency",
            Some("number" | "integer" | "float") => "number",
            Some("date") => "date",
            Some("datetime" | "timestamp") => "datetime-local",
            Some("boolean") => "checkbox",
            Some("textarea" | "text") => {
                if self.max.is_some_and(|max| max > 200.0) {
                    "textarea"
                } else {
                    "text"
                }
            }
            _ => "text",
        }
        .into()
    }
}

fn to_id(value: &Value) -> Value {
    let id_of = |item: &Value| {
        item.get("id")
            .filter(|id| !id.is_null())
            .map(|id| text(js_string(id)))
    };
    match value {
        Value::Array(items) => list(
            items
                .iter()
                .map(|item| {
                    if item.is_object() {
                        id_of(item).unwrap_or_else(|| item.clone())
                    } else {
                        item.clone()
                    }
                })
                .collect(),
        ),
        Value::Object(_) => id_of(value).unwrap_or_else(|| value.clone()),
        other => other.clone(),
    }
}

fn schema_entity_fields(entity: Option<&Value>) -> Option<&Vec<Value>> {
    entity
        .filter(|entity| entity.is_object())
        .and_then(|entity| entity.get("fields"))
        .and_then(Value::as_array)
}

pub(crate) fn initial_data(props: &Value) -> ValueMap {
    let entity = props.get("entity");
    let row =
        entity.filter(|entity| entity.is_object() && schema_entity_fields(Some(entity)).is_none());
    let caller = props.get("initialData").filter(|data| data.is_object());
    let mut merged = ValueMap::default();
    for source in [row, caller].into_iter().flatten() {
        if let Some(fields) = source.as_object() {
            for (key, value) in fields.iter() {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    let mut normalized = ValueMap::default();
    for (key, value) in merged.iter() {
        let value = if key.as_str() == "id" {
            value.clone()
        } else {
            to_id(value)
        };
        normalized.insert(key.clone(), value);
    }
    normalized
}

fn effective_fields(props: &Value) -> Vec<SchemaField> {
    let entity = props.get("entity");
    let schema = schema_entity_fields(entity);
    let declared = props
        .get("fields")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let source: Vec<Value> = if declared.is_empty() {
        schema.cloned().unwrap_or_default()
    } else {
        declared
    };
    let overrides = get_array(props, "fieldOverrides");
    source
        .iter()
        .filter_map(|field| {
            let parsed = if let Some(name) = field.as_str() {
                let found = schema.and_then(|fields| {
                    fields
                        .iter()
                        .find(|candidate| get_string(candidate, "name").as_deref() == Some(name))
                });
                match found.and_then(SchemaField::parse) {
                    Some(mut entity_field) => {
                        entity_field.hint =
                            found.and_then(|found| get_string(found, "description"));
                        entity_field
                    }
                    None => SchemaField::named(name),
                }
            } else {
                SchemaField::parse(field)?
            };
            Some(apply_override(parsed, &overrides))
        })
        .collect()
}

fn apply_override(mut field: SchemaField, overrides: &[Value]) -> SchemaField {
    if let Some(over) = overrides
        .iter()
        .find(|over| get_string(over, "name").as_deref() == Some(field.name.as_str()))
    {
        if let Some(label) = get_string(over, "label") {
            field.label = Some(label);
        }
        if let Some(placeholder) = get_string(over, "placeholder") {
            field.placeholder = Some(placeholder);
        }
        if let Some(hint) = get_string(over, "hint") {
            field.hint = Some(hint);
        }
    }
    field
}

struct FieldContext<'a> {
    uid: usize,
    is_loading: bool,
    data: &'a ValueMap,
    relations: Option<&'a Value>,
    relations_loading: Option<&'a Value>,
}

fn change_event(mode: &str, name: &str) -> Value {
    text(format!("{CHANGE}{mode}:{name}"))
}

fn hint_node(id: String, hint: &str) -> Value {
    node(
        "typography",
        vec![
            ("variant", text("caption")),
            ("color", text("muted")),
            ("id", text(id)),
            ("content", text(hint)),
        ],
    )
}

fn upload_node(
    accept: Option<&str>,
    max_size: Option<i64>,
    ctx: &FieldContext,
    name: &str,
) -> Value {
    let mut fields = vec![
        ("maxFiles", Value::Int(1)),
        ("disabled", Value::Bool(ctx.is_loading)),
        ("onFiles", change_event("file", name)),
    ];
    if let Some(accept) = accept {
        fields.push(("accept", text(accept)));
    }
    if let Some(max_size) = max_size {
        fields.push(("maxSize", Value::Int(max_size)));
    }
    node("upload-drop-zone", fields)
}

fn input_node(
    input_type: &str,
    dom_id: &str,
    field: &SchemaField,
    value: Value,
    event: Value,
    ctx: &FieldContext,
) -> Value {
    let mut fields = vec![
        ("inputType", text(input_type)),
        ("id", text(dom_id)),
        ("value", value),
        ("required", Value::Bool(field.required)),
        ("disabled", Value::Bool(ctx.is_loading)),
        ("onChange", event),
    ];
    if let Some(placeholder) = &field.placeholder {
        fields.push(("placeholder", text(placeholder)));
    }
    if let Some(pattern) = &field.pattern {
        fields.push(("pattern", text(pattern)));
    }
    node("input", fields)
}

fn numeric_value(current: &Value) -> Value {
    match current {
        Value::Number(_) | Value::Int(_) => current.clone(),
        other => text(js_string(other)),
    }
}

fn control_node(
    field: &SchemaField,
    input_type: &str,
    dom_id: &str,
    current: &Value,
    label: &str,
    ctx: &FieldContext,
) -> Value {
    let name = field.name.as_str();
    let select_placeholder =
        nonempty(field.placeholder.clone()).unwrap_or_else(|| format!("Select {label}..."));
    match input_type {
        "checkbox" => node(
            "checkbox",
            vec![
                ("id", text(dom_id)),
                ("label", text(label)),
                ("checked", Value::Bool(truthy(current))),
                ("disabled", Value::Bool(ctx.is_loading)),
                ("onChange", change_event("checked", name)),
            ],
        ),
        "textarea" => {
            let mut fields = vec![
                ("id", text(dom_id)),
                ("value", text(js_string(current))),
                ("required", Value::Bool(field.required)),
                ("disabled", Value::Bool(ctx.is_loading)),
                ("onChange", change_event("text", name)),
            ];
            if let Some(placeholder) = &field.placeholder {
                fields.push(("placeholder", text(placeholder)));
            }
            node("textarea", fields)
        }
        "select" => node(
            "select",
            vec![
                ("id", text(dom_id)),
                ("options", list(field.enum_options())),
                ("value", text(js_string(current))),
                ("disabled", Value::Bool(ctx.is_loading)),
                ("placeholder", text(select_placeholder)),
                ("onValueChange", change_event("text", name)),
            ],
        ),
        "relation" => {
            let options = ctx
                .relations
                .and_then(|relations| relations.get(name))
                .cloned()
                .unwrap_or_else(|| list(Vec::new()));
            let loading = ctx
                .relations_loading
                .and_then(|loading| loading.get(name))
                .is_some_and(truthy);
            let many = field
                .relation
                .as_ref()
                .and_then(|relation| relation.cardinality.as_deref())
                .is_some_and(|cardinality| {
                    matches!(cardinality, "many" | "one-to-many" | "many-to-many")
                });
            if many {
                let selected: Vec<Value> = current
                    .as_array()
                    .map(|items| items.iter().map(|item| text(js_string(item))).collect())
                    .unwrap_or_default();
                return node(
                    "select",
                    vec![
                        ("id", text(dom_id)),
                        ("multiple", Value::Bool(true)),
                        ("searchable", Value::Bool(true)),
                        ("clearable", Value::Bool(true)),
                        ("options", options),
                        ("value", list(selected)),
                        ("disabled", Value::Bool(ctx.is_loading)),
                        ("placeholder", text(select_placeholder)),
                        ("onValueChange", change_event("list", name)),
                    ],
                );
            }
            let entity = field
                .relation
                .as_ref()
                .and_then(|relation| relation.entity.clone())
                .unwrap_or_else(|| label.to_string());
            let mut fields = vec![
                ("id", text(dom_id)),
                ("options", options),
                ("isLoading", Value::Bool(loading)),
                ("required", Value::Bool(field.required)),
                ("disabled", Value::Bool(ctx.is_loading)),
                ("placeholder", text(select_placeholder)),
                ("searchPlaceholder", text(format!("Search {entity}..."))),
                ("clearable", Value::Bool(!field.required)),
                ("onChange", change_event("relation", name)),
            ];
            if truthy(current) {
                fields.push(("value", text(js_string(current))));
            }
            node("relation-select", fields)
        }
        "array" => {
            let tags: Vec<Value> = match current {
                Value::Array(items) => items.iter().map(|item| text(js_string(item))).collect(),
                Value::Null => Vec::new(),
                other if js_string(other).is_empty() => Vec::new(),
                other => vec![text(js_string(other))],
            };
            let mut fields = vec![
                ("disabled", Value::Bool(ctx.is_loading)),
                ("value", list(tags)),
                ("onChange", change_event("list", name)),
            ];
            if let Some(placeholder) = &field.placeholder {
                fields.push(("placeholder", text(placeholder)));
            }
            node("tag-input", fields)
        }
        "number" => input_node(
            "number",
            dom_id,
            field,
            numeric_value(current),
            change_event("number", name),
            ctx,
        ),
        "currency" => {
            let mut spec = input_node(
                "number",
                dom_id,
                field,
                numeric_value(current),
                change_event("number", name),
                ctx,
            );
            if let Some(fields) = spec.as_object_mut() {
                fields.insert("leftIcon".into(), text("dollar-sign"));
            }
            spec
        }
        "file" => upload_node(field.pattern.as_deref(), None, ctx, name),
        "image" => {
            let url = current.as_str().unwrap_or("").to_string();
            let mut children = Vec::new();
            if !url.is_empty() {
                children.push(node(
                    "image",
                    vec![
                        ("src", text(&url)),
                        ("alt", text("")),
                        ("rounded", text("none")),
                        ("aria-hidden", Value::Bool(true)),
                        (
                            "className",
                            text("h-24 w-full max-w-xs rounded-container border border-border object-cover"),
                        ),
                    ],
                ));
            }
            children.push(upload_node(
                Some("image/*"),
                Some(2 * 1024 * 1024),
                ctx,
                name,
            ));
            let mut url_field = field.clone();
            url_field.placeholder = Some("…or paste an image address".to_string());
            children.push(input_node(
                "url",
                dom_id,
                &url_field,
                text(url),
                change_event("text", name),
                ctx,
            ));
            node(
                "vstack",
                vec![("gap", text("sm")), ("children", list(children))],
            )
        }
        "date" | "datetime-local" | "email" | "url" | "password" => input_node(
            input_type,
            dom_id,
            field,
            text(js_string(current)),
            change_event("text", name),
            ctx,
        ),
        _ => input_node(
            "text",
            dom_id,
            field,
            text(js_string(current)),
            change_event("text", name),
            ctx,
        ),
    }
}

fn field_node(field: &SchemaField, ctx: &FieldContext) -> Value {
    let input_type = field.input_type();
    let label = nonempty(field.label.clone()).unwrap_or_else(|| field.name.clone());
    let current = ctx
        .data
        .get(field.name.as_str())
        .filter(|value| !value.is_null())
        .or(field.default.as_ref())
        .cloned()
        .unwrap_or_else(|| text(""));
    let dom_id = format!("form-{}-{}", ctx.uid, field.name);
    let control = control_node(field, &input_type, &dom_id, &current, &label, ctx);
    let hint = field
        .hint
        .as_ref()
        .map(|hint| hint_node(format!("{dom_id}-hint"), hint));
    if input_type == "checkbox" || input_type == "image" {
        let mut children = Vec::new();
        if input_type == "image" {
            children.push(node(
                "label",
                vec![
                    ("text", text(&label)),
                    ("htmlFor", text(&dom_id)),
                    ("required", Value::Bool(field.required)),
                ],
            ));
        }
        children.push(control);
        children.extend(hint);
        return node(
            "vstack",
            vec![("gap", text("xs")), ("children", list(children))],
        );
    }
    let mut children = vec![
        node(
            "label",
            vec![
                ("text", text(&label)),
                ("htmlFor", text(&dom_id)),
                ("required", Value::Bool(field.required)),
            ],
        ),
        control,
    ];
    children.extend(hint);
    node(
        "box",
        vec![(
            "children",
            list(vec![node(
                "vstack",
                vec![("gap", text("xs")), ("children", list(children))],
            )]),
        )],
    )
}

fn section_node(section: &Value, collapsed: bool, gap: &str, ctx: &FieldContext) -> Value {
    let id = get_string(section, "id").unwrap_or_default();
    let title = get_string(section, "title").unwrap_or_default();
    let collapsible = get_bool_or(section, "collapsible", false);
    let mut header_fields = vec![(
        "className",
        text(if collapsible {
            "px-4 py-3 bg-muted flex items-center justify-between cursor-pointer hover:bg-muted/80"
        } else {
            "px-4 py-3 bg-muted flex items-center justify-between"
        }),
    )];
    let mut header_children = vec![node(
        "typography",
        vec![
            ("variant", text("label")),
            ("weight", text("semibold")),
            ("content", text(&title)),
        ],
    )];
    if collapsible {
        header_fields.push(("role", text("button")));
        header_fields.push(("tabIndex", Value::Int(0)));
        header_fields.push(("aria-expanded", Value::Bool(!collapsed)));
        header_fields.push(("action", text(format!("{TOGGLE}{id}"))));
        header_children.push(node(
            "icon",
            vec![
                ("name", text("chevron-down")),
                ("size", text("md")),
                (
                    "className",
                    text(if collapsed {
                        "text-muted-foreground transition-transform rotate-180"
                    } else {
                        "text-muted-foreground transition-transform"
                    }),
                ),
            ],
        ));
    }
    header_fields.push(("children", list(header_children)));
    let mut children = vec![
        node(
            "typography",
            vec![("content", text(&title)), ("className", text("sr-only"))],
        ),
        node("box", header_fields),
    ];
    if !collapsed {
        let fields: Vec<Value> = get_array(section, "fields")
            .iter()
            .filter_map(SchemaField::parse)
            .map(|field| field_node(&field, ctx))
            .collect();
        children.push(node(
            "box",
            vec![
                ("padding", text("md")),
                (
                    "children",
                    list(vec![node(
                        "vstack",
                        vec![("gap", text(gap)), ("children", list(fields))],
                    )]),
                ),
            ],
        ));
    }
    node(
        "box",
        vec![
            (
                "className",
                text("m-0 min-w-0 p-0 border border-border rounded-container overflow-hidden"),
            ),
            ("children", list(children)),
        ],
    )
}

fn payload_value(payload: &Value) -> Value {
    match payload {
        Value::Object(fields) => fields
            .get("value")
            .or_else(|| fields.get("checked"))
            .cloned()
            .unwrap_or(Value::Null),
        other => other.clone(),
    }
}

fn changed_value(mode: &str, payload: &Value) -> Option<Option<Value>> {
    let raw = payload_value(payload);
    match mode {
        "text" => Some(Some(text(js_string(&raw)))),
        "number" => {
            let entry = js_string(&raw);
            if entry.is_empty() {
                return Some(None);
            }
            entry
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
                .map(|number| Some(Value::Number(number)))
        }
        "checked" => Some(Some(Value::Bool(truthy(&raw)))),
        "relation" => {
            let selected = js_string(&raw);
            Some((!selected.is_empty()).then(|| text(selected)))
        }
        "list" => match raw {
            Value::Array(_) => Some(Some(raw)),
            Value::String(_) => Some(Some(list(vec![raw]))),
            _ => None,
        },
        _ => None,
    }
}

#[allow(non_snake_case)]
fn FormBody(body: FormBodyProps) -> Element {
    let props = &body.props;
    let view = body.view.as_view();
    let registry = &body.registry;
    let uid = use_hook(|| NEXT_FORM.fetch_add(1, Ordering::Relaxed));
    let initial = initial_data(props);
    let mut data = use_signal(|| initial.clone());
    let mut collapsed = use_signal(Vec::<String>::new);
    let last_initial = use_hook(|| Rc::new(RefCell::new(initial.clone())));
    if *last_initial.borrow() != initial {
        *last_initial.borrow_mut() = initial.clone();
        if !initial.is_empty() {
            data.set(initial.clone());
        }
    }
    let submit_event = get_string_or(props, "submitEvent", "SAVE");
    let cancel_event = get_string_or(props, "cancelEvent", "CANCEL");
    let on_submit = get_string(props, "onSubmit");
    let on_cancel = get_string(props, "onCancel");
    let sink = view.sink;
    let submit_initial = initial.clone();
    let form_sink = Callback::new(move |(event, payload): (String, Value)| {
        if let Some(rest) = event.strip_prefix(CHANGE) {
            let Some((mode, name)) = rest.split_once(':') else {
                return;
            };
            let Some(change) = changed_value(mode, &payload) else {
                return;
            };
            let mut next = data.read().clone();
            match &change {
                Some(value) => {
                    next.insert(name.into(), value.clone());
                }
                None => {
                    next.shift_remove(name);
                }
            }
            data.set(next.clone());
            if let Some(sink) = sink {
                sink.call((
                    "FIELD_CHANGED".to_string(),
                    object_value([
                        ("fieldId", text(name)),
                        ("value", change.unwrap_or(Value::Null)),
                        ("formValues", Value::object(next)),
                    ]),
                ));
            }
        } else if let Some(id) = event.strip_prefix(TOGGLE) {
            let mut next = collapsed.read().clone();
            if let Some(index) = next.iter().position(|entry| entry == id) {
                next.remove(index);
            } else {
                next.push(id.to_string());
            }
            collapsed.set(next);
        } else if event == SUBMIT {
            let mut merged = submit_initial.clone();
            for (key, value) in data.read().iter() {
                merged.insert(key.clone(), value.clone());
            }
            let payload = object_value([("data", Value::object(merged))]);
            if let Some(sink) = sink {
                sink.call((submit_event.clone(), payload.clone()));
                if let Some(extra) = &on_submit {
                    sink.call((extra.clone(), payload));
                }
            }
        } else if event == CANCEL {
            if let Some(sink) = sink {
                sink.call((cancel_event.clone(), Value::Null));
                sink.call(("CLOSE".to_string(), Value::Null));
                if let Some(extra) = &on_cancel {
                    sink.call((extra.clone(), Value::Null));
                }
            }
        }
    });
    let field_view = view.for_source(view.source_trait.clone(), form_sink);

    let layout = get_string_or(props, "layout", "vertical");
    let gap = get_string_or(props, "gap", "md");
    let layout_class = match layout.as_str() {
        "horizontal" => "flex flex-row flex-wrap items-end",
        "inline" => "flex flex-row flex-wrap items-center",
        _ => "flex flex-col",
    };
    let gap_class = match gap.as_str() {
        "sm" => "gap-2",
        "lg" => "gap-6",
        _ => "gap-4",
    };
    let stack_gap = match gap.as_str() {
        "sm" => "sm",
        "lg" => "lg",
        _ => "md",
    };
    let vertical = if layout == "horizontal" || layout == "inline" {
        ""
    } else {
        "w-full max-w-xl"
    };
    let paints_surface = get_string_or(props, "surface", "auto") == "auto"
        && try_use_context::<crate::components::core::atoms::card::OnSurface>().is_none();
    use_hook(|| {
        if paints_surface {
            provide_context(crate::components::core::atoms::card::OnSurface);
        }
    });
    let surface = if paints_surface {
        "surface-content p-card-md"
    } else {
        ""
    };
    let class = merge_class_name(
        &format!("{layout_class} {gap_class} {vertical} {surface}"),
        props,
    );

    let is_loading = get_bool_or(props, "isLoading", false);
    let relations = props.get("relationsData");
    let relations_loading = props.get("relationsLoading");
    let snapshot = data.read().clone();
    let ctx = FieldContext {
        uid,
        is_loading,
        data: &snapshot,
        relations,
        relations_loading,
    };

    let mut leading: Vec<Value> = Vec::new();
    if let Some(error) = props.get("error").filter(|error| truthy(error)) {
        let message = nonempty(get_string(error, "message"))
            .unwrap_or_else(|| "An error occurred".to_string());
        leading.push(node(
            "alert",
            vec![
                ("variant", text("error")),
                ("className", text("mb-4")),
                ("message", text(message)),
            ],
        ));
    }

    let sections = get_array(props, "sections");
    let collapsed_ids = collapsed.read().clone();
    let section_nodes: Vec<Value> = sections
        .iter()
        .map(|section| {
            let id = get_string(section, "id").unwrap_or_default();
            section_node(section, collapsed_ids.contains(&id), stack_gap, &ctx)
        })
        .collect();
    if !section_nodes.is_empty() {
        leading.push(node(
            "vstack",
            vec![
                ("gap", text(stack_gap)),
                ("children", list(section_nodes.clone())),
            ],
        ));
    }

    let fields = effective_fields(props);
    let field_nodes: Vec<Value> = fields.iter().map(|field| field_node(field, &ctx)).collect();
    let has_fields = !field_nodes.is_empty();

    let declared_fields = props
        .get("fields")
        .and_then(Value::as_array)
        .is_some_and(|fields| !fields.is_empty());
    let show_cancel = get_bool(props, "showCancel").unwrap_or(declared_fields);
    let show_submit = get_bool_or(props, "showSubmit", true);
    let mut actions = None;
    if (has_fields || !section_nodes.is_empty()) && (show_submit || show_cancel) {
        let mut buttons = Vec::new();
        if show_cancel {
            buttons.push(node(
                "button",
                vec![
                    ("variant", text("secondary")),
                    ("label", text(get_string_or(props, "cancelLabel", "Cancel"))),
                    ("disabled", Value::Bool(is_loading)),
                    ("action", text(CANCEL)),
                ],
            ));
        }
        if show_submit {
            buttons.push(node(
                "button",
                vec![
                    ("variant", text("primary")),
                    ("label", text(get_string_or(props, "submitLabel", "Save"))),
                    ("isLoading", Value::Bool(is_loading)),
                    ("action", text(SUBMIT)),
                ],
            ));
        }
        actions = Some(node(
            "hstack",
            vec![
                ("gap", text("sm")),
                ("justify", text("end")),
                ("wrap", Value::Bool(true)),
                (
                    "className",
                    text("sticky bottom-0 z-10 -mx-1 px-1 pt-4 pb-1 bg-card/95 surface-material"),
                ),
                ("children", list(buttons)),
            ],
        ));
    }
    let children = child_patterns(props, view.trait_content);

    rsx! {
        form {
            class: "{class}",
            "data-pattern": "form-section",
            onsubmit: move |event| {
                event.prevent_default();
                form_sink.call((SUBMIT.to_string(), Value::Null));
            },
            for spec in leading.iter() { {registry.render_spec(spec, None, &field_view)} }
            for spec in field_nodes.iter() { {registry.render_spec(spec, None, &field_view)} }
            for (child, src) in children.iter() { {view.render_child(registry, child, src.as_deref())} }
            if let Some(spec) = actions.as_ref() { {registry.render_spec(spec, None, &field_view)} }
        }
    }
}
