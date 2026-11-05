//! Native Dioxus renderer for the shared pattern contract.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    object_value, pattern_spec, DioxusPatternRenderer, DioxusView, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `document-viewer` pattern.
pub struct DocumentViewerRenderer;

impl DocumentViewerRenderer {
    const NAME: &str = "document-viewer";
}

const ZOOM_IN: &str = "__document_viewer_zoom_in";
const ZOOM_OUT: &str = "__document_viewer_zoom_out";
const PAGE_PREV: &str = "__document_viewer_page_prev";
const PAGE_NEXT: &str = "__document_viewer_page_next";
const DOWNLOAD: &str = "__document_viewer_download";
const PRINT: &str = "__document_viewer_print";
const TAB: &str = "__document_viewer_tab";
const ACTION_PREFIX: &str = "__document_viewer_action:";

#[derive(Props, Clone, PartialEq)]
struct ViewerProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

fn arr(items: Vec<Value>) -> Value {
    Value::Array(items.into())
}

fn text(content: &str, variant: &str, color: &str, class: &str) -> Value {
    pattern_spec(
        "typography",
        [
            ("content", Value::string(content)),
            ("variant", Value::string(variant)),
            ("color", Value::string(color)),
            ("className", Value::string(class)),
        ],
    )
}

fn icon_button(icon: &str, action: &str, label: &str, disabled: bool) -> Value {
    pattern_spec(
        "button",
        [
            ("label", Value::string("")),
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("icon", Value::string(icon)),
            ("action", Value::string(action)),
            ("aria-label", Value::string(label)),
            ("disabled", Value::Bool(disabled)),
        ],
    )
}

fn css_size(props: &Value) -> String {
    match props.get("height") {
        Some(Value::String(height)) => height.to_string(),
        Some(Value::Int(px)) => format!("{px}px"),
        Some(Value::Number(px)) => format!("{px}px"),
        _ => "600px".to_string(),
    }
}

#[allow(non_snake_case)]
fn Viewer(input: ViewerProps) -> Element {
    let props = &input.props;
    let registry = &input.registry;
    let mut zoom = use_signal(|| 100_i64);
    let mut active_doc = use_signal(|| 0_usize);
    let initial_page = get_number(props, "currentPage")
        .map(|page| page as i64)
        .unwrap_or(1);
    let mut page = use_signal(|| initial_page);

    let class_name = get_string_or(props, "className", "");
    let documents = get_array(props, "documents");
    let actions = get_array(props, "actions");
    let total_pages = get_number(props, "totalPages").map(|total| total as i64);
    let src = get_string(props, "src").filter(|value| !value.is_empty());
    let content = get_string(props, "content").filter(|value| !value.is_empty());
    let doc_type = get_string_or(props, "documentType", "pdf");
    let title = get_string(props, "title").filter(|value| !value.is_empty());
    let show_toolbar = get_bool_or(props, "showToolbar", true);
    let show_download = get_bool_or(props, "showDownload", false);
    let show_print = get_bool_or(props, "showPrint", false);
    let height = css_size(props);

    let active = documents.get(active_doc());
    let active_src = active
        .and_then(|doc| get_string(doc, "src"))
        .filter(|value| !value.is_empty())
        .or(src.clone());
    let active_content = active
        .and_then(|doc| get_string(doc, "content"))
        .filter(|value| !value.is_empty())
        .or(content.clone());
    let active_type = active
        .and_then(|doc| get_string(doc, "documentType"))
        .unwrap_or(doc_type);

    let inner_sink = input.view.sink;
    let action_events: Vec<String> = actions
        .iter()
        .map(|action| get_string_or(action, "event", ""))
        .collect();
    let download_src = active_src.clone();
    let adapter = Callback::new(
        move |(event, payload): (String, Value)| match event.as_str() {
            ZOOM_IN => zoom.set((zoom() + 25).min(200)),
            ZOOM_OUT => zoom.set((zoom() - 25).max(50)),
            PAGE_PREV => {
                let old = page();
                page.set((old - 1).max(1));
                if let Some(sink) = inner_sink {
                    sink.call((
                        "DOCUMENT_PAGE_CHANGE".to_string(),
                        object_value([("page", Value::Int(old - 1))]),
                    ));
                }
            }
            PAGE_NEXT => {
                if let Some(total) = total_pages {
                    let old = page();
                    page.set((old + 1).min(total));
                    if let Some(sink) = inner_sink {
                        sink.call((
                            "DOCUMENT_PAGE_CHANGE".to_string(),
                            object_value([("page", Value::Int(old + 1))]),
                        ));
                    }
                }
            }
            DOWNLOAD => {
                if let (Some(sink), Some(src)) = (inner_sink, &download_src) {
                    sink.call((
                        "DOCUMENT_DOWNLOAD".to_string(),
                        object_value([("src", Value::string(src.as_str()))]),
                    ));
                }
            }
            PRINT => {
                if let Some(sink) = inner_sink {
                    sink.call(("DOCUMENT_PRINT".to_string(), object_value::<&str>([])));
                }
            }
            TAB => {
                if let Some(id) = payload.get("tabId").and_then(Value::as_str) {
                    if let Ok(index) = id.trim_start_matches("doc-").parse::<usize>() {
                        active_doc.set(index);
                    }
                }
            }
            other => {
                if let Some(index) = other
                    .strip_prefix(ACTION_PREFIX)
                    .and_then(|i| i.parse::<usize>().ok())
                {
                    if let (Some(sink), Some(event)) = (
                        inner_sink,
                        action_events.get(index).filter(|e| !e.is_empty()),
                    ) {
                        sink.call((event.clone(), object_value([("page", Value::Int(page()))])));
                    }
                } else if let Some(sink) = inner_sink {
                    sink.call((event, payload));
                }
            }
        },
    );
    let mut owned = input.view.clone();
    owned.sink = Some(adapter);
    let view = owned.as_view();

    let class_field = || ("className", Value::string(class_name.as_str()));

    if get_bool_or(props, "isLoading", false) {
        let spec = pattern_spec(
            "loading-state",
            [("message", Value::string("Loading...")), class_field()],
        );
        return registry.render_spec(&spec, None, &view);
    }
    if let Some(error) = props
        .get("error")
        .filter(|error| !matches!(error, Value::Null))
    {
        let spec = pattern_spec(
            "error-state",
            [
                ("title", Value::string("Document error")),
                (
                    "message",
                    Value::string(get_string_or(error, "message", "")),
                ),
                class_field(),
            ],
        );
        return registry.render_spec(&spec, None, &view);
    }
    if active_src.is_none() && active_content.is_none() {
        let spec = pattern_spec(
            "empty-state",
            [
                ("icon", Value::string("file-text")),
                ("title", Value::string("No document")),
                ("description", Value::string("No document to display.")),
                class_field(),
            ],
        );
        return registry.render_spec(&spec, None, &view);
    }

    let tabs = if documents.len() > 1 {
        let items: Vec<Value> = documents
            .iter()
            .enumerate()
            .map(|(index, doc)| {
                object_value([
                    ("id", Value::string(format!("doc-{index}"))),
                    ("label", Value::string(get_string_or(doc, "label", ""))),
                ])
            })
            .collect();
        Some(pattern_spec(
            "box",
            [
                ("className", Value::string("border-b border-border")),
                (
                    "children",
                    arr(vec![pattern_spec(
                        "tabs",
                        [
                            ("items", arr(items)),
                            ("activeTab", Value::string(format!("doc-{}", active_doc()))),
                            ("tabChangeEvent", Value::string(TAB)),
                        ],
                    )]),
                ),
            ],
        ))
    } else {
        None
    };

    let toolbar = show_toolbar.then(|| {
        let mut left = vec![pattern_spec(
            "icon",
            [
                ("name", Value::string("file-text")),
                ("size", Value::string("sm")),
                ("className", Value::string("text-muted-foreground")),
            ],
        )];
        if let Some(title) = &title {
            left.push(text(
                title,
                "small",
                "primary",
                "truncate max-w-[200px] font-medium",
            ));
        }
        let mut right = vec![
            icon_button("zoom-out", ZOOM_OUT, "Zoom out", false),
            text(
                &format!("{}%", zoom()),
                "caption",
                "secondary",
                "tabular-nums w-10 text-center",
            ),
            icon_button("zoom-in", ZOOM_IN, "Zoom in", false),
        ];
        if let Some(total) = total_pages.filter(|total| *total > 1) {
            right.push(pattern_spec(
                "box",
                [("className", Value::string("w-px h-4 bg-border mx-1"))],
            ));
            right.push(icon_button(
                "chevron-left",
                PAGE_PREV,
                "Previous page",
                page() <= 1,
            ));
            right.push(text(
                &format!("{} / {}", page(), total),
                "caption",
                "secondary",
                "tabular-nums",
            ));
            right.push(icon_button(
                "chevron-right",
                PAGE_NEXT,
                "Next page",
                page() >= total,
            ));
        }
        right.push(pattern_spec(
            "box",
            [("className", Value::string("w-px h-4 bg-border mx-1"))],
        ));
        if show_download {
            right.push(icon_button(
                "download",
                DOWNLOAD,
                "Download document",
                false,
            ));
        }
        if show_print {
            right.push(icon_button("printer", PRINT, "Print document", false));
        }
        for (index, action) in actions.iter().enumerate() {
            right.push(pattern_spec(
                "box",
                [
                    ("display", Value::string("inline-flex")),
                    ("action", Value::string(format!("{ACTION_PREFIX}{index}"))),
                    (
                        "children",
                        arr(vec![pattern_spec(
                            "badge",
                            [
                                ("variant", Value::string("default")),
                                ("label", Value::string(get_string_or(action, "label", ""))),
                                (
                                    "className",
                                    Value::string(
                                        "cursor-pointer hover:opacity-80 transition-opacity",
                                    ),
                                ),
                            ],
                        )]),
                    ),
                ],
            ));
        }
        pattern_spec(
            "hstack",
            [
                ("gap", Value::string("sm")),
                ("align", Value::string("center")),
                ("justify", Value::string("between")),
                (
                    "className",
                    Value::string("px-4 py-2 border-b border-border bg-muted/30"),
                ),
                (
                    "children",
                    arr(vec![
                        pattern_spec(
                            "hstack",
                            [
                                ("gap", Value::string("sm")),
                                ("align", Value::string("center")),
                                ("children", arr(left)),
                            ],
                        ),
                        pattern_spec(
                            "hstack",
                            [
                                ("gap", Value::string("xs")),
                                ("align", Value::string("center")),
                                ("children", arr(right)),
                            ],
                        ),
                    ]),
                ),
            ],
        )
    });

    let zoom_value = zoom();
    let body = if active_type == "pdf" && active_src.is_some() {
        let source = active_src.clone().unwrap_or_default();
        let frame_title = title.clone().unwrap_or_else(|| "Document".to_string());
        let style = format!(
            "height:{height};transform:scale({});transform-origin:top left;width:{}%",
            zoom_value as f64 / 100.0,
            10000.0 / zoom_value as f64
        );
        rsx! { iframe { class: "w-full border-0", src: "{source}", title: "{frame_title}", style: "{style}" } }
    } else if active_type == "html" && active_content.is_some() {
        let html = active_content.clone().unwrap_or_default();
        rsx! { div { class: "w-full overflow-auto p-4", style: "height:{height};font-size:{zoom_value}%", dangerous_inner_html: "{html}" } }
    } else {
        let spec = pattern_spec(
            "typography",
            [
                (
                    "content",
                    Value::string(active_content.clone().unwrap_or_default()),
                ),
                ("variant", Value::string("body")),
                ("color", Value::string("primary")),
                (
                    "className",
                    Value::string("whitespace-pre-wrap break-words"),
                ),
            ],
        );
        rsx! { div { class: "w-full overflow-auto p-4 font-mono text-sm", style: "height:{height};font-size:{zoom_value}%", {registry.render_spec(&spec, None, &view)} } }
    };

    let surface = if get_string(props, "surface").as_deref() == Some("none") {
        ""
    } else {
        "surface-content"
    };
    let root_class = format!("{surface} p-card-md rounded-container overflow-hidden {class_name}");
    rsx! {
        div { class: "{root_class}", ..crate::registry::shared_root_attrs(props),
            div { class: "flex flex-col gap-0 items-stretch justify-start",
                if let Some(tabs) = tabs { {registry.render_spec(&tabs, None, &view)} }
                if let Some(toolbar) = toolbar { {registry.render_spec(&toolbar, None, &view)} }
                div { class: "w-full overflow-hidden bg-muted/20", {body} }
            }
        }
    }
}

impl DioxusPatternRenderer for DocumentViewerRenderer {
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
        rsx! { Viewer { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
