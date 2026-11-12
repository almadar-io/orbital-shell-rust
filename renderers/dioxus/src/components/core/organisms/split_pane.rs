//! Native Dioxus renderer for the `split-pane` pattern, mirroring the web `SplitPane` organism.

use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::merge_class_name;
use crate::renderer::{
    cn, emit_event, number_value, object_value, target_point, DioxusPatternRenderer, DioxusView,
    OwnedDioxusView, TargetPoint,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `split-pane` pattern.
pub struct SplitPaneRenderer;

impl SplitPaneRenderer {
    const NAME: &str = "split-pane";
}

/// The first pane's share (0-100) for a drag at `at` over the whole container, clamped so each pane keeps
/// `min_size` pixels, as the web's `handlePointerMove`; `None` before the container has a size.
pub(crate) fn drag_ratio(at: TargetPoint, horizontal: bool, min_size: f64) -> Option<f64> {
    let (position, extent) = if horizontal { (at.x, at.width) } else { (at.y, at.height) };
    if extent <= 0.0 {
        return None;
    }
    let min = (min_size / extent * 100.0).min(50.0);
    Some((position / extent * 100.0).clamp(min, 100.0 - min))
}

/// The two panes, the handle and the drag that moves it: one mechanism for the `split-pane` pattern and the dock's
/// splits, as the web's DockLayout composes SplitPane. `ratio` is the first pane's share; `on_ratio` hears each drag.
#[derive(Props, Clone, PartialEq)]
pub(crate) struct SplitFrameProps {
    horizontal: bool,
    ratio: f64,
    min_size: f64,
    resizable: bool,
    class: String,
    left_class: String,
    right_class: String,
    #[props(default)]
    attrs: Vec<Attribute>,
    left: Element,
    right: Element,
    on_ratio: Callback<f64>,
}

#[allow(non_snake_case)]
pub(crate) fn SplitFrame(input: SplitFrameProps) -> Element {
    let SplitFrameProps { horizontal, ratio, min_size, resizable, class, left_class, right_class, attrs, left, right, on_ratio } = input;
    let mut dragging = use_signal(|| false);
    let shown = Value::Number(ratio).to_string_value();
    let root_class = cn(&[
        "relative flex w-full h-full",
        if horizontal { "flex-row" } else { "flex-col" },
        &class,
    ]);
    let left_style = format!(
        "{}:{shown}%;flex-shrink:0",
        if horizontal { "width" } else { "height" }
    );
    let handle_class = cn(&[
        "flex-shrink-0 bg-border transition-colors touch-none",
        if horizontal {
            "w-1 cursor-col-resize hover:w-1.5 hover:bg-muted-foreground"
        } else {
            "h-1 cursor-row-resize hover:h-1.5 hover:bg-muted-foreground"
        },
    ]);
    let overlay_class = cn(&[
        "absolute inset-0 z-50",
        if horizontal { "cursor-col-resize" } else { "cursor-row-resize" },
    ]);
    rsx! { div { class: "{root_class}", ..attrs,
        div { class: "{left_class}", style: "{left_style}", {left} }
        if resizable {
            div {
                class: "{handle_class}",
                "data-split-handle": "true",
                onmousedown: move |_| dragging.set(true),
            }
        }
        div { class: "{right_class}", {right} }
        if dragging() {
            div {
                class: "{overlay_class}",
                "data-split-overlay": "true",
                onmousemove: move |event| {
                    if let Some(next) = target_point(&event).and_then(|at| drag_ratio(at, horizontal, min_size)) {
                        on_ratio.call(next);
                    }
                },
                onmouseup: move |_| dragging.set(false),
                onmouseleave: move |_| dragging.set(false),
            }
        }
    } }
}

#[derive(Props, Clone, PartialEq)]
struct SplitPaneProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

#[allow(non_snake_case)]
fn SplitPane(input: SplitPaneProps) -> Element {
    let view = input.view.as_view();
    let props = &input.props;
    let registry = &input.registry;
    let declared = get_number_or(props, "ratio", 50.0);
    let on_ratio_event = get_string(props, "onRatioChange").filter(|name| !name.is_empty());
    let mut own = use_signal(|| declared);
    // Controlled when the program hears the ratio: the split follows the prop, as the web's `onRatioChange`.
    let controlled = on_ratio_event.is_some();
    let ratio = if controlled { declared } else { own() };
    let sink = view.sink;
    let on_ratio = Callback::new(move |next: f64| {
        if controlled {
            emit_event(sink, &on_ratio_event, object_value([("ratio", number_value(next))]));
        } else {
            own.set(next);
        }
    });
    let fields = props.as_object();
    let left = fields.and_then(|fields| fields.get("left")).map(|left| view.render_node(registry, left));
    let right = fields.and_then(|fields| fields.get("right")).map(|right| view.render_node(registry, right));
    rsx! { SplitFrame {
        horizontal: get_string_or(props, "direction", "horizontal") != "vertical",
        ratio,
        min_size: get_number_or(props, "minSize", 100.0),
        resizable: get_bool_or(props, "resizable", true),
        class: merge_class_name("", props),
        left_class: cn(&["flex flex-col overflow-auto", &get_string_or(props, "leftClassName", "")]),
        right_class: cn(&["flex flex-col flex-1 min-h-0 min-w-0 overflow-auto", &get_string_or(props, "rightClassName", "")]),
        attrs: shared_root_attrs(props),
        left: left.unwrap_or(rsx! {}),
        right: right.unwrap_or(rsx! {}),
        on_ratio,
    } }
}

impl DioxusPatternRenderer for SplitPaneRenderer {
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
        rsx! { SplitPane { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}

#[cfg(test)]
mod ratio_tests {
    use super::*;

    fn at(x: f64, width: f64) -> TargetPoint {
        TargetPoint { x, y: 0.0, width, height: 300.0 }
    }

    #[test]
    fn a_drag_sets_the_share_of_the_container_and_keeps_each_pane_its_minimum() {
        assert_eq!(drag_ratio(at(300.0, 400.0), true, 100.0), Some(75.0));
        assert_eq!(drag_ratio(at(10.0, 400.0), true, 100.0), Some(25.0), "the first pane keeps 100px");
        assert_eq!(drag_ratio(at(395.0, 400.0), true, 100.0), Some(75.0), "the second pane keeps 100px");
        assert_eq!(drag_ratio(TargetPoint { x: 0.0, y: 150.0, width: 400.0, height: 300.0 }, false, 0.0), Some(50.0));
        assert_eq!(drag_ratio(at(10.0, 0.0), true, 100.0), None, "control: no size yet, no ratio");
        assert_eq!(drag_ratio(at(10.0, 150.0), true, 100.0), Some(50.0), "control: a container smaller than both minimums splits evenly");
    }
}

#[cfg(all(test, feature = "native", not(target_arch = "wasm32")))]
mod dom_tests {
    use super::*;
    use blitz_dom::{Document as _, DocumentConfig};
    use blitz_traits::events::{BlitzMouseButtonEvent, MouseEventButton, MouseEventButtons, UiEvent};
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus_native::DioxusDocument;
    use keyboard_types::Modifiers;
    use orbital_core::ValueMap;
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static EVENTS: RefCell<Vec<(String, Value)>> = const { RefCell::new(Vec::new()) };
        static PROPS: RefCell<Value> = const { RefCell::new(Value::Null) };
    }

    fn app() -> Element {
        let props = PROPS.with(|props| props.borrow().clone());
        let entities = HashMap::new();
        let trait_content = ValueMap::new();
        let sink = Callback::new(|event| EVENTS.with(|events| events.borrow_mut().push(event)));
        let view = DioxusView {
            trait_name: "Split",
            orbital_name: "Split",
            current_state: "idle",
            entities: &entities,
            trait_content: &trait_content,
            source_trait: None,
            sink: Some(sink),
            source_sink: Callback::new(move |_| sink),
            navigate: None,
            dock: None,
            viewport_width: None,
            canvas_theme: None,
            theme_mode: None,
            toggle_theme: None,
        };
        let mut registry = DioxusPatternRegistry::new();
        registry.register(Box::new(SplitPaneRenderer));
        registry.render_spec(&props, None, &view)
    }

    fn settle(doc: &mut DioxusDocument) {
        for _ in 0..4 {
            if !doc.poll(None) {
                break;
            }
        }
        doc.resolve(0.0);
    }

    fn mouse(x: f32, y: f32, buttons: MouseEventButtons) -> BlitzMouseButtonEvent {
        BlitzMouseButtonEvent { x, y, button: MouseEventButton::Main, buttons, mods: Modifiers::default() }
    }

    fn document(props: serde_json::Value) -> DioxusDocument {
        PROPS.with(|slot| *slot.borrow_mut() = serde_json::from_value(props).unwrap());
        EVENTS.with(|events| events.borrow_mut().clear());
        let mut doc = DioxusDocument::new(
            dioxus_core::VirtualDom::new(app),
            DocumentConfig {
                viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Light)),
                ..Default::default()
            },
        );
        for css in crate::styles::stylesheets() {
            doc.add_user_agent_stylesheet(&css);
        }
        doc.initial_build();
        settle(&mut doc);
        doc
    }

    /// Press the handle, then move to `x` over the container and release there.
    fn drag_handle_to(doc: &mut DioxusDocument, x: f32) {
        let handle = doc.query_selector("[data-split-handle]").unwrap().expect("a handle");
        let press = doc.get_node(handle).unwrap().synthetic_click_event_data(Modifiers::default());
        doc.handle_ui_event(UiEvent::MouseMove(mouse(press.x, press.y, MouseEventButtons::None)));
        doc.handle_ui_event(UiEvent::MouseDown(mouse(press.x, press.y, MouseEventButtons::Primary)));
        settle(doc);
        assert!(doc.query_selector("[data-split-overlay]").unwrap().is_some(), "the press lays the drag overlay");
        doc.handle_ui_event(UiEvent::MouseMove(mouse(x, 150.0, MouseEventButtons::Primary)));
        settle(doc);
        doc.handle_ui_event(UiEvent::MouseUp(mouse(x, 150.0, MouseEventButtons::None)));
        settle(doc);
    }

    fn ratios() -> Vec<f64> {
        EVENTS.with(|events| {
            events
                .borrow()
                .iter()
                .filter(|(name, _)| name == "RATIO")
                .filter_map(|(_, payload)| payload.get("ratio").and_then(Value::as_number))
                .collect()
        })
    }

    #[test]
    fn dragging_the_handle_reports_the_clamped_ratio_to_a_listening_program() {
        let _guard = crate::EVENT_CONVERTER_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let props = serde_json::json!({ "type": "split-pane", "ratio": 50, "minSize": 100, "onRatioChange": "RATIO", "className": "h-64", "left": "L", "right": "R" });
        let mut doc = document(props.clone());
        drag_handle_to(&mut doc, 300.0);
        assert_eq!(ratios(), vec![75.0]);
        let mut doc = document(props);
        drag_handle_to(&mut doc, 20.0);
        assert_eq!(ratios(), vec![25.0], "the first pane keeps its 100px minimum");
        assert!(doc.query_selector("[data-split-overlay]").unwrap().is_none(), "the release lifts the overlay");
    }

    #[test]
    fn control_a_pane_that_is_not_resizable_has_no_handle_and_reports_nothing() {
        let _guard = crate::EVENT_CONVERTER_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let doc = document(serde_json::json!({ "type": "split-pane", "resizable": false, "onRatioChange": "RATIO", "left": "L", "right": "R" }));
        assert!(doc.query_selector("[data-split-handle]").unwrap().is_none());
        assert!(ratios().is_empty());
    }
}
