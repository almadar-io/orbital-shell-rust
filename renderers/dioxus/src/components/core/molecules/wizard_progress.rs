use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, merge_class_name, pattern_spec, BOX_ATOM_BASE};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::{get_array, get_bool_or, get_number_or, get_string};
use orbital_core::{Value, ValueMap};

pub struct WizardProgressRenderer;

impl DioxusPatternRenderer for WizardProgressRenderer {
    fn pattern_name(&self) -> &str {
        "wizard-progress"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let steps = get_array(props, "steps");
        let current = get_number_or(props, "currentStep", 0.0) as usize;
        let total = steps.len();
        let compact = get_bool_or(props, "compact", false);
        let allow_navigation = get_bool_or(props, "allowNavigation", true);
        let step_click_event = get_string(props, "stepClickEvent");
        let class = merge_class_name(
            &cn(&[
                BOX_ATOM_BASE,
                "border-[length:var(--border-width)] border-border",
                "@container border-b-heavy border-x-0 border-t-0 border-border",
                if compact { "px-4 py-2" } else { "px-6 py-4" },
            ]),
            props,
        );
        let row_class = cn(&[BOX_ATOM_BASE, "flex items-center gap-2"]);

        rsx! {
            div { class: "{class}", ..shared_root_attrs(props),
                div { class: "{row_class}",
                    for (index, step) in steps.iter().enumerate() {
                        {
                            let title = if let Some(title) = step.as_str() {
                                title.to_string()
                            } else {
                                get_string(step, "title").unwrap_or_else(|| format!("Step {}", index + 1))
                            };
                            let active = index == current;
                            let completed = index < current;
                            let can_navigate = completed && allow_navigation && step_click_event.is_some();
                            let label = if completed { String::new() } else { (index + 1).to_string() };
                            let aria_label = format!("Step {} of {}: {}", index + 1, total, title);
                            let button_class = format!(
                                "flex-shrink-0 p-0 rounded-full text-sm tabular-nums border-heavy disabled:opacity-100 {} {} {}",
                                if compact { "w-6 h-6" } else { "w-8 h-8" },
                                if active || completed { "bg-primary text-primary-foreground border-primary" } else { "bg-card text-muted-foreground border-border" },
                                if can_navigate { "hover:bg-primary-hover" } else { "" },
                            );
                            let mut button = ValueMap::new();
                            button.insert("type".into(), Value::string("button"));
                            button.insert("label".into(), Value::string(label));
                            button.insert("variant".into(), Value::string("ghost"));
                            button.insert("disabled".into(), Value::Bool(!can_navigate));
                            button.insert("aria-label".into(), Value::string(aria_label));
                            button.insert("className".into(), Value::string(button_class));
                            if active { button.insert("aria-current".into(), Value::string("step")); }
                            if completed { button.insert("icon".into(), Value::string("check")); }
                            if can_navigate {
                                if let Some(event) = &step_click_event {
                                    let mut payload = ValueMap::new();
                                    payload.insert("stepIndex".into(), Value::Int(index as i64));
                                    button.insert("action".into(), Value::string(event.clone()));
                                    button.insert("actionPayload".into(), Value::Object(payload.into()));
                                }
                            }
                            let button = Value::Object(button.into());
                            let title_class = cn(&[
                                BOX_ATOM_BASE,
                                "min-w-0",
                                if active { "" } else { "hidden @md:block" },
                                if active { "text-foreground" } else { "text-muted-foreground" },
                            ]);
                            let title_spec = pattern_spec(
                                "typography",
                                [
                                    ("variant", Value::string("small")),
                                    ("weight", Value::string("medium")),
                                    ("truncate", Value::Bool(true)),
                                    ("content", Value::string(title.to_string())),
                                ],
                            );
                            let separator_class = cn(&[BOX_ATOM_BASE, "flex-1 min-w-4 h-0.5", if index < current { "bg-primary" } else { "bg-border" }]);
                            rsx! {
                                {registry.render_spec(&button, None, view)}
                                if !compact { div { class: "{title_class}", "data-step-title": "", {registry.render_spec(&title_spec, None, view)} } }
                                if index + 1 < total { div { aria_hidden: "true", class: "{separator_class}" } }
                            }
                        }
                    }
                }
            }
        }
    }
}
