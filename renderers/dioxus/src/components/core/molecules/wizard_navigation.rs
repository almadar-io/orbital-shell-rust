use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, dom_passthrough, object_value, pattern_spec, DioxusPatternRenderer, DioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `wizard-navigation` pattern.
pub struct WizardNavigationRenderer;

fn step_button(
    variant: &str,
    label: &str,
    event: &str,
    payload: &Value,
    disabled: bool,
    left_icon: Option<&str>,
    right_icon: Option<&str>,
) -> Value {
    let mut fields = vec![
        ("variant", Value::string(variant)),
        ("label", Value::string(label)),
        ("action", Value::string(event)),
        ("actionPayload", payload.clone()),
        ("disabled", Value::Bool(disabled)),
    ];
    if let Some(icon) = left_icon {
        fields.push(("leftIcon", Value::string(icon)));
    }
    if let Some(icon) = right_icon {
        fields.push(("rightIcon", Value::string(icon)));
    }
    pattern_spec("button", fields)
}

impl DioxusPatternRenderer for WizardNavigationRenderer {
    fn pattern_name(&self) -> &str {
        "wizard-navigation"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let current = get_number_or(props, "currentStep", 0.0) as i64;
        let total = get_number_or(props, "totalSteps", 0.0) as i64;
        let is_valid = get_bool_or(props, "isValid", true);
        let show_back = get_bool_or(props, "showBack", true);
        let show_next = get_bool_or(props, "showNext", true);
        let show_complete = get_bool_or(props, "showComplete", true);
        let compact = get_bool_or(props, "compact", false);
        let back_label = get_string_or(props, "backLabel", &crate::i18n::t("wizard.back"));
        let next_label = get_string_or(props, "nextLabel", &crate::i18n::t("wizard.next"));
        let complete_label =
            get_string_or(props, "completeLabel", &crate::i18n::t("wizard.complete"));
        let on_back = get_string_or(props, "onBack", "WIZARD_BACK");
        let on_next = get_string_or(props, "onNext", "WIZARD_NEXT");
        let on_complete = get_string_or(props, "onComplete", "WIZARD_COMPLETE");
        let is_first = current == 0;
        let is_last = current == total - 1;
        let payload = object_value([
            ("currentStep", Value::Int(current)),
            ("totalSteps", Value::Int(total)),
        ]);
        let empty = || pattern_spec("box", [("className", Value::string(""))]);

        let back = if show_back {
            step_button(
                "secondary",
                &back_label,
                &on_back,
                &payload,
                is_first,
                Some("chevron-left"),
                None,
            )
        } else {
            empty()
        };
        let counter = pattern_spec(
            "hstack",
            [
                ("align", Value::string("center")),
                ("gap", Value::string("sm")),
                (
                    "children",
                    array_value(vec![pattern_spec(
                        "typography",
                        [
                            ("variant", Value::string("caption")),
                            ("color", Value::string("muted")),
                            (
                                "content",
                                Value::string(format!("Step {} of {}", current + 1, total)),
                            ),
                        ],
                    )]),
                ),
            ],
        );
        let forward = if is_last && show_complete {
            step_button(
                "primary",
                &complete_label,
                &on_complete,
                &payload,
                !is_valid,
                None,
                None,
            )
        } else if show_next {
            step_button(
                "primary",
                &next_label,
                &on_next,
                &payload,
                !is_valid,
                None,
                Some("chevron-right"),
            )
        } else {
            empty()
        };
        let class = cn(&[
            "border-t-heavy border-x-0 border-b-0 border-border flex flex-wrap gap-2 justify-between items-center",
            if compact { "px-4 py-2" } else { "px-6 py-4" },
            get_string(props, "className").as_deref().unwrap_or(""),
        ]);
        let mut root = vec![
            ("border".to_string(), Value::Bool(true)),
            ("className".to_string(), Value::string(class)),
            (
                "children".to_string(),
                array_value(vec![back, counter, forward]),
            ),
        ];
        root.extend(dom_passthrough(props));
        registry.render_spec(&pattern_spec("box", root), None, view)
    }
}
