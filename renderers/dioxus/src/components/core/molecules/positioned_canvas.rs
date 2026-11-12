//! Native Dioxus renderer for the `positioned-canvas` pattern: the web's positioned item plane with select and drag.

use crate::registry::{shared_root_attrs_with_defaults, DioxusPatternRegistry};
use crate::renderer::{
    cn, merge_class_name, object_value, pattern_spec, DioxusPatternRenderer, DioxusView, EventSink,
    OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `positioned-canvas` pattern.
pub struct PositionedCanvasRenderer;

impl PositionedCanvasRenderer {
    const NAME: &str = "positioned-canvas";
}

#[derive(Props, Clone, PartialEq)]
struct CanvasProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[derive(Clone, PartialEq)]
struct Drag {
    id: String,
    offset_x: f64,
    offset_y: f64,
    moved: bool,
}

const BOX: &str = "bg-transparent rounded-none shadow-none";

fn status_classes(status: &str) -> Option<&'static str> {
    match status {
        "empty" => Some("bg-surface border-border text-foreground"),
        "seated" => Some("bg-surface border-success text-foreground"),
        "ordered" => Some("bg-surface border-warning text-foreground"),
        "awaiting-bill" => Some("bg-surface border-info text-foreground"),
        "cleaning" => Some("bg-muted border-border text-muted-foreground"),
        _ => None,
    }
}

fn status_badge(status: &str) -> Option<(&'static str, &'static str)> {
    match status {
        "empty" => Some(("default", "Empty")),
        "seated" => Some(("success", "Seated")),
        "ordered" => Some(("warning", "Ordered")),
        "awaiting-bill" => Some(("info", "Awaiting bill")),
        "cleaning" => Some(("neutral", "Cleaning")),
        _ => None,
    }
}

fn shape_classes(shape: &str) -> Option<(&'static str, f64, f64)> {
    match shape {
        "round" => Some(("rounded-full w-24 h-24", 96.0, 96.0)),
        "square" => Some(("rounded-container w-24 h-24", 96.0, 96.0)),
        "rectangle" => Some(("rounded-container w-36 h-20", 144.0, 80.0)),
        _ => None,
    }
}

fn status_icon(status: &str) -> Option<Value> {
    let name = match status {
        "seated" => "users",
        "ordered" => "coffee",
        "awaiting-bill" => "alert-circle",
        _ => return None,
    };
    Some(pattern_spec(
        "icon",
        [
            ("name", Value::string(name)),
            ("className", Value::string("w-4 h-4")),
        ],
    ))
}

fn typography(
    variant: &str,
    color: Option<&str>,
    weight: Option<&str>,
    class: &str,
    content: &str,
) -> Value {
    let mut fields = vec![
        ("variant", Value::string(variant)),
        ("content", Value::string(content)),
    ];
    if let Some(color) = color {
        fields.push(("color", Value::string(color)));
    }
    if let Some(weight) = weight {
        fields.push(("weight", Value::string(weight)));
    }
    if !class.is_empty() {
        fields.push(("className", Value::string(class)));
    }
    pattern_spec("typography", fields)
}

use crate::renderer::emit_event as emit;

fn id_payload(id: Option<&str>) -> Value {
    object_value([("id", id.map(Value::string).unwrap_or(Value::Null))])
}

#[allow(non_snake_case)]
fn Canvas(input: CanvasProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let width = get_number_or(props, "width", 800.0);
    let height = get_number_or(props, "height", 600.0);
    let selected_id = get_string(props, "selectedId");
    let editable = get_bool_or(props, "editable", false);
    let select_event = get_string(props, "selectEvent");
    let move_event = get_string(props, "moveEvent");
    let selectable = select_event.is_some();
    let sink = view.sink;
    let mut drag = use_signal(|| None::<Drag>);
    let mut dragging = use_signal(|| None::<String>);

    let root_class = merge_class_name(
        &format!("{BOX} max-w-full overflow-auto rounded-container"),
        props,
    );
    let attrs = shared_root_attrs_with_defaults(
        props,
        &[
            ("role", Value::string("group")),
            ("aria-label", Value::string("Floor plan")),
        ],
    );
    let plane_class = cn(&[
        BOX,
        "relative bg-background border border-border rounded-container overflow-hidden touch-none",
    ]);
    let plane_style = format!("width:{width}px;height:{height}px");
    let clear_event = select_event.clone();

    let items = get_array(props, "items");
    let mut nodes = Vec::new();
    for item in items.iter() {
        let Some(item_id) = get_string(item, "id") else {
            tracing::error!("positioned-canvas item has no id");
            continue;
        };
        let label = get_string_or(item, "label", "");
        let x = get_number(item, "x");
        let y = get_number(item, "y");
        let capacity = get_number(item, "capacity");
        let party_size = get_number(item, "partySize");
        let server_name = get_string(item, "serverName");
        let status = get_string_or(item, "status", "empty");
        let shape = get_string_or(item, "shape", "round");
        let is_selected = selected_id.as_deref() == Some(item_id.as_str());
        let is_dragging = dragging().as_deref() == Some(item_id.as_str());
        let shape_info = shape_classes(&shape);
        let badge = status_badge(&status);
        if badge.is_none() || shape_info.is_none() {
            tracing::error!(id = %item_id, status = %status, shape = %shape, "positioned-canvas item has an undeclared status or shape");
        }
        let class = cn(&[
            "absolute flex flex-col items-center justify-center gap-1 border-heavy select-none",
            "transition-shadow",
            status_classes(&status).unwrap_or(""),
            shape_info.map(|info| info.0).unwrap_or(""),
            if editable {
                "cursor-move"
            } else {
                "cursor-pointer"
            },
            if is_selected {
                "outline outline-2 outline-offset-2 outline-primary shadow-elevation-card"
            } else {
                ""
            },
            if is_dragging {
                "shadow-elevation-popover z-10"
            } else {
                ""
            },
        ]);
        let style = format!(
            "{}{}touch-action:none",
            x.map(|x| format!("left:{x}px;")).unwrap_or_default(),
            y.map(|y| format!("top:{y}px;")).unwrap_or_default()
        );
        let label_row = {
            let mut children = Vec::new();
            if let Some(icon) = status_icon(&status) {
                children.push(icon);
            }
            children.push(typography("body2", None, Some("semibold"), "", &label));
            children
        };
        let caption = match (party_size, capacity) {
            (Some(party), Some(capacity)) if status == "seated" => format!("{party}/{capacity}"),
            (_, Some(capacity)) => format!("Cap {capacity}"),
            (_, None) => String::new(),
        };
        let caption_spec = typography("caption", Some("secondary"), None, "", &caption);
        let status_label = badge.map(|badge| badge.1).unwrap_or("");
        let sr_spec = typography("caption", None, None, "sr-only", status_label);
        let server_spec = server_name
            .filter(|_| status == "seated")
            .filter(|name| !name.is_empty())
            .map(|name| {
                typography(
                    "caption",
                    Some("secondary"),
                    None,
                    "truncate max-w-[80%]",
                    &name,
                )
            });
        let badge_spec = (is_selected && badge.is_some()).then(|| {
            let (variant, label) = badge.unwrap_or(("default", ""));
            pattern_spec(
                "badge",
                [
                    ("variant", Value::string(variant)),
                    ("size", Value::string("sm")),
                    ("className", Value::string("absolute -top-2 -right-2")),
                    ("label", Value::string(label)),
                ],
            )
        });
        let item_width = shape_info.map(|info| info.1).unwrap_or(0.0);
        let item_height = shape_info.map(|info| info.2).unwrap_or(0.0);
        let item_x = x.unwrap_or(0.0);
        let item_y = y.unwrap_or(0.0);
        let toggle_event = select_event.clone();
        let toggle_selected = selected_id.clone();
        let toggle_id = item_id.clone();
        let toggle = move || {
            let next = if toggle_selected.as_deref() == Some(toggle_id.as_str()) {
                None
            } else {
                Some(toggle_id.as_str())
            };
            emit(sink, &toggle_event, id_payload(next));
        };
        let key_toggle = toggle.clone();
        let up_toggle = toggle.clone();
        let down_id = item_id.clone();
        let move_id = item_id.clone();
        let move_name = move_event.clone();
        let test_id = format!("item-node-{item_id}");
        let pressed = is_selected.to_string();
        let tab_index = if selectable { Some(0) } else { None };
        let role = if selectable { Some("button") } else { None };
        let aria_pressed = if selectable {
            Some(pressed.clone())
        } else {
            None
        };
        let render = |spec: &Value| registry.render_spec(spec, None, &view);
        let label_row_nodes = label_row.iter().map(&render).collect::<Vec<_>>();
        let caption_node = render(&caption_spec);
        let sr_node = render(&sr_spec);
        let server_node = server_spec.as_ref().map(&render);
        let badge_node = badge_spec.as_ref().map(&render);
        nodes.push(rsx! {
            div {
                key: "{item_id}",
                "data-testid": "{test_id}",
                "data-status": "{status}",
                class: "{class}",
                style: "{style}",
                role: role,
                tabindex: tab_index,
                "aria-pressed": aria_pressed,
                onclick: move |event| event.stop_propagation(),
                onkeydown: move |event| {
                    if matches!(event.key(), Key::Enter) || matches!(event.key(), Key::Character(ref c) if c == " ") {
                        event.prevent_default();
                        key_toggle();
                    }
                },
                onpointerdown: move |event| {
                    if !editable {
                        return;
                    }
                    let at = event.element_coordinates();
                    drag.set(Some(Drag { id: down_id.clone(), offset_x: at.x, offset_y: at.y, moved: false }));
                    dragging.set(Some(down_id.clone()));
                },
                onpointermove: move |event| {
                    let Some(mut current) = drag.peek().clone() else { return; };
                    if current.id != move_id {
                        return;
                    }
                    let at = event.element_coordinates();
                    let new_x = item_x + at.x - current.offset_x;
                    let new_y = item_y + at.y - current.offset_y;
                    let clamped_x = new_x.min(width - item_width).max(0.0);
                    let clamped_y = new_y.min(height - item_height).max(0.0);
                    current.moved = true;
                    drag.set(Some(current));
                    emit(
                        sink,
                        &move_name,
                        object_value([
                            ("id", Value::string(move_id.as_str())),
                            ("x", Value::Number(clamped_x)),
                            ("y", Value::Number(clamped_y)),
                        ]),
                    );
                },
                onpointerup: {
                    let up = up_toggle.clone();
                    move |_| {
                        let Some(current) = drag.peek().clone() else { return; };
                        drag.set(None);
                        dragging.set(None);
                        if !current.moved {
                            up();
                        }
                    }
                },
                onpointercancel: move |_| {
                    let Some(current) = drag.peek().clone() else { return; };
                    drag.set(None);
                    dragging.set(None);
                    if !current.moved {
                        up_toggle();
                    }
                },
                div { class: "{BOX} flex items-center gap-1",
                    for node in label_row_nodes { {node} }
                }
                {caption_node}
                {sr_node}
                if let Some(node) = server_node { {node} }
                if let Some(node) = badge_node { {node} }
            }
        });
    }

    rsx! {
        div { class: "{root_class}", ..attrs,
            div {
                class: "{plane_class}",
                style: "{plane_style}",
                "data-testid": "positioned-canvas",
                onclick: move |_| emit(sink, &clear_event, id_payload(None)),
                for node in nodes { {node} }
            }
        }
    }
}

impl DioxusPatternRenderer for PositionedCanvasRenderer {
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
        rsx! { Canvas { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}
