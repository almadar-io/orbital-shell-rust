use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{cn, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `upload-drop-zone` pattern.
pub struct UploadDropZoneRenderer;

#[derive(Props, Clone, PartialEq)]
struct UploadDropZoneProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn format_file_size(bytes: f64) -> String {
    if bytes < 1024.0 {
        format!("{}B", bytes as i64)
    } else if bytes < 1024.0 * 1024.0 {
        format!("{}KB", (bytes / 1024.0).round() as i64)
    } else {
        format!("{}MB", (bytes / (1024.0 * 1024.0)).round() as i64)
    }
}

#[allow(non_snake_case)]
fn UploadDropZone(input: UploadDropZoneProps) -> Element {
    let props = &input.props;
    let view = input.view.as_view();
    let mut drag_over = use_signal(|| false);
    let accept = get_string(props, "accept").filter(|accept| !accept.is_empty());
    let max_size = get_number(props, "maxSize").filter(|size| *size != 0.0);
    let max_files = get_number_or(props, "maxFiles", 1.0);
    let label = get_string(props, "label")
        .unwrap_or_else(|| "Drop files here or click to browse".to_string());
    let disabled = get_bool_or(props, "disabled", false);
    let description = get_string(props, "description").unwrap_or_else(|| {
        let mut parts = Vec::new();
        if let Some(accept) = &accept {
            parts.push(format!("Accepted: {accept}"));
        }
        if let Some(size) = max_size {
            parts.push(format!("Max size: {}", format_file_size(size)));
        }
        if max_files > 1.0 {
            parts.push(format!("Up to {} files", max_files as i64));
        }
        parts.join(". ")
    });
    let icon = get_string(props, "icon").unwrap_or_else(|| "upload".to_string());
    let icon_spec = pattern_spec(
        "icon",
        [
            ("name", Value::string(icon)),
            ("size", Value::string("lg")),
            ("className", Value::string("text-muted-foreground mb-2")),
        ],
    );
    let title_spec = pattern_spec(
        "typography",
        [
            ("variant", Value::string("body1")),
            ("className", Value::string("text-center font-medium mb-1")),
            (
                "content",
                Value::string(if drag_over() {
                    "Drop files here".to_string()
                } else {
                    label.clone()
                }),
            ),
        ],
    );
    let description_spec = pattern_spec(
        "typography",
        [
            ("variant", Value::string("caption")),
            ("color", Value::string("muted")),
            ("className", Value::string("text-center")),
            ("content", Value::string(description)),
        ],
    );
    let class = cn(&[
        "relative flex flex-col items-center justify-center p-8 rounded-interactive border-heavy border-dashed transition-colors duration-fast",
        if disabled { "" } else { "cursor-pointer" },
        if drag_over() {
            "border-primary bg-primary bg-opacity-5"
        } else {
            "border-border bg-surface"
        },
        if disabled { "opacity-50 cursor-not-allowed" } else { "" },
        "hover:border-primary hover:bg-muted",
        get_string(props, "className").as_deref().unwrap_or(""),
    ]);
    let aria_label = get_string(props, "aria-label").unwrap_or(label);
    let multiple = max_files > 1.0;
    let accept_attr = accept.unwrap_or_default();
    rsx! {
        div { class: "{class}", role: "button", tabindex: if disabled { "-1" } else { "0" }, aria_label: "{aria_label}",
            ondragover: move |event| {
                event.prevent_default();
                event.stop_propagation();
                if !disabled { drag_over.set(true); }
            },
            ondragleave: move |event| {
                event.prevent_default();
                event.stop_propagation();
                drag_over.set(false);
            },
            ondrop: move |event| {
                event.prevent_default();
                event.stop_propagation();
                drag_over.set(false);
            },
            ..shared_root_attrs(props),
            input { r#type: "file", class: "hidden", accept: "{accept_attr}", multiple: multiple, disabled: disabled, aria_hidden: "true" }
            {input.registry.render_spec(&icon_spec, None, &view)}
            {input.registry.render_spec(&title_spec, None, &view)}
            {input.registry.render_spec(&description_spec, None, &view)}
        }
    }
}

impl DioxusPatternRenderer for UploadDropZoneRenderer {
    fn pattern_name(&self) -> &str {
        "upload-drop-zone"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { UploadDropZone { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
