//! Native Dioxus body for the `media-gallery` pattern, mirroring `MediaGallery.tsx`.

use crate::renderer::cn;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    number_value, object_value, pattern_spec, DioxusPatternRenderer, DioxusView, EventSink,
    OwnedDioxusView, BOX_ATOM_BASE,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::collections::BTreeSet;

pub struct MediaGalleryRenderer;

#[derive(Props, Clone, PartialEq)]
struct GalleryProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[derive(Clone, PartialEq)]
struct Media {
    id: String,
    src: String,
    alt: Option<String>,
    thumbnail: Option<String>,
    caption: Option<String>,
    row: Value,
}

fn image_url(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.to_string()),
        other => get_string(other, "url"),
    }
}

fn display(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| format_value(value))
}

fn text_or_none(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

fn items_of(props: &Value) -> Vec<Media> {
    let prop_items = get_array(props, "items");
    if !prop_items.is_empty() {
        return prop_items
            .iter()
            .map(|item| Media {
                id: item.get("id").map(display).unwrap_or_default(),
                src: image_url(item.get("src")).unwrap_or_default(),
                alt: text_or_none(get_string(item, "alt")),
                thumbnail: text_or_none(image_url(item.get("thumbnail"))),
                caption: text_or_none(get_string(item, "caption")),
                row: item.clone(),
            })
            .collect();
    }
    let src_field = get_string_or(props, "srcField", "src");
    let caption_field = get_string_or(props, "captionField", "caption");
    get_array(props, "entity")
        .iter()
        .enumerate()
        .map(|(index, record)| {
            let alt = text_or_none(record.get("alt").filter(|alt| !alt.is_null()).map(display));
            let thumbnail = image_url(record.get("thumbnail"));
            let caption = record
                .get(&caption_field)
                .filter(|caption| !caption.is_null())
                .map(display);
            Media {
                id: record
                    .get("id")
                    .filter(|id| !id.is_null())
                    .map(display)
                    .unwrap_or_else(|| index.to_string()),
                src: image_url(record.get(&src_field)).unwrap_or_default(),
                alt,
                thumbnail,
                caption,
                row: record.clone(),
            }
        })
        .collect()
}

#[allow(non_snake_case)]
fn Gallery(input: GalleryProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let mut lightbox = use_signal(|| None::<usize>);
    let mut failed = use_signal(BTreeSet::<String>::new);
    let class = get_string(props, "className");
    let wrapper = |spec: Value| {
        rsx! { div { class: "{BOX_ATOM_BASE}", ..crate::registry::shared_root_attrs(props), {registry.render_spec(&spec, None, &view)} } }
    };
    let with_class = |mut fields: Vec<(&'static str, Value)>| {
        if let Some(class) = &class {
            fields.push(("className", Value::string(class.clone())));
        }
        fields
    };

    if get_bool_or(props, "isLoading", false) {
        return wrapper(pattern_spec(
            "loading-state",
            with_class(vec![("message", Value::string("Loading..."))]),
        ));
    }
    if let Some(error) = props.get("error").filter(|error| !error.is_null()) {
        return wrapper(pattern_spec(
            "error-state",
            with_class(vec![
                ("title", Value::string("Gallery error")),
                (
                    "message",
                    Value::string(get_string_or(error, "message", "")),
                ),
            ]),
        ));
    }
    let items = items_of(props);
    let show_upload = get_bool_or(props, "showUpload", false);
    if items.is_empty() && !show_upload {
        return wrapper(pattern_spec(
            "empty-state",
            with_class(vec![
                ("icon", Value::string("image")),
                ("title", Value::string("No media")),
                ("description", Value::string("No media items to display.")),
            ]),
        ));
    }

    let title = get_string(props, "title").filter(|title| !title.is_empty());
    let actions = get_array(props, "actions");
    let selectable = get_bool_or(props, "selectable", false);
    let selected: Vec<String> = get_array(props, "selectedItems")
        .iter()
        .map(display)
        .collect();
    let selection_event = get_string_or(props, "selectionEvent", "SELECTION_CHANGE");
    let columns = get_number_or(props, "columns", 3.0) as i64;
    let columns_class = match columns {
        2 => "grid-cols-2",
        4 => "grid-cols-2 sm:grid-cols-3 lg:grid-cols-4",
        5 => "grid-cols-2 sm:grid-cols-3 lg:grid-cols-5",
        6 => "grid-cols-3 sm:grid-cols-4 lg:grid-cols-6",
        _ => "grid-cols-2 sm:grid-cols-3",
    };
    let aspect = match get_string_or(props, "aspectRatio", "square").as_str() {
        "landscape" => "aspect-video",
        "portrait" => "aspect-[3/4]",
        _ => "aspect-square",
    };
    let surface_class = if get_string_or(props, "surface", "auto") == "auto" { "surface-content" } else { "" };
    let root_class = cn(&[
        BOX_ATOM_BASE,
        surface_class,
        "p-6 rounded-container",
        get_string_or(props, "className", "").trim(),
    ]);
    let grid_class = cn(&[BOX_ATOM_BASE, "grid gap-3", columns_class]);
    let sink = view.sink;

    let header = (title.is_some() || show_upload || !actions.is_empty()).then(|| {
        let mut right: Vec<Value> = Vec::new();
        if show_upload {
            right.push(pattern_spec(
                "button",
                [
                    ("variant", Value::string("secondary")),
                    ("size", Value::string("sm")),
                    ("icon", Value::string("upload")),
                    ("label", Value::string("Upload")),
                    ("action", Value::string("MEDIA_UPLOAD")),
                ],
            ));
        }
        for action in actions.iter() {
            let mut fields = vec![
                (
                    "className",
                    Value::string("cursor-pointer hover:opacity-80 transition-opacity"),
                ),
                (
                    "children",
                    Value::Array(
                        vec![pattern_spec(
                            "badge",
                            [
                                ("variant", Value::string("default")),
                                ("label", Value::string(get_string_or(action, "label", ""))),
                            ],
                        )]
                        .into(),
                    ),
                ),
            ];
            if let Some(event) = get_string(action, "event") {
                fields.push(("action", Value::string(event)));
            }
            right.push(pattern_spec("box", fields));
        }
        let mut row: Vec<Value> = Vec::new();
        if let Some(title) = &title {
            row.push(pattern_spec(
                "typography",
                [
                    ("variant", Value::string("h5")),
                    ("content", Value::string(title.clone())),
                ],
            ));
        }
        row.push(pattern_spec(
            "hstack",
            [
                ("gap", Value::string("sm")),
                ("children", Value::Array(right.into())),
            ],
        ));
        pattern_spec(
            "hstack",
            [
                ("justify", Value::string("between")),
                ("align", Value::string("center")),
                ("children", Value::Array(row.into())),
            ],
        )
    });
    let count_spec = (selectable && !selected.is_empty()).then(|| {
        pattern_spec(
            "hstack",
            [
                ("gap", Value::string("sm")),
                ("align", Value::string("center")),
                (
                    "children",
                    Value::Array(
                        vec![pattern_spec(
                            "badge",
                            [
                                ("variant", Value::string("info")),
                                (
                                    "label",
                                    Value::string(format!("{} selected", selected.len())),
                                ),
                            ],
                        )]
                        .into(),
                    ),
                ),
            ],
        )
    });

    let close: EventSink = Callback::new(move |_: (String, Value)| lightbox.set(None));
    let lightbox_view = view.for_source(view.source_trait.clone(), close);
    let lightbox_spec = pattern_spec(
        "lightbox",
        [
            (
                "images",
                Value::Array(
                    items
                        .iter()
                        .map(|item| {
                            let alt = item.alt.clone().or_else(|| item.caption.clone());
                            let mut fields = vec![("src", Value::string(item.src.clone()))];
                            if let Some(alt) = alt {
                                fields.push(("alt", Value::string(alt)));
                            }
                            if let Some(caption) = &item.caption {
                                fields.push(("caption", Value::string(caption.clone())));
                            }
                            object_value(fields)
                        })
                        .collect::<Vec<_>>()
                        .into(),
                ),
            ),
            ("currentIndex", number_value(lightbox().unwrap_or(0) as f64)),
            ("isOpen", Value::Bool(lightbox().is_some())),
            ("closeAction", Value::string("MEDIA_LIGHTBOX_CLOSE")),
        ],
    );
    let attrs = crate::registry::shared_root_attrs(props);

    rsx! {
        div { class: "{root_class}", ..attrs,
            div { class: "flex flex-col gap-4 items-stretch justify-start", style: "height:auto",
                if let Some(spec) = header.as_ref() { {registry.render_spec(spec, None, &view)} }
                if let Some(spec) = count_spec.as_ref() { {registry.render_spec(spec, None, &view)} }
                div { class: "{grid_class}",
                    for (index, item) in items.iter().enumerate() {
                        {
                            let is_selected = selected.contains(&item.id);
                            let tone = if is_selected { "border-primary ring-2 ring-primary/30" } else { "border-transparent hover:border-border" };
                            let label = item.alt.clone().or_else(|| item.caption.clone()).unwrap_or_else(|| item.id.clone());
                            let alt = item.alt.clone().or_else(|| item.caption.clone()).unwrap_or_default();
                            let src = item.thumbnail.clone().filter(|thumb| !thumb.is_empty()).unwrap_or_else(|| item.src.clone());
                            let broken = failed().contains(&item.id);
                            let image_icon = pattern_spec("icon", [("name", Value::string("image")), ("size", Value::string("lg"))]);
                            let zoom_icon = pattern_spec("icon", [
                                ("name", Value::string("zoom-in")),
                                ("className", Value::string("text-white opacity-0 group-hover:opacity-100 transition-opacity duration-fast")),
                            ]);
                            let caption_spec = item.caption.as_ref().map(|caption| pattern_spec("typography", [
                                ("variant", Value::string("caption")),
                                ("className", Value::string("text-white truncate")),
                                ("content", Value::string(caption.clone())),
                            ]));
                            let check_spec = pattern_spec("typography", [
                                ("variant", Value::string("caption")),
                                ("className", Value::string("text-primary-foreground text-xs")),
                                ("content", Value::string("\u{2713}")),
                            ]);
                            let item_class = cn(&[
                                BOX_ATOM_BASE,
                                "group relative overflow-hidden rounded-container cursor-pointer",
                                "border-heavy transition-all duration-fast",
                                tone,
                                aspect,
                            ]);
                            let mut pick = {
                                let id = item.id.clone();
                                let row = item.row.clone();
                                let selected = selected.clone();
                                let event = selection_event.clone();
                                move || {
                                    if selectable {
                                        let next: Vec<Value> = if selected.contains(&id) {
                                            selected.iter().filter(|chosen| **chosen != id).cloned().map(Value::string).collect()
                                        } else {
                                            selected.iter().cloned().chain(std::iter::once(id.clone())).map(Value::string).collect()
                                        };
                                        if let Some(sink) = sink {
                                            sink.call((event.clone(), object_value([("selection", Value::Array(next.into()))])));
                                        }
                                    } else {
                                        lightbox.set(Some(index));
                                    }
                                    if let Some(sink) = sink {
                                        sink.call(("MEDIA_SELECT".to_string(), object_value([("row", row.clone())])));
                                    }
                                }
                            };
                            let mut pick_key = pick.clone();
                            let failed_id = item.id.clone();
                            rsx! {
                                div {
                                    key: "{item.id}",
                                    class: "{item_class}",
                                    role: "button",
                                    tabindex: "0",
                                    "aria-label": "{label}",
                                    "aria-pressed": if selectable { Some(is_selected.to_string()) } else { None },
                                    onclick: move |_| pick(),
                                    onkeydown: move |event| {
                                        if event.key() == Key::Enter || event.key() == Key::Character(" ".to_string()) {
                                            event.prevent_default();
                                            pick_key();
                                        }
                                    },
                                    if broken {
                                        div { class: "{BOX_ATOM_BASE} w-full h-full flex items-center justify-center bg-muted text-muted-foreground",
                                            {registry.render_spec(&image_icon, None, &view)}
                                        }
                                    } else {
                                        img {
                                            src: "{src}",
                                            alt: "{alt}",
                                            class: "w-full h-full object-cover",
                                            loading: "lazy",
                                            onerror: move |_| { failed.write().insert(failed_id.clone()); },
                                        }
                                    }
                                    div { class: "{BOX_ATOM_BASE} absolute inset-0 bg-foreground/0 group-hover:bg-foreground/20 transition-colors duration-fast flex items-center justify-center",
                                        {registry.render_spec(&zoom_icon, None, &view)}
                                    }
                                    if let Some(spec) = caption_spec.as_ref() {
                                        div { class: "{BOX_ATOM_BASE} absolute bottom-0 left-0 right-0 p-2 bg-gradient-to-t from-black/60 to-transparent",
                                            {registry.render_spec(spec, None, &view)}
                                        }
                                    }
                                    if selectable && is_selected {
                                        div { class: "{BOX_ATOM_BASE} absolute top-2 right-2 w-5 h-5 rounded-full bg-primary flex items-center justify-center",
                                            {registry.render_spec(&check_spec, None, &view)}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        {registry.render_spec(&lightbox_spec, None, &lightbox_view)}
    }
}

impl MediaGalleryRenderer {
    const NAME: &str = "media-gallery";
}

impl DioxusPatternRenderer for MediaGalleryRenderer {
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
        rsx! { Gallery { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
