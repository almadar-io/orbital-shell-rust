//! Native body of the `rich-text-editor` pattern, mirroring the web `RichTextEditor` molecule's
//! frame: styles, toolbar and editing surface DOM. The contentEditable engine (execCommand, selection
//! tracking, markdown shortcuts, HTML sanitising) has no native counterpart and is not carried.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{pattern_spec, DioxusPatternRenderer, DioxusView, BOX_ATOM_BASE};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `rich-text-editor` pattern.
pub struct RichTextEditorRenderer;

impl RichTextEditorRenderer {
    const NAME: &str = "rich-text-editor";
}

const RICH_TEXT_CSS: &str = r#"
.almadar-rich-text { line-height: 1.7; color: var(--color-foreground); }
.almadar-rich-text h1 { font-size: 1.875rem; font-weight: 700; line-height: 1.25; margin: 1.25em 0 0.4em; }
.almadar-rich-text h2 { font-size: 1.5rem; font-weight: 650; line-height: 1.3; margin: 1.1em 0 0.4em; }
.almadar-rich-text h3 { font-size: 1.25rem; font-weight: 600; line-height: 1.35; margin: 1em 0 0.35em; }
.almadar-rich-text h1:first-child, .almadar-rich-text h2:first-child, .almadar-rich-text h3:first-child { margin-top: 0; }
.almadar-rich-text p { margin: 0.5em 0; }
.almadar-rich-text ul { list-style: disc; padding-inline-start: 1.5rem; margin: 0.5em 0; }
.almadar-rich-text ol { list-style: decimal; padding-inline-start: 1.5rem; margin: 0.5em 0; }
.almadar-rich-text li { margin: 0.25em 0; }
.almadar-rich-text blockquote { border-inline-start: 3px solid var(--color-primary); padding-inline-start: 1rem; font-style: italic; color: var(--color-muted-foreground); margin: 0.75em 0; }
.almadar-rich-text pre { background: var(--color-muted); border: 1px solid var(--color-border); border-radius: 0.375rem; padding: 0.75rem 1rem; font-family: ui-monospace, monospace; font-size: 0.875em; white-space: pre-wrap; margin: 0.75em 0; }
.almadar-rich-text code { font-family: ui-monospace, monospace; font-size: 0.875em; background: var(--color-muted); border-radius: 0.25rem; padding: 0.1em 0.35em; }
.almadar-rich-text pre code { background: transparent; padding: 0; }
.almadar-rich-text a { color: var(--color-primary); text-decoration: underline; }
.almadar-rich-text hr { border: none; border-top: 1px solid var(--color-border); margin: 1.25em 0; }
.almadar-rich-text img { max-width: 100%; border-radius: 0.375rem; }
"#;

const TOOLBAR_BUTTONS: [(&str, &str, bool); 14] = [
    ("bold", "richTextEditor.bold", false),
    ("italic", "richTextEditor.italic", false),
    ("underline", "richTextEditor.underline", false),
    ("strikethrough", "richTextEditor.strikethrough", true),
    ("heading-1", "richTextEditor.heading1", false),
    ("heading-2", "richTextEditor.heading2", false),
    ("heading-3", "richTextEditor.heading3", true),
    ("list", "richTextEditor.bulletList", false),
    ("list-ordered", "richTextEditor.numberedList", true),
    ("quote", "richTextEditor.quote", false),
    ("code", "richTextEditor.code", false),
    ("link", "richTextEditor.link", false),
    ("image", "richTextEditor.image", false),
    ("minus", "richTextEditor.divider", false),
];

fn toolbar_button(icon: &str, label: &str) -> Value {
    pattern_spec(
        "button",
        [
            ("label", Value::string("")),
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("icon", Value::string(icon)),
            ("aria-label", Value::string(label)),
            ("aria-pressed", Value::string("false")),
            ("title", Value::string(label)),
            (
                "className",
                Value::string("h-8 w-8 p-0 gap-0 justify-center"),
            ),
        ],
    )
}

impl DioxusPatternRenderer for RichTextEditorRenderer {
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
        let class_name = get_string_or(props, "className", "");
        let value = get_string_or(props, "value", "");
        if get_bool_or(props, "readOnly", false) {
            let class = format!("{BOX_ATOM_BASE} almadar-rich-text max-w-none {class_name}");
            return rsx! {
                div { class: "{class}", ..shared_root_attrs(props),
                    style { "{RICH_TEXT_CSS}" }
                    div { class: "{BOX_ATOM_BASE}", dangerous_inner_html: "{value}" }
                }
            };
        }
        let show_toolbar = get_bool_or(props, "showToolbar", true);
        let placeholder = get_string_or(
            props,
            "placeholder",
            &crate::i18n::t("richTextEditor.placeholder"),
        );
        let empty = value.trim().is_empty();
        let seeded = if empty {
            "<p><br></p>".to_string()
        } else {
            value
        };
        let buttons: Vec<(Value, bool)> = TOOLBAR_BUTTONS
            .iter()
            .map(|(icon, label, divider_after)| {
                (toolbar_button(icon, &crate::i18n::t(label)), *divider_after)
            })
            .collect();
        let class = format!("{BOX_ATOM_BASE} flex flex-col gap-2 {class_name}");
        rsx! {
            div { class: "{class}", ..shared_root_attrs(props),
                style { "{RICH_TEXT_CSS}" }
                input { r#type: "file", accept: "image/*", class: "hidden", aria_hidden: "true", tabindex: "-1" }
                if show_toolbar {
                    div { class: "{BOX_ATOM_BASE} sticky top-0 z-10 flex flex-wrap items-center gap-0.5 self-start rounded-container border border-border bg-background/95 px-1 py-0.5 shadow-elevation-popover",
                        role: "toolbar", aria_label: crate::i18n::t("richTextEditor.editorToolbar"),
                        for (spec, divider) in buttons.iter() {
                            {registry.render_spec(spec, None, view)}
                            if *divider { div { class: "{BOX_ATOM_BASE} mx-1 h-5 w-px bg-border" } }
                        }
                    }
                }
                div { class: "{BOX_ATOM_BASE} almadar-rich-text max-w-none min-h-[8rem] outline-none focus-visible:ring-[length:var(--focus-ring-width)] focus-visible:ring-ring focus-visible:ring-inset data-[empty=true]:before:content-[attr(data-placeholder)] data-[empty=true]:before:text-muted-foreground/60 data-[empty=true]:before:pointer-events-none data-[empty=true]:before:absolute relative cursor-text",
                    contenteditable: "true", role: "textbox", aria_multiline: "true", aria_label: crate::i18n::t("richTextEditor.editorSurface"),
                    "data-placeholder": "{placeholder}", "data-empty": "{empty}",
                    dangerous_inner_html: "{seeded}",
                }
            }
        }
    }
}
