use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{
    array_value, cn, object_value, pattern_spec, typography_spec, DioxusPatternRenderer,
    DioxusView, EventSink, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `option-constraint-group` pattern.
pub struct OptionConstraintGroupRenderer;

#[derive(Props, Clone, PartialEq)]
struct OptionConstraintGroupProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

const TOGGLE: &str = "__option_constraint_toggle:";

#[derive(Clone)]
enum Constraint {
    Single { required: bool },
    Multi { min: Option<i64>, max: Option<i64> },
}

#[derive(Clone)]
struct Choice {
    id: String,
    label: String,
    price_delta: Option<f64>,
    disabled: bool,
    out_of_stock: bool,
}

fn constraint(props: &Value) -> Constraint {
    let Some(spec) = props.get("constraint").filter(|spec| spec.is_object()) else {
        return Constraint::Single { required: false };
    };
    if get_string(spec, "type").as_deref() == Some("multi") {
        Constraint::Multi {
            min: get_number(spec, "min").map(|n| n as i64),
            max: get_number(spec, "max").map(|n| n as i64),
        }
    } else {
        Constraint::Single {
            required: get_bool_or(spec, "required", false),
        }
    }
}

fn hint(constraint: &Constraint) -> String {
    match constraint {
        Constraint::Single { required: true } => crate::i18n::t("optionConstraint.requiredOne"),
        Constraint::Single { required: false } => crate::i18n::t("optionConstraint.optionalOne"),
        Constraint::Multi { min, max } => {
            match (min.filter(|n| *n != 0), max.filter(|n| *n != 0)) {
                (Some(min), Some(max)) if min == max => format!("Pick exactly {min}"),
                (Some(min), Some(max)) => format!("Pick {min}-{max}"),
                (Some(min), None) => format!("Pick at least {min}"),
                (None, Some(max)) => format!("Pick up to {max}"),
                (None, None) => crate::i18n::t("optionConstraint.optional"),
            }
        }
    }
}

fn validate(selected: &[String], constraint: &Constraint) -> Option<String> {
    let count = selected.len() as i64;
    match constraint {
        Constraint::Single { required } => {
            if *required && selected.is_empty() {
                Some(crate::i18n::t("optionConstraint.error.pickOne"))
            } else if count > 1 {
                Some(crate::i18n::t("optionConstraint.error.pickOnlyOne"))
            } else {
                None
            }
        }
        Constraint::Multi { min, max } => {
            if let Some(min) = min.filter(|min| count < *min) {
                Some(format!("Pick at least {} more", min - count))
            } else {
                max.filter(|max| count > *max)
                    .map(|max| format!("Remove {} options", count - max))
            }
        }
    }
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
                    id: get_string_or(item, "id", ""),
                    label: get_string_or(item, "label", ""),
                    price_delta: get_number(item, "priceDelta"),
                    disabled: get_bool_or(item, "disabled", false),
                    out_of_stock: get_bool_or(item, "outOfStock", false),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn price_delta(delta: f64) -> String {
    format!(
        "{}${:.2}",
        if delta >= 0.0 { "+" } else { "-" },
        delta.abs()
    )
}

#[allow(non_snake_case)]
fn OptionConstraintGroup(input: OptionConstraintGroupProps) -> Element {
    let props = &input.props;
    let view = input.view.as_view();
    let registry = &input.registry;
    let group_id = get_string_or(props, "groupId", "");
    let title = get_string_or(props, "title", "");
    let description = get_string(props, "description").filter(|text| !text.is_empty());
    let size = get_string_or(props, "size", "md");
    let options = choices(props);
    let constraint = constraint(props);
    let selected = get_string_array(props, "selected");
    let change_event = get_string(props, "changeEvent").filter(|name| !name.is_empty());
    let sink = view.sink;
    let hint_text = hint(&constraint);
    let error = validate(&selected, &constraint);
    let input_name = format!("option-{group_id}");
    let option_gap = if size == "sm" { "gap-2" } else { "gap-2.5" };
    let single = matches!(constraint, Constraint::Single { .. });
    let required_single = matches!(constraint, Constraint::Single { required: true });
    let hint_id = format!("{group_id}-hint");
    let described_by = [get_string(props, "aria-describedby"), Some(hint_id.clone())]
        .into_iter()
        .flatten()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    let relay_selected = selected.clone();
    let relay: EventSink = Callback::new(move |(name, payload): (String, Value)| {
        let Some(id) = name.strip_prefix(TOGGLE) else {
            return;
        };
        let checked = get_bool(&payload, "checked").unwrap_or(true);
        let next = if single {
            if checked {
                vec![id.to_string()]
            } else {
                Vec::new()
            }
        } else if checked {
            if relay_selected.iter().any(|item| item == id) {
                return;
            }
            let mut next = relay_selected.clone();
            next.push(id.to_string());
            next
        } else {
            relay_selected
                .iter()
                .filter(|item| *item != id)
                .cloned()
                .collect()
        };
        if let (Some(sink), Some(event)) = (sink, &change_event) {
            sink.call((
                event.clone(),
                object_value([(
                    "selected",
                    array_value(next.into_iter().map(Value::string).collect()),
                )]),
            ));
        }
    });
    let relay_view = view.for_source(view.source_trait.clone(), relay);

    let class = cn(&[
        "space-y-2",
        get_string(props, "className").as_deref().unwrap_or(""),
    ]);
    let title_spec = pattern_spec(
        "typography",
        [
            ("variant", Value::string("label")),
            ("weight", Value::string("semibold")),
            ("content", Value::string(title)),
        ],
    );
    let hint_spec = pattern_spec(
        "typography",
        [
            ("variant", Value::string("caption")),
            (
                "color",
                Value::string(if required_single { "error" } else { "muted" }),
            ),
            ("id", Value::string(hint_id)),
            ("content", Value::string(hint_text)),
        ],
    );
    let description_spec =
        description.map(|text| typography_spec("caption", Some("muted"), "", &text));
    let error_spec = error.map(|text| typography_spec("caption", Some("error"), "", &text));
    let list_class = format!("flex flex-col {option_gap}");
    let text_size = if size == "sm" { "sm" } else { "md" };

    rsx! {
        fieldset { class: "{class}", "aria-describedby": "{described_by}", ..shared_root_attrs(props),
            div { class: "flex items-baseline justify-between gap-2",
                {registry.render_spec(&title_spec, None, &view)}
                {registry.render_spec(&hint_spec, None, &view)}
            }
            if let Some(spec) = description_spec.as_ref() { {registry.render_spec(spec, None, &view)} }
            div { class: "{list_class}",
                for option in options.iter() {
                    {
                        let is_checked = selected.iter().any(|item| item == &option.id);
                        let is_disabled = option.disabled || option.out_of_stock;
                        let option_id = format!("{input_name}-{}", option.id);
                        let label_class = cn(&[
                            "text-sm text-foreground flex items-center gap-2.5",
                            if is_disabled { "select-none font-normal cursor-not-allowed opacity-60" } else { "cursor-pointer select-none font-normal" },
                        ]);
                        let action = format!("{TOGGLE}{}", option.id);
                        let control = if single {
                            pattern_spec("radio", [
                                ("name", Value::string(input_name.clone())),
                                ("value", Value::string(option.id.clone())),
                                ("checked", Value::Bool(is_checked)),
                                ("disabled", Value::Bool(is_disabled)),
                                ("action", Value::string(action)),
                                ("aria-invalid", Value::Bool(error_spec.is_some())),
                            ])
                        } else {
                            pattern_spec("checkbox", [
                                ("checked", Value::Bool(is_checked)),
                                ("disabled", Value::Bool(is_disabled)),
                                ("onChange", Value::string(action)),
                                ("className", Value::string("h-4 w-4 accent-primary")),
                                ("aria-invalid", Value::Bool(error_spec.is_some())),
                            ])
                        };
                        let text = pattern_spec("typography", [
                            ("size", Value::string(text_size)),
                            ("className", Value::string(cn(&["flex-1 text-foreground", if option.out_of_stock { "line-through" } else { "" }]))),
                            ("content", Value::string(option.label.clone())),
                        ]);
                        let price = option
                            .price_delta
                            .filter(|delta| *delta != 0.0)
                            .map(|delta| typography_spec("caption", Some("muted"), "", &price_delta(delta)));
                        let stock = option.out_of_stock.then(|| {
                            pattern_spec("typography", [
                                ("variant", Value::string("caption")),
                                ("color", Value::string("warning")),
                                ("className", Value::string("rounded-interactive border border-warning/40 px-1.5 py-0.5")),
                                ("content", Value::string(crate::i18n::t("optionConstraint.outOfStock"))),
                            ])
                        });
                        rsx! {
                            label { key: "{option.id}", class: "{label_class}", r#for: "{option_id}",
                                {registry.render_spec(&control, None, &relay_view)}
                                {registry.render_spec(&text, None, &view)}
                                if let Some(spec) = price.as_ref() { {registry.render_spec(spec, None, &view)} }
                                if let Some(spec) = stock.as_ref() { {registry.render_spec(spec, None, &view)} }
                            }
                        }
                    }
                }
            }
            if let Some(spec) = error_spec.as_ref() {
                div { role: "alert", {registry.render_spec(spec, None, &view)} }
            }
        }
    }
}

impl DioxusPatternRenderer for OptionConstraintGroupRenderer {
    fn pattern_name(&self) -> &str {
        "option-constraint-group"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { OptionConstraintGroup { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
