use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    array_value, cn, layout_box_spec, merge_class_name, object_value, pattern_spec,
    render_with_local_sink, DioxusPatternRenderer, DioxusView, EventSink, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

const END_OF_SURVEY: &str = "end-of-survey";
const SEP: char = '\u{1f}';
const OPERATORS: [&str; 4] = ["equals", "not-equals", "contains", "in"];

static RULE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Dioxus renderer for the `branching-logic-builder` pattern.
pub struct BranchingLogicBuilderRenderer;

impl BranchingLogicBuilderRenderer {
    const NAME: &str = "branching-logic-builder";
}

#[derive(Clone, PartialEq)]
struct Question {
    id: String,
    label: String,
    option_values: Option<Vec<String>>,
}

#[derive(Clone, PartialEq)]
enum RuleValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Clone, PartialEq)]
struct Rule {
    id: String,
    source: String,
    operator: String,
    value: RuleValue,
    target: String,
}

fn text_list(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_string))
        .collect()
}

fn question_of(item: &Value) -> Option<Question> {
    let option_values = match item.get("optionValues") {
        None | Some(Value::Null) => None,
        Some(values) => Some(text_list(values)?),
    };
    Some(Question {
        id: get_string(item, "id")?,
        label: get_string(item, "label")?,
        option_values,
    })
}

fn rule_of(item: &Value) -> Option<Rule> {
    let operator = get_string(item, "operator").filter(|op| OPERATORS.contains(&op.as_str()))?;
    let value = match item.get("value")? {
        Value::String(text) => RuleValue::One(text.to_string()),
        other => RuleValue::Many(text_list(other)?),
    };
    Some(Rule {
        id: get_string(item, "id")?,
        source: get_string(item, "sourceQuestionId")?,
        operator,
        value,
        target: get_string(item, "targetQuestionId")?,
    })
}

fn questions_of(props: &Value) -> Vec<Question> {
    get_array(props, "questions")
        .iter()
        .map(question_of)
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

fn rules_of(props: &Value) -> Vec<Rule> {
    get_array(props, "rules")
        .iter()
        .map(rule_of)
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

fn rule_value(rule: &Rule) -> Value {
    let value = match &rule.value {
        RuleValue::One(text) => Value::string(text.to_string()),
        RuleValue::Many(items) => array_value(
            items
                .iter()
                .map(|item| Value::string(item.to_string()))
                .collect(),
        ),
    };
    object_value([
        ("id", Value::string(rule.id.to_string())),
        ("sourceQuestionId", Value::string(rule.source.to_string())),
        ("operator", Value::string(rule.operator.to_string())),
        ("value", value),
        ("targetQuestionId", Value::string(rule.target.to_string())),
    ])
}

fn is_broken(rule: &Rule, questions: &[Question]) -> bool {
    let source_exists = questions.iter().any(|q| q.id == rule.source);
    let target_exists =
        rule.target == END_OF_SURVEY || questions.iter().any(|q| q.id == rule.target);
    !source_exists || !target_exists
}

fn new_rule_id() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default();
    format!(
        "rule-{millis}-{:x}",
        RULE_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn event_name(kind: &str, rest: &str) -> String {
    format!("{kind}{SEP}{rest}")
}

#[derive(Clone)]
struct Ctx {
    rules: Signal<Vec<Rule>>,
    graph: Signal<bool>,
    questions: Rc<Vec<Question>>,
    parent: Option<EventSink>,
    change_event: Option<String>,
}

impl Ctx {
    fn update(&self, next: Vec<Rule>) {
        let mut rules = self.rules;
        rules.set(next.clone());
        if let (Some(parent), Some(event)) = (self.parent, &self.change_event) {
            let payload =
                object_value([("rules", array_value(next.iter().map(rule_value).collect()))]);
            parent.call((event.clone(), payload));
        }
    }

    fn handle(&self, event: &str, payload: &Value) -> bool {
        let Some((kind, rest)) = event.split_once(SEP) else {
            return false;
        };
        let current = self.rules.read().clone();
        let payload_text = |key: &str| get_string(payload, key).unwrap_or_default();
        match kind {
            "view" => {
                let mut graph = self.graph;
                graph.set(rest == "graph");
            }
            "add" => {
                let Some(first) = self.questions.first() else {
                    return true;
                };
                let target = self
                    .questions
                    .get(1)
                    .map(|q| q.id.to_string())
                    .unwrap_or_else(|| END_OF_SURVEY.to_string());
                let mut next = current;
                next.push(Rule {
                    id: new_rule_id(),
                    source: first.id.to_string(),
                    operator: "equals".to_string(),
                    value: RuleValue::One(
                        first
                            .option_values
                            .as_ref()
                            .and_then(|values| values.first().cloned())
                            .unwrap_or_default(),
                    ),
                    target,
                });
                self.update(next);
            }
            "del" => {
                self.update(current.into_iter().filter(|r| r.id != rest).collect());
            }
            "src" | "op" | "val" | "tgt" | "chipadd" | "chiprm" => {
                let next = current
                    .into_iter()
                    .map(|mut rule| {
                        if rule.id != rest {
                            return rule;
                        }
                        match kind {
                            "src" => rule.source = payload_text("value"),
                            "tgt" => rule.target = payload_text("value"),
                            "val" => rule.value = RuleValue::One(payload_text("value")),
                            "op" => {
                                let next_op = payload_text("value");
                                rule.value = match (&rule.value, next_op == "in") {
                                    (RuleValue::One(text), true) => {
                                        RuleValue::Many(if text.is_empty() {
                                            Vec::new()
                                        } else {
                                            vec![text.to_string()]
                                        })
                                    }
                                    (RuleValue::Many(items), false) => {
                                        RuleValue::One(items.first().cloned().unwrap_or_default())
                                    }
                                    (other, _) => other.clone(),
                                };
                                rule.operator = next_op;
                            }
                            "chipadd" => {
                                let added = payload_text("value");
                                let mut items = match &rule.value {
                                    RuleValue::Many(items) => items.clone(),
                                    RuleValue::One(_) => Vec::new(),
                                };
                                if !added.is_empty() && !items.contains(&added) {
                                    items.push(added);
                                    rule.value = RuleValue::Many(items);
                                }
                            }
                            _ => {
                                let removed = get_string(payload, "label").unwrap_or_default();
                                let items = match &rule.value {
                                    RuleValue::Many(items) => items.clone(),
                                    RuleValue::One(_) => Vec::new(),
                                };
                                rule.value = RuleValue::Many(
                                    items.into_iter().filter(|c| *c != removed).collect(),
                                );
                            }
                        }
                        rule
                    })
                    .collect::<Vec<_>>();
                self.update(next);
            }
            _ => return false,
        }
        true
    }
}

fn typography(
    variant: &str,
    color: Option<&str>,
    weight: Option<&str>,
    class: &str,
    content: Value,
) -> Value {
    let mut fields = vec![("variant", Value::string(variant))];
    if let Some(color) = color {
        fields.push(("color", Value::string(color)));
    }
    if let Some(weight) = weight {
        fields.push(("weight", Value::string(weight)));
    }
    if !class.is_empty() {
        fields.push(("className", Value::string(class)));
    }
    fields.push((
        if matches!(content, Value::Array(_)) {
            "children"
        } else {
            "content"
        },
        content,
    ));
    pattern_spec("typography", fields)
}

fn options_value(items: impl IntoIterator<Item = (String, String)>) -> Value {
    array_value(
        items
            .into_iter()
            .map(|(value, label)| {
                object_value([
                    ("value", Value::string(value)),
                    ("label", Value::string(label)),
                ])
            })
            .collect(),
    )
}

fn badge(variant: &str, label: String) -> Value {
    pattern_spec(
        "badge",
        [
            ("variant", Value::string(variant)),
            ("size", Value::string("sm")),
            ("label", Value::string(label)),
        ],
    )
}

fn button(variant: &str, icon: &str, label: &str, action: String, disabled: bool) -> Value {
    pattern_spec(
        "button",
        [
            ("variant", Value::string(variant)),
            ("size", Value::string("sm")),
            ("leftIcon", Value::string(icon)),
            ("label", Value::string(label)),
            ("action", Value::string(action)),
            ("disabled", Value::Bool(disabled)),
        ],
    )
}

fn select(
    options: Value,
    value: &str,
    placeholder: &str,
    event: String,
    disabled: bool,
    error: Option<&str>,
) -> Value {
    let mut fields = vec![
        ("options", options),
        ("value", Value::string(value.to_string())),
        ("onValueChange", Value::string(event)),
        ("disabled", Value::Bool(disabled)),
    ];
    if !placeholder.is_empty() {
        fields.push(("placeholder", Value::string(placeholder)));
    }
    if let Some(error) = error {
        fields.push(("error", Value::string(error)));
    }
    pattern_spec("select", fields)
}

fn text_input(placeholder: &str, value: &str, disabled: bool, extra: (&str, Value)) -> Value {
    pattern_spec(
        "input",
        [
            ("inputType", Value::string("text")),
            ("placeholder", Value::string(placeholder)),
            ("value", Value::string(value.to_string())),
            ("disabled", Value::Bool(disabled)),
            extra,
        ],
    )
}

fn flex_box(class: &str, children: Vec<Value>) -> Value {
    layout_box_spec(class, None, Vec::new(), children)
}

fn rule_row(rule: &Rule, questions: &[Question], read_only: bool, broken: bool) -> Value {
    let id = rule.id.as_str();
    let source_options = options_value(
        questions
            .iter()
            .map(|q| (q.id.to_string(), q.label.to_string())),
    );
    let target_options = options_value(
        questions
            .iter()
            .map(|q| (q.id.to_string(), q.label.to_string()))
            .chain([(END_OF_SURVEY.to_string(), "End of survey".to_string())]),
    );
    let operator_options = options_value(
        [
            ("equals", "equals"),
            ("not-equals", "does not equal"),
            ("contains", "contains"),
            ("in", "is one of"),
        ]
        .map(|(value, label)| (value.to_string(), label.to_string())),
    );
    let value_options: Vec<String> = questions
        .iter()
        .find(|q| q.id == rule.source)
        .and_then(|q| q.option_values.clone())
        .unwrap_or_default();
    let chips: Vec<String> = match &rule.value {
        RuleValue::Many(items) => items.clone(),
        RuleValue::One(_) => Vec::new(),
    };
    let scalar = match &rule.value {
        RuleValue::One(text) => text.as_str(),
        RuleValue::Many(_) => "",
    };
    let broken_text = "Broken reference";

    let value_cell = if rule.operator == "in" {
        let mut cell: Vec<Value> = chips
            .iter()
            .map(|chip| {
                let mut fields = vec![
                    ("variant", Value::string("primary")),
                    ("size", Value::string("sm")),
                    ("label", Value::string(chip.to_string())),
                    ("removable", Value::Bool(!read_only)),
                ];
                if !read_only {
                    fields.push(("removeEvent", Value::string(event_name("chiprm", id))));
                }
                pattern_spec("filter-pill", fields)
            })
            .collect();
        if !value_options.is_empty() {
            let remaining = options_value(
                value_options
                    .iter()
                    .filter(|v| !chips.contains(v))
                    .map(|v| (v.to_string(), v.to_string())),
            );
            cell.push(flex_box(
                "min-w-[8rem]",
                vec![select(
                    remaining,
                    "",
                    "Add value",
                    event_name("chipadd", id),
                    read_only,
                    None,
                )],
            ));
        } else {
            cell.push(text_input(
                "Type value, press Enter",
                "",
                read_only,
                ("action", Value::string(event_name("chipadd", id))),
            ));
        }
        flex_box(
            "flex min-w-[10rem] grow basis-40 flex-wrap items-center gap-1",
            cell,
        )
    } else {
        let inner = if !value_options.is_empty() {
            select(
                options_value(value_options.iter().map(|v| (v.to_string(), v.to_string()))),
                scalar,
                "Select value",
                event_name("val", id),
                read_only,
                None,
            )
        } else {
            text_input(
                "Value",
                scalar,
                read_only,
                ("onChange", Value::string(event_name("val", id))),
            )
        };
        flex_box("min-w-[8rem] grow basis-32", vec![inner])
    };

    let mut row = vec![
        typography(
            "label",
            None,
            Some("semibold"),
            "shrink-0",
            Value::string("If"),
        ),
        flex_box(
            "min-w-[10rem] grow basis-40",
            vec![select(
                source_options,
                &rule.source,
                "Select question",
                event_name("src", id),
                read_only,
                broken.then_some(broken_text),
            )],
        ),
        flex_box(
            "min-w-[8rem] basis-32",
            vec![select(
                operator_options,
                &rule.operator,
                "",
                event_name("op", id),
                read_only,
                None,
            )],
        ),
        value_cell,
        typography(
            "label",
            None,
            Some("semibold"),
            "shrink-0 inline-flex items-center gap-1",
            array_value(vec![
                pattern_spec(
                    "icon",
                    [
                        ("name", Value::string("arrow-right")),
                        ("className", Value::string("h-4 w-4")),
                    ],
                ),
                Value::string("go to"),
            ]),
        ),
        flex_box(
            "min-w-[10rem] grow basis-40",
            vec![select(
                target_options,
                &rule.target,
                "Select target",
                event_name("tgt", id),
                read_only,
                (broken && rule.target != END_OF_SURVEY).then_some(broken_text),
            )],
        ),
    ];
    if !read_only {
        row.push(pattern_spec(
            "button",
            [
                ("variant", Value::string("ghost")),
                ("size", Value::string("sm")),
                ("leftIcon", Value::string("trash")),
                ("action", Value::string(event_name("del", id))),
                ("aria-label", Value::string("Delete rule")),
            ],
        ));
    }
    let mut body = vec![flex_box("flex flex-wrap items-center gap-2", row)];
    if broken {
        body.push(badge("error", broken_text.to_string()));
    }
    pattern_spec(
        "card",
        [
            ("variant", Value::string("bordered")),
            ("padding", Value::string("sm")),
            (
                "className",
                Value::string(cn(&[
                    "flex flex-col gap-2",
                    if broken { "border-error" } else { "" },
                ])),
            ),
            ("children", array_value(body)),
        ],
    )
}

fn header(rule_count: usize, broken_count: usize, graph: bool) -> Value {
    let count_label = if rule_count == 1 {
        format!("{rule_count} rule")
    } else {
        format!("{rule_count} rules")
    };
    let mut title = vec![
        pattern_spec(
            "icon",
            [
                ("name", Value::string("git-branch")),
                ("className", Value::string("h-5 w-5 text-foreground")),
            ],
        ),
        typography(
            "subheading",
            None,
            Some("semibold"),
            "",
            Value::string("Branching logic"),
        ),
        badge("neutral", count_label),
    ];
    if broken_count > 0 {
        title.push(badge("error", format!("{broken_count} broken")));
    }
    flex_box(
        "flex flex-wrap items-center justify-between gap-2",
        vec![
            flex_box("flex items-center gap-2", title),
            flex_box(
                "flex items-center gap-1 rounded-container border border-border bg-card p-0.5",
                vec![
                    button(
                        if graph { "ghost" } else { "primary" },
                        "pencil",
                        "Rules",
                        event_name("view", "edit"),
                        false,
                    ),
                    button(
                        if graph { "primary" } else { "ghost" },
                        "eye",
                        "Logic graph",
                        event_name("view", "graph"),
                        false,
                    ),
                ],
            ),
        ],
    )
}

#[derive(Props, Clone, PartialEq)]
struct BuilderProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn Builder(input: BuilderProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let questions = questions_of(props);
    let incoming = rules_of(props);
    let last = use_hook(|| Rc::new(RefCell::new(incoming.clone())));
    let mut rules = use_signal(|| incoming.clone());
    let graph = use_signal(|| false);
    if *last.borrow() != incoming {
        *last.borrow_mut() = incoming.clone();
        rules.set(incoming);
    }
    let read_only = get_bool_or(props, "readOnly", false);
    let current = rules.read().clone();
    let broken_count = current.iter().filter(|r| is_broken(r, &questions)).count();
    let no_questions = questions.is_empty();
    let show_graph = graph();
    let ctx = Ctx {
        rules,
        graph,
        questions: Rc::new(questions.clone()),
        parent: view.sink,
        change_event: get_string(props, "rulesChangeEvent").filter(|event| !event.is_empty()),
    };
    let local = |spec: &Value| {
        let ctx = ctx.clone();
        render_with_local_sink(&view, registry, spec, move |event, payload| {
            ctx.handle(event, payload)
        })
    };
    let class = merge_class_name("flex flex-col gap-3", props);
    let header_el = local(&header(current.len(), broken_count, show_graph));
    let empty_el = local(&pattern_spec(
        "card",
        [
            ("variant", Value::string("bordered")),
            ("padding", Value::string("lg")),
            ("className", Value::string("text-center")),
            (
                "children",
                array_value(vec![typography(
                    "body",
                    Some("muted"),
                    None,
                    "",
                    Value::string(if no_questions {
                        "Add questions before building branching rules."
                    } else {
                        "No rules yet. Add a rule to define branching logic."
                    }),
                )]),
            ),
        ],
    ));
    let rows = current
        .iter()
        .map(|rule| {
            let chip_count = match &rule.value {
                RuleValue::Many(items) => items.len(),
                RuleValue::One(_) => 0,
            };
            (
                format!("{}:{chip_count}", rule.id),
                local(&rule_row(
                    rule,
                    &questions,
                    read_only,
                    is_broken(rule, &questions),
                )),
            )
        })
        .collect::<Vec<_>>();
    let add_el = local(&flex_box(
        "",
        vec![button(
            "secondary",
            "plus",
            "Add rule",
            event_name("add", ""),
            no_questions,
        )],
    ));
    rsx! {
        div { class: "{class}", ..shared_root_attrs(props),
            {header_el}
            if show_graph {
                div { class: "overflow-auto p-2" }
            } else {
                div { class: "flex flex-col gap-2",
                    if current.is_empty() { {empty_el} }
                    for (key, row) in rows.into_iter() {
                        Fragment { key: "{key}", {row} }
                    }
                    if !read_only { {add_el} }
                }
            }
        }
    }
}

impl DioxusPatternRenderer for BranchingLogicBuilderRenderer {
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
        rsx! { Builder { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
