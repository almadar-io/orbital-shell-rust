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

const NODE_WIDTH: f64 = 180.0;
const NODE_HEIGHT: f64 = 56.0;
const NODE_GAP_Y: f64 = 80.0;
const GRAPH_PADDING: f64 = 32.0;

struct GraphNode {
    key: String,
    label: String,
    is_end: bool,
    x: f64,
    y: f64,
}

struct GraphEdge {
    key: String,
    path: String,
    label: String,
    box_x: f64,
    box_y: f64,
    box_width: f64,
    text_x: f64,
    text_y: f64,
}

struct GraphLayout {
    width: f64,
    height: f64,
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

fn clip_label(text: &str) -> String {
    if text.chars().count() > 22 {
        let head: String = text.chars().take(21).collect();
        format!("{head}\u{2026}")
    } else {
        text.to_string()
    }
}

fn edge_label(rule: &Rule) -> String {
    match &rule.value {
        RuleValue::Many(items) if rule.operator == "in" => format!("in [{}]", items.join(", ")),
        RuleValue::Many(items) => format!("{} {}", rule.operator, items.join(", ")),
        RuleValue::One(text) => format!("{} {text}", rule.operator),
    }
}

fn graph_layout(questions: &[Question], rules: &[Rule], end_label: &str) -> GraphLayout {
    let items: Vec<(&str, &str, bool)> = questions
        .iter()
        .map(|q| (q.id.as_str(), q.label.as_str(), false))
        .chain(std::iter::once((END_OF_SURVEY, end_label, true)))
        .collect();
    let mut positions: std::collections::HashMap<&str, (f64, f64)> = Default::default();
    for (i, (id, _, _)) in items.iter().enumerate() {
        positions.insert(
            id,
            (
                GRAPH_PADDING,
                GRAPH_PADDING + i as f64 * (NODE_HEIGHT + NODE_GAP_Y),
            ),
        );
    }
    let nodes = items
        .iter()
        .map(|(id, label, is_end)| {
            let (x, y) = positions[id];
            GraphNode {
                key: id.to_string(),
                label: clip_label(label),
                is_end: *is_end,
                x,
                y,
            }
        })
        .collect();
    let edges = rules
        .iter()
        .enumerate()
        .filter_map(|(i, rule)| {
            let from = positions.get(rule.source.as_str())?;
            let to = positions.get(rule.target.as_str())?;
            let start_x = from.0 + NODE_WIDTH;
            let start_y = from.1 + NODE_HEIGHT / 2.0;
            let end_x = to.0 + NODE_WIDTH;
            let end_y = to.1 + NODE_HEIGHT / 2.0;
            let offset = 40.0 + (i % 4) as f64 * 30.0;
            let cx = start_x.max(end_x) + offset;
            let label = edge_label(rule);
            let label_y = (start_y + end_y) / 2.0;
            Some(GraphEdge {
                key: rule.id.clone(),
                path: format!(
                    "M {start_x} {start_y} C {cx} {start_y}, {cx} {end_y}, {end_x} {end_y}"
                ),
                box_x: cx - 4.0,
                box_y: label_y - 10.0,
                box_width: (label.chars().count() as f64 * 7.0 + 12.0).min(160.0),
                text_x: cx + 4.0,
                text_y: label_y + 4.0,
                label: clip_label(&label),
            })
        })
        .collect();
    GraphLayout {
        width: NODE_WIDTH + GRAPH_PADDING * 2.0 + 220.0,
        height: GRAPH_PADDING * 2.0 + items.len() as f64 * (NODE_HEIGHT + NODE_GAP_Y),
        nodes,
        edges,
    }
}

fn logic_graph(layout: &GraphLayout) -> Element {
    rsx! {
        div { class: "overflow-auto p-2",
            svg {
                width: "{layout.width}",
                height: "{layout.height}",
                role: "img",
                "aria-label": "Branching logic graph",
                style: "display: block",
                defs {
                    marker {
                        id: "branching-arrowhead",
                        view_box: "0 0 10 10",
                        ref_x: "8",
                        ref_y: "5",
                        marker_width: "8",
                        marker_height: "8",
                        orient: "auto",
                        path { d: "M 0 0 L 10 5 L 0 10 z", fill: "var(--color-muted-foreground)" }
                    }
                }
                for edge in layout.edges.iter() {
                    g { key: "{edge.key}",
                        path {
                            d: "{edge.path}",
                            fill: "none",
                            stroke: "var(--color-muted-foreground)",
                            stroke_width: "1.5",
                            marker_end: "url(#branching-arrowhead)",
                        }
                        rect {
                            x: "{edge.box_x}",
                            y: "{edge.box_y}",
                            width: "{edge.box_width}",
                            height: "20",
                            rx: "4",
                            fill: "var(--color-card)",
                            stroke: "var(--color-border)",
                        }
                        text {
                            x: "{edge.text_x}",
                            y: "{edge.text_y}",
                            font_size: "11",
                            font_family: "JetBrains Mono, monospace",
                            fill: "var(--color-foreground)",
                            "{edge.label}"
                        }
                    }
                }
                for node in layout.nodes.iter() {
                    g { key: "{node.key}",
                        rect {
                            x: "{node.x}",
                            y: "{node.y}",
                            width: "{NODE_WIDTH}",
                            height: "{NODE_HEIGHT}",
                            rx: "6",
                            fill: if node.is_end { "var(--color-surface)" } else { "var(--color-card)" },
                            stroke: if node.is_end { "var(--color-error)" } else { "var(--color-border)" },
                            stroke_width: if node.is_end { "2" } else { "1.5" },
                        }
                        text {
                            x: "{node.x + NODE_WIDTH / 2.0}",
                            y: "{node.y + NODE_HEIGHT / 2.0 + 4.0}",
                            text_anchor: "middle",
                            font_size: "13",
                            font_family: "Inter, sans-serif",
                            fill: "var(--color-foreground)",
                            font_weight: "600",
                            "{node.label}"
                        }
                    }
                }
            }
        }
    }
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
                {logic_graph(&graph_layout(&questions, &current, "End of survey"))}
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

#[cfg(test)]
mod tests {
    use super::*;

    fn question(id: &str, label: &str) -> Question {
        Question {
            id: id.into(),
            label: label.into(),
            option_values: None,
        }
    }

    fn rule(id: &str, source: &str, op: &str, value: RuleValue, target: &str) -> Rule {
        Rule {
            id: id.into(),
            source: source.into(),
            operator: op.into(),
            value,
            target: target.into(),
        }
    }

    #[test]
    fn graph_has_a_node_per_question_plus_end_and_a_bezier_per_rule() {
        let questions = [question("q1", "First"), question("q2", "Second")];
        let rules = [
            rule("r1", "q1", "equals", RuleValue::One("yes".into()), "q2"),
            rule(
                "r2",
                "q2",
                "in",
                RuleValue::Many(vec!["a".into(), "b".into()]),
                END_OF_SURVEY,
            ),
        ];
        let layout = graph_layout(&questions, &rules, "End of survey");
        assert_eq!((layout.width, layout.height), (464.0, 472.0));
        let nodes: Vec<_> = layout
            .nodes
            .iter()
            .map(|n| (n.key.as_str(), n.x, n.y, n.is_end))
            .collect();
        assert_eq!(
            nodes,
            vec![
                ("q1", 32.0, 32.0, false),
                ("q2", 32.0, 168.0, false),
                (END_OF_SURVEY, 32.0, 304.0, true)
            ]
        );
        assert_eq!(layout.nodes[2].label, "End of survey");
        assert_eq!(layout.edges.len(), 2);
        assert_eq!(layout.edges[0].path, "M 212 60 C 252 60, 252 196, 212 196");
        assert_eq!(layout.edges[0].label, "equals yes");
        assert_eq!(
            (
                layout.edges[0].box_x,
                layout.edges[0].box_y,
                layout.edges[0].box_width
            ),
            (248.0, 118.0, 82.0)
        );
        assert_eq!(
            (layout.edges[0].text_x, layout.edges[0].text_y),
            (256.0, 132.0)
        );
        assert_eq!(
            layout.edges[1].path,
            "M 212 196 C 282 196, 282 332, 212 332"
        );
        assert_eq!(layout.edges[1].label, "in [a, b]");
    }

    #[test]
    fn control_a_rule_with_a_missing_endpoint_draws_no_edge_and_no_rules_draw_only_nodes() {
        let questions = [question("q1", "First")];
        let broken = [rule(
            "r1",
            "q1",
            "contains",
            RuleValue::One("x".into()),
            "gone",
        )];
        assert!(graph_layout(&questions, &broken, "End of survey")
            .edges
            .is_empty());
        let empty = graph_layout(&questions, &[], "End of survey");
        assert!(empty.edges.is_empty());
        assert_eq!(empty.nodes.len(), 2);
    }

    #[test]
    fn labels_clip_at_twenty_two_characters_as_the_web_does() {
        assert_eq!(
            clip_label("abcdefghijklmnopqrstuv"),
            "abcdefghijklmnopqrstuv"
        );
        assert_eq!(
            clip_label("abcdefghijklmnopqrstuvw"),
            "abcdefghijklmnopqrstu\u{2026}"
        );
        let many = rule(
            "r",
            "a",
            "contains",
            RuleValue::Many(vec!["x".into(), "y".into()]),
            "b",
        );
        assert_eq!(edge_label(&many), "contains x, y");
    }
}
