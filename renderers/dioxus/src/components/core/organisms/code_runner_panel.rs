//! Native Dioxus renderer for the `code-runner-panel` pattern (web: CodeRunnerPanel.tsx).
//! Text is the web's default-locale (en) catalog. The code is shown read-only: the `code-block` body has no editor.

use crate::components::core::molecules::agent_activity_row::{
    field, plain_text_of, render_pattern, typography,
};
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{object_value, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct CodeRunnerPanelRenderer;

impl CodeRunnerPanelRenderer {
    const NAME: &str = "code-runner-panel";
}

fn test_row(
    index: usize,
    test: &Value,
    registry: &DioxusPatternRegistry,
    view: &DioxusView<'_>,
) -> Element {
    let passed = get_bool_or(test, "passed", false);
    let number = index + 1;
    let title = if passed {
        format!("Test {number}: passed")
    } else {
        format!("Test {number}: failed")
    };
    let line = |text: String| {
        typography(
            registry,
            view,
            "small",
            &text,
            None,
            None,
            Some("text-background"),
        )
    };
    rsx! {
        div { class: "flex flex-row gap-2 items-start text-xs",
            {render_pattern(registry, view, "icon", vec![
                ("name", Value::string(if passed { "circle-check" } else { "circle-x" })),
                ("size", Value::string("sm")),
                ("color", Value::string(if passed { "success" } else { "error" })),
                ("className", Value::string("mt-0.5")),
            ])}
            div { class: "flex flex-col gap-1 items-stretch justify-start flex-1",
                {typography(registry, view, "small", &title, None, None, Some(if passed { "text-success" } else { "text-error" }))}
                {line(format!("Input: {}", plain_text_of(field(test, "input"))))}
                {line(format!("Expected: {}", plain_text_of(field(test, "expectedOutput"))))}
                {line(format!("Actual: {}", plain_text_of(field(test, "actualOutput"))))}
            }
        }
    }
}

impl DioxusPatternRenderer for CodeRunnerPanelRenderer {
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
        let code = get_string_or(props, "code", "");
        let language = get_string_or(props, "language", "");
        let class_name = get_string_or(props, "className", "");
        let block = |extra: Vec<(&str, Value)>| {
            let mut fields = vec![
                ("language", Value::string(language.as_str())),
                ("code", Value::string(code.as_str())),
            ];
            fields.extend(extra);
            render_pattern(registry, view, "code-block", fields)
        };
        if !get_bool_or(props, "runnable", false) {
            return rsx! { div { class: "{class_name}", ..shared_root_attrs(props), {block(vec![])} } };
        }
        let run_event = get_string_or(props, "runEvent", "RUN_CODE");
        let reset_event = get_string(props, "resetEvent").filter(|name| !name.is_empty());
        let run_id = get_string_or(props, "runId", "");
        let running = get_bool_or(props, "running", false);
        let output = field(props, "output").filter(|output| output.as_object().is_some());
        let error = field(props, "error").filter(|error| error.as_str().is_some());
        let has_output = output.is_some() || error.is_some();
        let root_class = format!("space-y-3 {class_name}");

        let copy_button = render_pattern(
            registry,
            view,
            "button",
            vec![
                ("variant", Value::string("ghost")),
                ("size", Value::string("sm")),
                ("icon", Value::string("copy")),
                ("label", Value::string("")),
                ("aria-label", Value::string(crate::i18n::t("common.copy"))),
            ],
        );
        let mut reset_fields = vec![
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("icon", Value::string("rotate-ccw")),
            ("label", Value::string("")),
            ("disabled", Value::Bool(running)),
            ("aria-label", Value::string(crate::i18n::t("common.reset"))),
        ];
        if let Some(event) = &reset_event {
            reset_fields.push(("action", Value::string(event.as_str())));
            reset_fields.push((
                "actionPayload",
                object_value([("runId", Value::string(run_id.as_str()))]),
            ));
        }
        let reset_button = render_pattern(registry, view, "button", reset_fields);
        let run_button = render_pattern(
            registry,
            view,
            "button",
            vec![
                ("variant", Value::string("primary")),
                ("size", Value::string("sm")),
                (
                    "icon",
                    Value::string(if running { "rotate-ccw" } else { "play" }),
                ),
                (
                    "label",
                    Value::string(if running {
                        crate::i18n::t("common.loading")
                    } else {
                        crate::i18n::t("codeRunner.run")
                    }),
                ),
                ("isLoading", Value::Bool(running)),
                (
                    "className",
                    Value::string(if running { "[&_svg]:animate-spin" } else { "" }),
                ),
                ("action", Value::string(run_event.as_str())),
                (
                    "actionPayload",
                    object_value([
                        ("code", Value::string(code.as_str())),
                        ("language", Value::string(language.as_str())),
                        ("runId", Value::string(run_id.as_str())),
                    ]),
                ),
            ],
        );

        let output_pane = has_output.then(|| {
            let exit_code = output.and_then(|output| get_number(output, "exitCode"));
            let stdout = output.map(|o| get_string_or(o, "stdout", "")).unwrap_or_default();
            let stderr = output.map(|o| get_string_or(o, "stderr", "")).unwrap_or_default();
            let tests = output.map(|o| get_array(o, "testResults")).unwrap_or_default();
            let body = if let Some(error) = error {
                typography(registry, view, "small", &plain_text_of(Some(error)), None, None, Some("text-error whitespace-pre-wrap"))
            } else {
                rsx! {
                    if !stdout.is_empty() {
                        {typography(registry, view, "small", &stdout, None, None, Some("text-background whitespace-pre-wrap"))}
                    }
                    if !stderr.is_empty() {
                        {typography(registry, view, "small", &stderr, None, None, Some("text-error whitespace-pre-wrap"))}
                    }
                    if stdout.is_empty() && stderr.is_empty() {
                        {typography(registry, view, "small", &crate::i18n::t("codeRunner.noOutput"), None, None, Some("text-background italic"))}
                    }
                    if !tests.is_empty() {
                        div { class: "mt-3 pt-3 border-t border-border space-y-2",
                            for (index, test) in tests.iter().enumerate() {
                                {test_row(index, test, registry, view)}
                            }
                        }
                    }
                }
            };
            rsx! {
                div { class: "rounded-container border border-border bg-foreground overflow-hidden",
                    div { class: "flex flex-row gap-2 items-center px-3 py-2 bg-card border-b border-border",
                        {render_pattern(registry, view, "icon", vec![
                            ("name", Value::string("terminal")),
                            ("size", Value::string("sm")),
                            ("className", Value::string("text-muted-foreground")),
                        ])}
                        {typography(registry, view, "small", &crate::i18n::t("codeRunner.output"), None, None, Some("text-foreground font-medium"))}
                        if let Some(code) = exit_code {
                            {render_pattern(registry, view, "badge", vec![
                                ("variant", Value::string(if code == 0.0 { "success" } else { "danger" })),
                                ("size", Value::string("sm")),
                                ("label", Value::string(format!("Exit {}", crate::renderer::plain_text(&Value::Number(code))))),
                            ])}
                        }
                    }
                    div { class: "flex flex-col gap-0 items-stretch justify-start p-3 font-mono text-sm", {body} }
                }
            }
        });

        rsx! {
            div { class: "{root_class}", ..shared_root_attrs(props),
                div { class: "group relative", style: "height:360px",
                    {block(vec![
                        ("editable", Value::Bool(true)),
                        ("showLanguageBadge", Value::Bool(true)),
                        ("showCopyButton", Value::Bool(false)),
                        ("maxHeight", Value::string("100%")),
                    ])}
                    div { class: "flex flex-row gap-1 items-center absolute top-2 right-2 z-10 opacity-0 pointer-events-none group-hover:opacity-100 group-hover:pointer-events-auto focus-within:opacity-100 focus-within:pointer-events-auto transition-opacity bg-[var(--color-card)]/90 backdrop-blur-sm rounded-container p-1 shadow-elevation-popover border border-border",
                        {copy_button}
                        {reset_button}
                        {run_button}
                    }
                }
                if let Some(pane) = output_pane { {pane} }
            }
        }
    }
}
