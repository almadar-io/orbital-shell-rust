use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::pattern_spec as pattern;
use crate::renderer::{cn, merge_class_name, BOX_ATOM_BASE};
use crate::renderer::{
    emit_empty_event, DioxusPatternRenderer, DioxusView, EventSink, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_array, get_bool_or, get_number, get_string};
use orbital_core::client::recurse::child_patterns;
use orbital_core::{Value, ValueMap};

pub struct WizardContainerRenderer;

#[derive(Props, Clone, PartialEq)]
struct WizardContainerProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn step_event(sink: Option<EventSink>, name: Option<&str>, field: &str, step: usize) {
    if let (Some(sink), Some(name)) = (sink, name.filter(|name| !name.is_empty())) {
        let mut payload = ValueMap::new();
        payload.insert(field.into(), Value::Int(step as i64));
        sink.call((name.to_string(), Value::Object(payload.into())));
    }
}

fn button(
    label: &str,
    variant: &str,
    disabled: bool,
    action: &str,
    left_icon: Option<&str>,
    right_icon: Option<&str>,
) -> Value {
    let mut fields = vec![
        ("label", Value::string(label)),
        ("variant", Value::string(variant)),
        ("disabled", Value::Bool(disabled)),
        ("action", Value::string(action)),
    ];
    if let Some(icon) = left_icon {
        fields.push(("leftIcon", Value::string(icon)));
    }
    if let Some(icon) = right_icon {
        fields.push(("rightIcon", Value::string(icon)));
    }
    pattern("button", fields)
}

#[allow(non_snake_case)]
fn WizardContainer(props: WizardContainerProps) -> Element {
    let mut internal_step = use_signal(|| 0_usize);
    let steps = get_array(&props.props, "steps");
    let controlled = get_number(&props.props, "currentStep")
        .map(|number| number as usize)
        .or_else(|| {
            get_string(&props.props, "currentStep")?
                .parse::<usize>()
                .ok()
        });
    let current = controlled
        .unwrap_or_else(|| internal_step())
        .min(steps.len().saturating_sub(1));
    let is_first = current == 0;
    let is_last = current + 1 >= steps.len();
    let show_progress = get_bool_or(&props.props, "showProgress", true);
    let allow_back = get_bool_or(&props.props, "allowBack", true);
    let compact = get_bool_or(&props.props, "compact", false);
    let loading = get_bool_or(&props.props, "isLoading", false);
    let error = props
        .props
        .as_object()
        .and_then(|fields| fields.get("error"));
    let class = merge_class_name("flex flex-col h-full", &props.props);
    let view = props.view.as_view();

    if loading {
        let state = pattern("loading-state", [] as [(&str, Value); 0]);
        return rsx! { div { class: "{class}", ..shared_root_attrs(&props.props), {props.registry.render_spec(&state, None, &view)} } };
    }
    if let Some(error) = error.filter(|error| !matches!(error, Value::Null)) {
        let message = get_string(error, "message").unwrap_or_default();
        let state = pattern("error-state", [("message", Value::string(message))]);
        return rsx! { div { class: "{class}", ..shared_root_attrs(&props.props), {props.registry.render_spec(&state, None, &view)} } };
    }

    let next_event = get_string(&props.props, "nextEvent");
    let back_event = get_string(&props.props, "backEvent");
    let complete_event = get_string(&props.props, "completeEvent");
    let on_step_change = get_string(&props.props, "onStepChange");
    let on_complete = get_string(&props.props, "onComplete");
    let sink = view.sink;
    let total = steps.len();
    let relay: EventSink =
        Callback::new(
            move |(action, payload): (String, Value)| match action.as_str() {
                "__wizard_back" if allow_back && current > 0 => {
                    step_event(sink, back_event.as_deref(), "step", current - 1);
                    if controlled.is_none() {
                        internal_step.set(current - 1);
                    }
                    step_event(sink, on_step_change.as_deref(), "stepIndex", current - 1);
                }
                "__wizard_next" if current + 1 < total => {
                    step_event(sink, next_event.as_deref(), "step", current + 1);
                    if controlled.is_none() {
                        internal_step.set(current + 1);
                    }
                    step_event(sink, on_step_change.as_deref(), "stepIndex", current + 1);
                }
                "__wizard_complete" if total > 0 && is_last => {
                    step_event(sink, complete_event.as_deref(), "step", current);
                    emit_empty_event(sink, on_complete.as_deref());
                }
                "__wizard_step_click" if allow_back => {
                    if let Some(index) = get_number(&payload, "stepIndex") {
                        if index >= 0.0 && index.fract() == 0.0 && index < current as f64 {
                            if controlled.is_none() {
                                internal_step.set(index as usize);
                            }
                            step_event(
                                sink,
                                on_step_change.as_deref(),
                                "stepIndex",
                                index as usize,
                            );
                        }
                    }
                }
                _ => {}
            },
        );
    let action_view = view.for_source(view.source_trait.clone(), relay);
    let progress_steps: Vec<Value> = steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let mut fields = ValueMap::new();
            let id = get_string(step, "id")
                .or_else(|| get_string(step, "tabId"))
                .unwrap_or_else(|| format!("step-{index}"));
            let title = get_string(step, "title")
                .or_else(|| get_string(step, "name"))
                .unwrap_or_else(|| format!("Step {} of {}", index + 1, total));
            fields.insert("id".into(), Value::string(id));
            fields.insert("title".into(), Value::string(title));
            Value::Object(fields.into())
        })
        .collect();
    let mut progress_fields = vec![
        ("steps", Value::Array(progress_steps.into())),
        ("currentStep", Value::Int(current as i64)),
        ("compact", Value::Bool(compact)),
        ("allowNavigation", Value::Bool(allow_back)),
    ];
    progress_fields.push(("stepClickEvent", Value::string("__wizard_step_click")));
    let progress = pattern("wizard-progress", progress_fields);
    let step = steps.get(current);
    let title = step
        .and_then(|step| get_string(step, "title").or_else(|| get_string(step, "name")))
        .unwrap_or_else(|| format!("Step {} of {}", current + 1, total));
    let description = step.and_then(|step| get_string(step, "description"));
    let content = step.and_then(|step| step.as_object()?.get("content"));
    let children = child_patterns(&props.props, view.trait_content);
    let back = button(
        "Back",
        "secondary",
        is_first || !allow_back,
        "__wizard_back",
        Some("chevron-left"),
        None,
    );
    let next = button(
        if is_last { "Complete" } else { "Next" },
        "primary",
        total == 0,
        if is_last {
            "__wizard_complete"
        } else {
            "__wizard_next"
        },
        None,
        if is_last { None } else { Some("chevron-right") },
    );
    let header_class = cn(&[
        BOX_ATOM_BASE,
        "px-6 py-4 border-[length:var(--border-width)] border-border",
        "border-b-heavy border-x-0 border-t-0 border-border",
    ]);
    let content_class = cn(&[
        BOX_ATOM_BASE,
        "flex-1 overflow-auto",
        if compact { "p-4" } else { "p-6" },
    ]);
    let footer_class = cn(&[
        BOX_ATOM_BASE,
        "border-[length:var(--border-width)] border-border",
        "border-t-heavy border-x-0 border-b-0 border-border flex justify-between",
        if compact { "px-4 py-2" } else { "px-6 py-4" },
    ]);
    let title_spec = pattern(
        "typography",
        [
            ("variant", Value::string("h4")),
            ("as", Value::string("h2")),
            ("content", Value::string(title)),
        ],
    );
    let description_spec = description.map(|text| {
        pattern(
            "typography",
            [
                ("variant", Value::string("body2")),
                ("content", Value::string(text)),
                ("className", Value::string("text-muted-foreground mt-1")),
            ],
        )
    });
    let step_label = pattern(
        "typography",
        [
            ("variant", Value::string("caption")),
            (
                "content",
                Value::string(format!("Step {} of {}", current + 1, total)),
            ),
            ("className", Value::string("text-muted-foreground")),
        ],
    );
    let step_label_class = cn(&[BOX_ATOM_BASE, "flex items-center gap-2"]);

    rsx! {
        div { class: "{class}", ..shared_root_attrs(&props.props),
            if show_progress { {props.registry.render_spec(&progress, None, &action_view)} }
            if total > 0 {
                if !compact {
                    div { class: "{header_class}",
                        {props.registry.render_spec(&title_spec, None, &view)}
                        if let Some(description) = description_spec { {props.registry.render_spec(&description, None, &view)} }
                    }
                }
                div { class: "{content_class}",
                    if let Some(content) = content { {view.render_node(&props.registry, content)} }
                    else { for (child, source) in children.iter() { {view.render_child(&props.registry, child, source.as_deref())} } }
                }
                div { class: "{footer_class}",
                    {props.registry.render_spec(&back, None, &action_view)}
                    div { class: "{step_label_class}", {props.registry.render_spec(&step_label, None, &view)} }
                    {props.registry.render_spec(&next, None, &action_view)}
                }
            }
        }
    }
}

impl DioxusPatternRenderer for WizardContainerRenderer {
    fn pattern_name(&self) -> &str {
        "wizard-container"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { WizardContainer { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
