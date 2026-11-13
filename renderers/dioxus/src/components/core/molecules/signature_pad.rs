use crate::registry::DioxusPatternRegistry;
use crate::renderer::{
    array_value, cn, emit_event, object_value, pattern_spec, target_point, DioxusPatternRenderer,
    DioxusView, EventSink, OwnedDioxusView,
};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

/// Dioxus renderer for the `signature-pad` pattern.
pub struct SignaturePadRenderer;

#[derive(Props, Clone, PartialEq)]
struct SignaturePadProps {
    props: Value,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
}

const CLEAR: &str = "__signature_pad_clear";
const SIGN: &str = "__signature_pad_sign";

/// The strokes as the PNG data URL the web pad's `canvas.toDataURL` answers: drawn at their own pixel positions
/// into a `width`×`height` bitmap with round caps, transparent elsewhere.
pub(crate) fn signature_png(strokes: &[Vec<(f64, f64)>], width: u32, height: u32, color: [u8; 4], stroke_width: f64) -> Option<String> {
    let (w, h) = (width as usize, height as usize);
    let mut pixels = vec![0u8; w * h * 4];
    let radius = (stroke_width / 2.0).max(0.5);
    let mut dot = |cx: f64, cy: f64| {
        let (x0, x1) = ((cx - radius).floor().max(0.0) as usize, ((cx + radius).ceil() as usize).min(w));
        let (y0, y1) = ((cy - radius).floor().max(0.0) as usize, ((cy + radius).ceil() as usize).min(h));
        for y in y0..y1 {
            for x in x0..x1 {
                let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                if dx * dx + dy * dy <= radius * radius {
                    pixels[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&color);
                }
            }
        }
    };
    for stroke in strokes {
        for pair in stroke.windows(2) {
            let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
            let steps = ((bx - ax).hypot(by - ay) * 2.0).ceil().max(1.0) as usize;
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                dot(ax + (bx - ax) * t, ay + (by - ay) * t);
            }
        }
        if let [(x, y)] = stroke.as_slice() {
            dot(*x, *y);
        }
    }
    encode_png(&pixels, width, height)
}

#[cfg(feature = "native")]
fn encode_png(pixels: &[u8], width: u32, height: u32) -> Option<String> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().ok()?.write_image_data(pixels).ok()?;
    }
    Some(format!("data:image/png;base64,{}", orbital_core::codec::base64_encode(&out)))
}

/// Without the Blitz shell there is no PNG encoder in this build, so no image is answered.
#[cfg(not(feature = "native"))]
fn encode_png(_pixels: &[u8], _width: u32, _height: u32) -> Option<String> {
    None
}

/// `#rgb` or `#rrggbb`; anything else (a CSS variable, a name) draws black, as a 2D canvas ignores a style it cannot parse.
fn stroke_rgba(color: Option<&str>) -> [u8; 4] {
    let hex = color.and_then(|c| c.strip_prefix('#')).unwrap_or("");
    let digit = |i: usize, n: usize| u8::from_str_radix(&hex[i..i + n], 16).ok();
    let rgb = match hex.len() {
        3 => (0..3).map(|i| digit(i, 1).map(|d| d * 17)).collect::<Option<Vec<u8>>>(),
        6 => (0..3).map(|i| digit(i * 2, 2)).collect::<Option<Vec<u8>>>(),
        _ => None,
    };
    match rgb.as_deref() {
        Some([r, g, b]) => [*r, *g, *b, 255],
        _ => [0, 0, 0, 255],
    }
}

fn stroke_path(stroke: &[(f64, f64)]) -> String {
    stroke
        .iter()
        .enumerate()
        .map(|(i, (x, y))| format!("{}{x:.1} {y:.1}", if i == 0 { "M" } else { " L" }))
        .collect()
}

#[allow(non_snake_case)]
fn SignaturePad(input: SignaturePadProps) -> Element {
    let props = &input.props;
    let view = input.view.as_view();
    let registry = &input.registry;
    let class_name = get_string(props, "className").unwrap_or_default();
    let initial = get_string(props, "value").is_some_and(|value| !value.is_empty());
    let mut has_signature = use_signal(move || initial);
    let mut strokes = use_signal(Vec::<Vec<(f64, f64)>>::new);
    let mut drawing = use_signal(|| false);
    let clear_event = get_string(props, "clearEvent").filter(|name| !name.is_empty());
    let sign_event = get_string(props, "signEvent").filter(|name| !name.is_empty());
    let on_change = get_string(props, "onChange").filter(|name| !name.is_empty());
    let height = get_number_or(props, "height", 200.0) as i64;
    let color = stroke_rgba(get_string(props, "strokeColor").as_deref());
    let stroke_width = get_number_or(props, "strokeWidth", 2.0);
    let stroke_style = get_string(props, "strokeColor").unwrap_or_else(|| "currentColor".to_string());
    let png = move |drawn: &[Vec<(f64, f64)>]| signature_png(drawn, 600, height.max(1) as u32, color, stroke_width);
    let sink = view.sink;
    let on_change_clear = on_change.clone();
    let sign_event_relay = sign_event.clone();
    let relay: EventSink = Callback::new(move |(name, _): (String, Value)| {
        if name == CLEAR {
            has_signature.set(false);
            strokes.set(Vec::new());
            emit_event(sink, &on_change_clear, object_value([("dataUrl", Value::Null)]));
            if let (Some(sink), Some(event)) = (sink, &clear_event) {
                sink.call((event.clone(), object_value::<&str>([])));
            }
        }
        if name == SIGN {
            let signature = png(&strokes.read()).map(Value::string).unwrap_or(Value::Null);
            emit_event(sink, &sign_event_relay, object_value([("signature", signature)]));
        }
    });

    if get_bool_or(props, "isLoading", false) {
        let spec = pattern_spec(
            "loading-state",
            [
                ("message", Value::string(crate::i18n::t("common.loading"))),
                ("className", Value::string(class_name)),
            ],
        );
        return registry.render_spec(&spec, None, &view);
    }
    if let Some(error) = props.get("error").filter(|error| !error.is_null()) {
        let message = error
            .as_str()
            .map(str::to_string)
            .or_else(|| get_string(error, "message"))
            .unwrap_or_default();
        let spec = pattern_spec(
            "error-state",
            [
                (
                    "title",
                    Value::string(crate::i18n::t("display.signaturePadError")),
                ),
                ("message", Value::string(message)),
                ("className", Value::string(class_name)),
            ],
        );
        return registry.render_spec(&spec, None, &view);
    }

    let label = get_string(props, "label").unwrap_or_else(|| crate::i18n::t("signaturePad.label"));
    let helper = get_string(props, "helperText")
        .unwrap_or_else(|| crate::i18n::t("signaturePad.helperText"));
    let read_only = get_bool_or(props, "readOnly", false);
    let card_class = cn(&[
        "rounded-container relative transition-all duration-normal chrome-panel surface-content p-card-md p-4",
        &class_name,
    ]);
    let pad_class = cn(&[
        "bg-background rounded-container shadow-none w-full border border-border",
        if read_only {
            "opacity-60 cursor-not-allowed"
        } else {
            "cursor-crosshair"
        },
    ]);
    let aria_label = if label.is_empty() {
        crate::i18n::t("signaturePad.label")
    } else {
        label.clone()
    };
    let label_spec = pattern_spec(
        "typography",
        [
            ("variant", Value::string("label")),
            ("weight", Value::string("medium")),
            ("content", Value::string(label.clone())),
        ],
    );
    let helper_spec = pattern_spec(
        "typography",
        [
            ("variant", Value::string("caption")),
            ("color", Value::string("secondary")),
            ("content", Value::string(helper.clone())),
        ],
    );
    let mut buttons = vec![pattern_spec(
        "button",
        [
            ("variant", Value::string("ghost")),
            ("size", Value::string("sm")),
            ("leftIcon", Value::string("eraser")),
            ("label", Value::string(crate::i18n::t("signaturePad.clear"))),
            ("action", Value::string(CLEAR)),
            ("disabled", Value::Bool(!has_signature())),
        ],
    )];
    if sign_event.is_some() {
        buttons.push(pattern_spec(
            "button",
            [
                ("variant", Value::string("primary")),
                ("size", Value::string("sm")),
                ("leftIcon", Value::string("check")),
                (
                    "label",
                    Value::string(crate::i18n::t("signaturePad.confirm")),
                ),
                ("action", Value::string(SIGN)),
                ("disabled", Value::Bool(!has_signature())),
            ],
        ));
    }
    let actions = pattern_spec(
        "hstack",
        [
            ("gap", Value::string("sm")),
            ("justify", Value::string("end")),
            ("children", array_value(buttons)),
        ],
    );
    let relay_view = view.for_source(view.source_trait.clone(), relay);
    let finish = Callback::new(move |_: ()| {
        if !drawing() {
            return;
        }
        drawing.set(false);
        has_signature.set(true);
        let data_url = png(&strokes.read()).map(Value::string).unwrap_or(Value::Null);
        emit_event(sink, &on_change, object_value([("dataUrl", data_url)]));
    });
    rsx! {
        div { class: "{card_class}", style: "height:auto",
            div { class: "flex flex-col gap-2 items-stretch justify-start", style: "height:auto",
                if !label.is_empty() { {registry.render_spec(&label_spec, None, &view)} }
                div { class: "{pad_class} relative", style: "height:{height}px", role: "img", aria_label: "{aria_label}",
                    svg { class: "absolute inset-0 w-full h-full pointer-events-none", fill: "none", stroke: "{stroke_style}", stroke_width: "{stroke_width}", stroke_linecap: "round", stroke_linejoin: "round",
                        for stroke in strokes.read().iter() { path { d: "{stroke_path(stroke)}" } }
                    }
                    if !read_only {
                        div {
                            class: "absolute inset-0",
                            "data-signature-surface": "true",
                            onmousedown: move |event| {
                                if let Some(at) = target_point(&event) {
                                    drawing.set(true);
                                    strokes.write().push(vec![(at.x, at.y)]);
                                }
                            },
                            onmousemove: move |event| {
                                if !drawing() {
                                    return;
                                }
                                if let (Some(at), Some(stroke)) = (target_point(&event), strokes.write().last_mut()) {
                                    stroke.push((at.x, at.y));
                                }
                            },
                            onmouseup: move |_| finish.call(()),
                            onmouseleave: move |_| finish.call(()),
                        }
                    }
                }
                if !helper.is_empty() { {registry.render_spec(&helper_spec, None, &view)} }
                if !read_only { {registry.render_spec(&actions, None, &relay_view)} }
            }
        }
    }
}

impl DioxusPatternRenderer for SignaturePadRenderer {
    fn pattern_name(&self) -> &str {
        "signature-pad"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { SignaturePad { props: props.clone(), view: view.to_owned(), registry: registry.clone() } }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stroke_color_is_a_hex_or_the_canvas_default() {
        assert_eq!(stroke_rgba(Some("#ff0000")), [255, 0, 0, 255]);
        assert_eq!(stroke_rgba(Some("#0f0")), [0, 255, 0, 255]);
        assert_eq!(stroke_rgba(Some("var(--color-foreground)")), [0, 0, 0, 255], "control: a style a canvas cannot parse draws black");
        assert_eq!(stroke_rgba(None), [0, 0, 0, 255]);
    }

    #[test]
    fn a_stroke_path_starts_with_a_move() {
        assert_eq!(stroke_path(&[(1.0, 2.0), (3.5, 4.0)]), "M1.0 2.0 L3.5 4.0");
    }

    #[cfg(feature = "native")]
    #[test]
    fn the_png_paints_the_stroke_and_leaves_the_rest_clear() {
        let url = signature_png(&[vec![(10.0, 10.0), (30.0, 10.0)]], 40, 20, [255, 0, 0, 255], 2.0).expect("png");
        let bytes = orbital_core::codec::base64_decode(url.strip_prefix("data:image/png;base64,").expect("data url")).unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut pixels).unwrap();
        let at = |x: usize, y: usize| &pixels[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4];
        assert_eq!(at(20, 10), [255, 0, 0, 255], "on the stroke");
        assert_eq!(at(20, 2), [0, 0, 0, 0], "control: off the stroke stays transparent");
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
            trait_name: "Pad",
            orbital_name: "Pad",
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
        crate::registry::default_registry().render_spec(&props, None, &view)
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
            DocumentConfig { viewport: Some(Viewport::new(600, 500, 1.0, ColorScheme::Light)), ..Default::default() },
        );
        for css in crate::styles::stylesheets() {
            doc.add_user_agent_stylesheet(&css);
        }
        doc.initial_build();
        settle(&mut doc);
        doc
    }

    fn events() -> Vec<(String, Value)> {
        EVENTS.with(|events| events.borrow().clone())
    }

    fn center(doc: &DioxusDocument, selector: &str) -> (f32, f32) {
        let id = doc.query_selector(selector).unwrap().unwrap_or_else(|| panic!("{selector}"));
        let at = doc.get_node(id).unwrap().synthetic_click_event_data(Modifiers::default());
        (at.x, at.y)
    }

    #[test]
    fn drawing_a_stroke_reports_its_png_and_confirm_signs_with_it() {
        let _guard = crate::EVENT_CONVERTER_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let mut doc = document(serde_json::json!({ "type": "signature-pad", "onChange": "CHANGED", "signEvent": "SIGNED" }));
        let (x, y) = center(&doc, "[data-signature-surface]");
        doc.handle_ui_event(UiEvent::MouseMove(mouse(x, y, MouseEventButtons::None)));
        doc.handle_ui_event(UiEvent::MouseDown(mouse(x, y, MouseEventButtons::Primary)));
        doc.handle_ui_event(UiEvent::MouseMove(mouse(x + 40.0, y + 10.0, MouseEventButtons::Primary)));
        doc.handle_ui_event(UiEvent::MouseUp(mouse(x + 40.0, y + 10.0, MouseEventButtons::None)));
        settle(&mut doc);
        let changed: Vec<_> = events().into_iter().filter(|(name, _)| name == "CHANGED").collect();
        assert_eq!(changed.len(), 1, "{:?}", events());
        let url = changed[0].1.get("dataUrl").and_then(Value::as_str).unwrap_or_default().to_string();
        assert!(url.starts_with("data:image/png;base64,"), "{url}");
        assert!(doc.query_selector("path").unwrap().is_some(), "the stroke is drawn");
        let (cx, cy) = center(&doc, "[data-testid=\"action-__signature_pad_sign\"]");
        doc.handle_ui_event(UiEvent::MouseMove(mouse(cx, cy, MouseEventButtons::None)));
        doc.handle_ui_event(UiEvent::MouseDown(mouse(cx, cy, MouseEventButtons::Primary)));
        doc.handle_ui_event(UiEvent::MouseUp(mouse(cx, cy, MouseEventButtons::None)));
        settle(&mut doc);
        let signed: Vec<_> = events().into_iter().filter(|(name, _)| name == "SIGNED").collect();
        assert_eq!(signed.len(), 1, "{:?}", events());
        assert_eq!(signed[0].1.get("signature").and_then(Value::as_str), Some(url.as_str()));
    }

    #[test]
    fn control_a_read_only_pad_offers_no_drawing_surface() {
        let _guard = crate::EVENT_CONVERTER_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let doc = document(serde_json::json!({ "type": "signature-pad", "readOnly": true, "onChange": "CHANGED" }));
        assert!(doc.query_selector("[data-signature-surface]").unwrap().is_none());
        assert!(events().is_empty());
    }
}
