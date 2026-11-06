//! The offscreen capture paints the mounted app to a PNG with no window.
#![cfg(all(feature = "native", not(target_arch = "wasm32")))]

use orbital_client_dioxus::offscreen::{capture_gpu_png, capture_png};
use orbital_client_dioxus::shell::mount_core;

fn program(main: serde_json::Value) -> String {
    serde_json::json!({
        "name": "t",
        "typeContractVersion": 1,
        "orbitals": [{
            "name": "Main",
            "entity": { "name": "Row", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
            "traits": [{ "name": "View", "linkedEntity": "Row", "stateMachine": {
                "states": [{ "name": "idle", "isInitial": true }],
                "transitions": [{ "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", main]] }]
            } }],
            "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "View" }] }]
        }]
    })
    .to_string()
}

fn dark_pixels(path: &std::path::Path) -> usize {
    let decoder = png::Decoder::new(std::fs::File::open(path).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf[..info.buffer_size()]
        .chunks(4)
        .filter(|px| px[0] < 128 && px[1] < 128 && px[2] < 128)
        .count()
}

fn red_pixels(path: &std::path::Path) -> usize {
    let decoder = png::Decoder::new(std::fs::File::open(path).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf[..info.buffer_size()]
        .chunks_exact(4)
        .filter(|px| px[0] > 180 && px[1] < 90 && px[2] < 90 && px[3] > 0)
        .count()
}

fn blue_pixels(path: &std::path::Path) -> usize {
    let decoder = png::Decoder::new(std::fs::File::open(path).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf[..info.buffer_size()]
        .chunks_exact(4)
        .filter(|px| px[2] > 150 && px[0] < 100 && px[1] < 100 && px[3] > 0)
        .count()
}

fn capture_rgba(path: &std::path::Path) -> Vec<u8> {
    let decoder = png::Decoder::new(std::fs::File::open(path).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    buf.truncate(info.buffer_size());
    buf
}

#[test]
fn controlled_fields_keep_late_echo_out_of_blitz_and_paint_external_clear() {
    use anyrender::ImageRenderer;
    use blitz_dom::{Document as _, DocumentConfig};
    use blitz_paint::paint_scene;
    use blitz_traits::events::{BlitzImeEvent, UiEvent};
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;
    use orbital_client_dioxus::{registry::default_registry, DioxusView};
    use std::cell::RefCell;

    thread_local! {
        static INITIAL: RefCell<orbital_core::Value> = const { RefCell::new(orbital_core::Value::Null) };
        static CURRENT: RefCell<Option<Signal<orbital_core::Value>>> = const { RefCell::new(None) };
    }

    fn app() -> Element {
        let props = use_signal(|| INITIAL.with(|initial| initial.borrow().clone()));
        CURRENT.with(|current| *current.borrow_mut() = Some(props));
        let registry = use_hook(default_registry);
        let entities = std::collections::HashMap::new();
        let trait_content = orbital_core::ValueMap::new();
        let sink = Callback::new(|_: (String, orbital_core::Value)| {});
        let view = DioxusView {
            trait_name: "Field",
            orbital_name: "Main",
            current_state: "ready",
            entities: &entities,
            trait_content: &trait_content,
            source_trait: None,
            sink: Some(sink),
            source_sink: Callback::new(move |_: Option<String>| sink),
            navigate: None,
            dock: None,
            viewport_width: None,
            canvas_theme: None,
            theme_mode: None,
            toggle_theme: None,
        };
        let current = props.read().clone();
        let rendered = registry.render_spec(&current, None, &view);
        rendered
    }

    fn authored_value(value: &str) {
        CURRENT.with(|current| {
            let mut props = current.borrow().expect("mounted field signal");
            let mut next = props.peek().clone();
            next.as_object_mut().expect("field props").insert(
                "value".into(),
                orbital_core::Value::String(value.to_owned().into()),
            );
            props.set(next);
        });
    }

    fn editor_text(doc: &DioxusDocument, selector: &str) -> String {
        let id = doc
            .query_selector(selector)
            .unwrap()
            .expect("mounted field");
        doc.get_node(id)
            .and_then(|node| node.element_data())
            .and_then(|element| element.text_input_data())
            .expect("Blitz text editor")
            .editor
            .text()
            .to_string()
    }

    fn gpu_frame(doc: &mut DioxusDocument) -> Vec<u8> {
        doc.resolve(0.0);
        let mut renderer = anyrender_vello::VelloImageRenderer::new(400, 160);
        let mut pixels = vec![0; 400 * 160 * 4];
        renderer.render(
            |painter| paint_scene(painter, doc, 1.0, 400, 160),
            &mut pixels,
        );
        pixels
    }

    fn settle(doc: &mut DioxusDocument) {
        for _ in 0..8 {
            if !doc.poll(None) {
                break;
            }
        }
    }

    for (pattern, selector) in [("input", "input#field"), ("textarea", "textarea#field")] {
        INITIAL.with(|initial| {
            *initial.borrow_mut() = serde_json::from_value(serde_json::json!({
                "type": pattern, "id": "field", "label": "Task", "value": "", "onChange": "EDIT"
            }))
            .unwrap();
        });
        let mut doc = DioxusDocument::new(
            dioxus_core::VirtualDom::new(app),
            DocumentConfig {
                viewport: Some(Viewport::new(400, 160, 1.0, ColorScheme::Light)),
                font_ctx: Some(orbital_client_dioxus::offscreen::embedded_font_context()),
                ..Default::default()
            },
        );
        for stylesheet in orbital_client_dioxus::styles::stylesheets() {
            doc.add_user_agent_stylesheet(&stylesheet);
        }
        doc.initial_build();
        settle(&mut doc);
        doc.resolve(0.0);
        let id = doc.query_selector(selector).unwrap().expect("field node");
        assert!(doc.set_focus_to(id));
        for draft in ["A", "AB"] {
            doc.handle_ui_event(UiEvent::Ime(BlitzImeEvent::TextState {
                text: draft.to_string(),
                selection: (draft.len(), draft.len()),
                compose_region: None,
            }));
            settle(&mut doc);
        }
        assert_eq!(editor_text(&doc, selector), "AB", "{pattern} local draft");
        let typed = gpu_frame(&mut doc);
        authored_value("A");
        settle(&mut doc);
        assert_eq!(editor_text(&doc, selector), "AB", "{pattern} delayed echo");
        authored_value("AB");
        settle(&mut doc);
        assert_eq!(editor_text(&doc, selector), "AB", "{pattern} latest echo");
        authored_value("");
        settle(&mut doc);
        assert_eq!(editor_text(&doc, selector), "", "{pattern} external clear");
        let cleared = gpu_frame(&mut doc);
        assert_ne!(
            typed, cleared,
            "{pattern} external clear did not change GPU frame"
        );
    }

    INITIAL.with(|initial| {
        *initial.borrow_mut() = serde_json::from_value(serde_json::json!({
            "type": "select", "id": "field", "label": "Task", "value": "A",
            "options": [{"value": "A", "label": "A"}, {"value": "B", "label": "B"}],
            "clearable": true, "onChange": "EDIT"
        }))
        .unwrap();
    });
    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(400, 160, 1.0, ColorScheme::Light)),
            font_ctx: Some(orbital_client_dioxus::offscreen::embedded_font_context()),
            ..Default::default()
        },
    );
    for stylesheet in orbital_client_dioxus::styles::stylesheets() {
        doc.add_user_agent_stylesheet(&stylesheet);
    }
    doc.initial_build();
    settle(&mut doc);
    doc.resolve(0.0);
    let trigger_text = |doc: &DioxusDocument| {
        let trigger = doc
            .query_selector("button[aria-haspopup='listbox']")
            .unwrap()
            .expect("Select trigger");
        doc.get_node(trigger).unwrap().text_content()
    };
    let initial_trigger = trigger_text(&doc);
    assert!(
        initial_trigger.contains('A'),
        "selected trigger={initial_trigger:?}, buttons={:?}",
        doc.query_selector_all("button")
            .unwrap()
            .into_iter()
            .map(|id| doc.get_node(id).unwrap().text_content())
            .collect::<Vec<_>>()
    );
    let clear = doc
        .query_selector_all("button")
        .unwrap()
        .into_iter()
        .find(|id| {
            doc.get_node(*id)
                .is_some_and(|node| node.text_content() == "Clear")
        })
        .expect("Select clear button");
    let click = doc
        .get_node(clear)
        .unwrap()
        .synthetic_click_event_data(dioxus_html::Modifiers::empty());
    doc.handle_ui_event(UiEvent::MouseMove(click.clone()));
    doc.handle_ui_event(UiEvent::MouseDown(click.clone()));
    doc.handle_ui_event(UiEvent::MouseUp(click));
    settle(&mut doc);
    let after_clear = trigger_text(&doc);
    assert!(
        after_clear.contains("Select..."),
        "Select after clear={after_clear:?}, buttons={:?}",
        doc.query_selector_all("button")
            .unwrap()
            .into_iter()
            .map(|id| doc.get_node(id).unwrap().text_content())
            .collect::<Vec<_>>()
    );
    authored_value("");
    settle(&mut doc);
    assert!(
        trigger_text(&doc).contains("Select..."),
        "Select clear echo"
    );
    authored_value("B");
    settle(&mut doc);
    assert!(trigger_text(&doc).contains('B'), "Select external value");
    let selected = gpu_frame(&mut doc);
    authored_value("");
    settle(&mut doc);
    assert!(
        trigger_text(&doc).contains("Select..."),
        "Select external clear"
    );
    let cleared = gpu_frame(&mut doc);
    assert_ne!(
        selected, cleared,
        "Select external clear did not change GPU frame"
    );
}

#[test]
fn geometric_pattern_modes_paint_through_native_gpu_at_two_viewports() {
    let dir = std::env::temp_dir().join(format!("dioxus-geometric-{}", std::process::id()));
    let mut background_counts = Vec::new();
    let mut frame_counts = Vec::new();
    orbital_client_dioxus::app::CSS_TEXT
        .with(|css| *css.borrow_mut() = orbital_client_dioxus::styles::stylesheets());
    for (width, height) in [(160_u32, 120_u32), (320, 200)] {
        for mode in ["background", "left", "right", "dual", "around", "frame"] {
            let path = dir.join(format!("{mode}-{width}x{height}.png"));
            let children = if mode == "frame" {
                serde_json::json!([{
                    "type": "box",
                    "style": { "backgroundColor": "#0000ff", "height": "24px" },
                    "children": [{ "type": "typography", "content": "FRAME CHILD" }]
                }])
            } else {
                serde_json::json!([])
            };
            let geometric = serde_json::json!({
                "type": "geometric-pattern",
                "variant": "rosette-filled",
                "mode": mode,
                "opacity": 1,
                "color": "#ff0000",
                "scale": 0.5,
                "strokeWidth": 3,
                "children": children
            });
            mount_core(&program(serde_json::json!({
                "type": "box",
                "style": {
                    "position": "relative",
                    "width": "100%",
                    "height": format!("{}px", height - 32)
                },
                "children": [geometric]
            })))
            .unwrap();
            capture_gpu_png(&path, width, height).unwrap();
            let pixels = capture_rgba(&path);
            let count = |x0: u32, x1: u32, color: &str| -> usize {
                (0..height)
                    .flat_map(|y| (x0..x1).map(move |x| (y, x)))
                    .filter(|(y, x)| {
                        let i = ((*y * width + *x) * 4) as usize;
                        let p = &pixels[i..i + 4];
                        match color {
                            "red" => p[0] > 180 && p[1] < 110 && p[2] < 110,
                            "blue" => p[2] > 160 && p[0] < 110 && p[1] < 110,
                            _ => unreachable!(),
                        }
                    })
                    .count()
            };
            let left = count(0, width / 4, "red");
            let middle = count(width * 3 / 8, width * 5 / 8, "red");
            let right = count(width * 3 / 4, width, "red");
            match mode {
                "background" => {
                    assert!(
                        left > 20 && right > 20,
                        "background did not tile at {width}x{height}: {left}/{right}"
                    );
                    background_counts.push(left + middle + right);
                }
                "left" => assert!(
                    left > right * 3 + 20,
                    "left mask failed at {width}x{height}: {left}/{right}"
                ),
                "right" => assert!(
                    right > left * 3 + 20,
                    "right mask failed at {width}x{height}: {left}/{right}"
                ),
                "dual" => assert!(
                    left + right > middle * 2 + 20,
                    "dual mask failed at {width}x{height}: {left}/{middle}/{right}"
                ),
                "around" => assert!(
                    left + right > middle * 2 + 20,
                    "around mask failed at {width}x{height}: {left}/{middle}/{right}"
                ),
                "frame" => {
                    let blue = count(0, width, "blue");
                    assert!(blue > 20, "frame child missing at {width}x{height}");
                    let red_rows = (0..height)
                        .filter(|y| {
                            (0..width).any(|x| {
                                let i = ((*y * width + x) * 4) as usize;
                                pixels[i] > 180 && pixels[i + 1] < 110 && pixels[i + 2] < 110
                            })
                        })
                        .collect::<Vec<_>>();
                    let blue_rows = (0..height)
                        .filter(|y| {
                            (0..width).any(|x| {
                                let i = ((*y * width + x) * 4) as usize;
                                pixels[i + 2] > 160 && pixels[i] < 110 && pixels[i + 1] < 110
                            })
                        })
                        .collect::<Vec<_>>();
                    assert!(
                        red_rows.first() < blue_rows.first() && red_rows.last() > blue_rows.last(),
                        "frame strips did not surround child at {width}x{height}"
                    );
                    frame_counts.push(left + middle + right);
                }
                _ => unreachable!(),
            }
        }
    }
    assert!(
        background_counts[1] > background_counts[0] * 2,
        "background did not grow with viewport: {background_counts:?}"
    );
    assert!(
        frame_counts[1] > frame_counts[0],
        "frame strips did not grow with viewport width: {frame_counts:?}"
    );
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn hidden_flex_sibling_and_empty_dioxus_slots_do_not_widen_buttons() {
    use blitz_dom::DocumentConfig;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        let empty = false;
        rsx! {
            div { class: "flex flex-col",
                div { class: "flex",
                    button { id: "direct-use", class: "inline-flex items-center justify-center gap-2 h-button-sm px-3 text-sm", "Use" }
                }
                div { class: "flex gap-2", id: "plain-row",
                    button { id: "plain-use", class: "inline-flex items-center justify-center gap-2 h-button-sm px-3 text-sm",
                        span { class: "inline-flex flex-1 min-w-0 items-center gap-2", span { "Use" } }
                    }
                    button { id: "plain-details", class: "inline-flex items-center justify-center gap-2 h-button-sm px-3 text-sm", "Details" }
                }
                div { class: "flex gap-2", id: "conditional-row",
                    button { id: "conditional-use", class: "inline-flex items-center justify-center gap-2 h-button-sm px-3 text-sm",
                        if empty { span { "busy" } }
                        span { class: "inline-flex flex-1 min-w-0 items-center gap-2",
                            if empty { span { "left" } }
                            span { "Use" }
                            if empty { span { "right" } }
                        }
                    }
                    div { id: "hidden-switch", class: "inline-flex hidden", "hidden switch" }
                    button { id: "conditional-details", class: "inline-flex items-center justify-center gap-2 h-button-sm px-3 text-sm", "Details" }
                }
            }
        }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(300, 100, 1.0, ColorScheme::Light)),
            font_ctx: Some(orbital_client_dioxus::offscreen::embedded_font_context()),
            ..Default::default()
        },
    );
    doc.initial_build();
    doc.create_head_element(
        "style",
        &[],
        &Some(include_str!("../assets/css/web-client.css").into()),
    );
    doc.resolve(0.0);
    let layout = |selector| {
        let id = doc.query_selector(selector).unwrap().unwrap();
        doc.get_node(id).unwrap().final_layout
    };
    let plain_use = layout("#plain-use");
    let direct_use = layout("#direct-use");
    let conditional_use = layout("#conditional-use");
    let plain_details = layout("#plain-details");
    let conditional_details = layout("#conditional-details");
    let hidden_switch = layout("#hidden-switch");
    assert!(
        (direct_use.size.width - plain_use.size.width).abs() < 1.0,
        "nested content inflated button: direct={} nested={}",
        direct_use.size.width,
        plain_use.size.width
    );
    assert!(
        (plain_use.size.width - conditional_use.size.width).abs() < 1.0,
        "empty Dioxus slots inflated button: plain={} conditional={}",
        plain_use.size.width,
        conditional_use.size.width
    );
    assert!(
        hidden_switch.size.width < 1.0,
        "hidden switch retained width {}",
        hidden_switch.size.width
    );
    assert!(
        (plain_details.location.x - conditional_details.location.x).abs() < 1.0,
        "second button shifted: plain={} conditional={}",
        plain_details.location.x,
        conditional_details.location.x
    );
}

#[test]
fn failed_avatar_image_request_paints_initials_after_onerror() {
    use anyrender::ImageRenderer;
    use blitz_dom::{net::Resource, Document as _, DocumentConfig};
    use blitz_paint::paint_scene;
    use blitz_traits::{
        net::{BoxedHandler, NetProvider, Request},
        shell::{ColorScheme, Viewport},
    };
    use dioxus_core::{provide_context, ScopeId, VirtualDom};
    use dioxus_native::DioxusDocument;
    use orbital_client_dioxus::offscreen::HeadSink;
    use std::{
        cell::RefCell,
        collections::VecDeque,
        rc::Rc,
        sync::{Arc, Mutex},
    };

    struct FailingImageProvider(Arc<Mutex<Vec<(usize, Resource)>>>);

    impl NetProvider<Resource> for FailingImageProvider {
        fn fetch(&self, doc_id: usize, _request: Request, handler: BoxedHandler<Resource>) {
            if let Some(error) = handler.error_resource() {
                self.0.lock().unwrap().push((doc_id, error));
            }
        }
    }

    orbital_client_dioxus::app::CSS_TEXT
        .with(|css| *css.borrow_mut() = orbital_client_dioxus::styles::stylesheets());
    mount_core(&program(serde_json::json!({
        "type": "avatar",
        "src": "missing-avatar.png",
        "initials": "ZZ",
        "size": "xl"
    })))
    .unwrap();

    let pending = Arc::new(Mutex::new(Vec::new()));
    let ops = Rc::new(RefCell::new(VecDeque::new()));
    let vdom = VirtualDom::new(orbital_client_dioxus::app::App);
    vdom.in_scope(ScopeId::ROOT, || {
        provide_context(Rc::new(HeadSink { ops: ops.clone() }) as Rc<dyn dioxus_document::Document>);
    });
    let mut doc = DioxusDocument::new(
        vdom,
        DocumentConfig {
            viewport: Some(Viewport::new(120, 120, 1.0, ColorScheme::Light)),
            font_ctx: Some(orbital_client_dioxus::offscreen::embedded_font_context()),
            net_provider: Some(Arc::new(FailingImageProvider(pending.clone()))),
            ..Default::default()
        },
    );
    doc.initial_build();
    for _ in 0..8 {
        let mut changed = false;
        while let Some(op) = ops.borrow_mut().pop_front() {
            doc.create_head_element(&op.name, &op.attributes, &op.contents);
            changed = true;
        }
        changed |= doc.poll(None);
        if !changed {
            break;
        }
    }
    doc.resolve(0.0);
    assert!(doc.query_selector("img").unwrap().is_some());

    let mut renderer = anyrender_vello_cpu::VelloCpuImageRenderer::new(120, 120);
    let mut before = vec![0u8; 120 * 120 * 4];
    renderer.render(
        |painter| paint_scene(painter, &doc, 1.0, 120, 120),
        &mut before,
    );

    let failures = std::mem::take(&mut *pending.lock().unwrap());
    assert_eq!(failures.len(), 1, "Avatar must request its image once");
    for (doc_id, resource) in failures {
        assert_eq!(doc_id, doc.id());
        assert!(matches!(&resource, Resource::ImageError(_, src) if src == "missing-avatar.png"));
        doc.load_resource(resource);
    }
    assert!(
        doc.poll(None),
        "image error should schedule the Avatar fallback"
    );
    doc.resolve(0.0);
    assert!(doc.query_selector("img").unwrap().is_none());
    let root = doc.root_node().children[0];
    assert!(doc.get_node(root).unwrap().text_content().contains("ZZ"));

    let mut after = vec![0u8; 120 * 120 * 4];
    renderer.render(
        |painter| paint_scene(painter, &doc, 1.0, 120, 120),
        &mut after,
    );
    assert_ne!(
        before, after,
        "initials fallback must change the painted frame"
    );
}

#[test]
fn pointer_events_none_overlay_passes_touch_to_checkbox_button() {
    use blitz_dom::DocumentConfig;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! {
            div { style: "position:relative;width:80px;height:80px",
                button { id: "checkbox", role: "checkbox", style: "position:absolute;left:0;top:0;width:40px;height:40px" }
                input { id: "bubble", r#type: "checkbox", style: "position:absolute;left:0;top:0;width:40px;height:40px;pointer-events:none;opacity:0" }
            }
        }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(80, 80, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    );
    doc.initial_build();
    doc.resolve(0.0);
    let button = doc.query_selector("#checkbox").unwrap().unwrap();
    let bubble = doc.query_selector("#bubble").unwrap().unwrap();
    let hit = doc.hit(20.0, 20.0).unwrap().node_id;
    assert_eq!(
        hit, button,
        "hidden input {bubble} stole checkbox tap: hit {hit}"
    );
}

#[test]
fn native_icon_uses_exact_lucide_geometry_color_and_stroke_width() {
    let source = include_str!("../assets/LUCIDE-SOURCE");
    assert!(source.contains("lucide-static 0.570.0"));
    assert!(
        source.contains("sha256 ff46553f4bc1d2575aa3c1806a14427a9cbdcdb92df7e85c30b2f3cb178f749c")
    );
    let nodes: serde_json::Value =
        serde_json::from_str(include_str!("../assets/lucide-nodes.json")).unwrap();
    let aliases: serde_json::Value =
        serde_json::from_str(include_str!("../assets/lucide-aliases.json")).unwrap();
    assert!(nodes["folder"]
        .as_array()
        .is_some_and(|shapes| !shapes.is_empty()));
    assert_eq!(aliases["close"], "x");
    assert_eq!(aliases["loader"], "loader-circle");

    orbital_client_dioxus::app::CSS_TEXT
        .with(|css| *css.borrow_mut() = orbital_client_dioxus::styles::stylesheets());
    let dir = std::env::temp_dir().join(format!("dioxus-lucide-{}", std::process::id()));
    let red = dir.join("red.png");
    let thick = dir.join("thick.png");
    let blue = dir.join("blue.png");
    let blank = dir.join("blank.png");
    for (path, color, width) in [
        (&red, "#dd0000", 2),
        (&thick, "#dd0000", 4),
        (&blue, "#0000dd", 2),
    ] {
        mount_core(&program(serde_json::json!({
            "type": "icon", "name": "folder", "size": "xl", "strokeWidth": width,
            "style": { "color": color },
        })))
        .unwrap();
        capture_gpu_png(path, 100, 100).unwrap();
    }
    mount_core(&program(serde_json::json!(null))).unwrap();
    capture_gpu_png(&blank, 100, 100).unwrap();
    let red_count = red_pixels(&red);
    let thick_count = red_pixels(&thick);
    let blue_count = blue_pixels(&blue);
    let blank_count = red_pixels(&blank) + blue_pixels(&blank);
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        red_count > blank_count + 10,
        "red Lucide icon painted {red_count} pixels; blank painted {blank_count}"
    );
    assert!(
        blue_count > blank_count + 10,
        "blue Lucide icon painted {blue_count} pixels; blank painted {blank_count}"
    );
    assert!(
        thick_count > red_count + 10,
        "strokeWidth=4 painted {thick_count} pixels; strokeWidth=2 painted {red_count}"
    );
}

#[test]
fn native_icon_inherits_theme_stroke_width_when_unset() {
    let mut css = orbital_client_dioxus::styles::stylesheets();
    css.push(
        "[data-theme=stroke-thin]{--icon-family:lucide;--icon-stroke-width:1}\
         [data-theme=stroke-thick]{--icon-family:lucide;--icon-stroke-width:4}"
            .to_string(),
    );
    orbital_client_dioxus::app::CSS_TEXT.with(|current| *current.borrow_mut() = css);
    let dir = std::env::temp_dir().join(format!("dioxus-icon-theme-stroke-{}", std::process::id()));
    let thin = dir.join("thin.png");
    let thick = dir.join("thick.png");
    for (theme, path) in [("stroke-thin", &thin), ("stroke-thick", &thick)] {
        orbital_client_dioxus::app::THEME
            .with(|current| *current.borrow_mut() = Some(theme.to_string()));
        mount_core(&program(serde_json::json!({
            "type": "icon", "name": "folder", "size": "xl", "style": { "color": "#dd0000" },
        })))
        .unwrap();
        capture_gpu_png(path, 100, 100).unwrap();
    }
    orbital_client_dioxus::app::THEME.with(|current| *current.borrow_mut() = None);
    let thin_pixels = red_pixels(&thin);
    let thick_pixels = red_pixels(&thick);
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        thin_pixels > 10,
        "thin theme painted {thin_pixels} red pixels"
    );
    assert!(
        thick_pixels > thin_pixels + 10,
        "theme stroke width 4 painted {thick_pixels} pixels; width 1 painted {thin_pixels}"
    );
}

#[test]
fn native_icon_ignores_stroke_token_from_inactive_theme_family() {
    let mut css = orbital_client_dioxus::styles::stylesheets();
    css.push(
        "[data-theme=phosphor-thin]{--icon-family:phosphor-outline;--icon-stroke-width:1}\
         [data-theme=phosphor-thick]{--icon-family:phosphor-outline;--icon-stroke-width:4}\
         [data-theme=lucide-thick]{--icon-family:lucide;--icon-stroke-width:4}"
            .to_string(),
    );
    orbital_client_dioxus::app::CSS_TEXT.with(|current| *current.borrow_mut() = css);
    let dir =
        std::env::temp_dir().join(format!("dioxus-icon-family-stroke-{}", std::process::id()));
    let mut counts = Vec::new();
    for theme in ["phosphor-thin", "phosphor-thick", "lucide-thick"] {
        let path = dir.join(format!("{theme}.png"));
        orbital_client_dioxus::app::THEME
            .with(|current| *current.borrow_mut() = Some(theme.to_string()));
        mount_core(&program(serde_json::json!({
            "type": "icon", "name": "folder", "size": "xl", "style": { "color": "#dd0000" },
        })))
        .unwrap();
        capture_gpu_png(&path, 100, 100).unwrap();
        counts.push(red_pixels(&path));
    }
    orbital_client_dioxus::app::THEME.with(|current| *current.borrow_mut() = None);
    std::fs::remove_dir_all(&dir).ok();
    assert!(counts[0] > 10, "inactive-family fallback painted no icon");
    assert_eq!(
        counts[0], counts[1],
        "inactive-family token changed Lucide fallback stroke"
    );
    assert!(
        counts[2] > counts[1] + 10,
        "active Lucide family did not use its stroke token: {counts:?}"
    );
}

#[test]
fn text_on_the_main_slot_paints_dark_pixels_and_a_blank_page_does_not() {
    let dir = std::env::temp_dir().join(format!("dioxus-capture-{}", std::process::id()));
    mount_core(&program(
        serde_json::json!({ "type": "typography", "variant": "h1", "content": "Hello capture" }),
    ))
    .unwrap();
    let text = dir.join("text.png");
    capture_png(&text, 400, 200).unwrap();
    mount_core(&program(serde_json::json!(null))).unwrap();
    let blank = dir.join("blank.png");
    capture_png(&blank, 400, 200).unwrap();
    let (with_text, without) = (dark_pixels(&text), dark_pixels(&blank));
    std::fs::remove_dir_all(&dir).ok();
    assert!(with_text > 50, "text painted {with_text} dark pixels");
    assert!(
        without < with_text / 10,
        "control: blank page painted {without}"
    );
}

#[test]
fn gpu_paints_the_same_slot_with_visible_text() {
    let dir = std::env::temp_dir().join(format!("dioxus-gpu-capture-{}", std::process::id()));
    mount_core(&program(
        serde_json::json!({ "type": "typography", "variant": "h1", "content": "GPU TODO" }),
    ))
    .unwrap();
    let text = dir.join("text.png");
    capture_gpu_png(&text, 400, 200).unwrap();
    mount_core(&program(serde_json::json!(null))).unwrap();
    let blank = dir.join("blank.png");
    capture_gpu_png(&blank, 400, 200).unwrap();
    let (with_text, without) = (dark_pixels(&text), dark_pixels(&blank));
    std::fs::remove_dir_all(&dir).ok();
    assert!(with_text > 50, "GPU painted {with_text} dark pixels");
    assert!(
        without < with_text / 10,
        "control: blank page painted {without}"
    );
}

#[test]
fn gpu_paints_svg_ring_stroke_pixels() {
    let dir = std::env::temp_dir().join(format!("dioxus-svg-gpu-capture-{}", std::process::id()));
    mount_core(&program(serde_json::json!({
        "type": "svg-ring",
        "cx": 50,
        "cy": 50,
        "r": 36,
        "strokeWidth": 6,
        "color": "#ff0000",
        "width": 100,
        "height": 100
    })))
    .unwrap();
    let ring = dir.join("ring.png");
    capture_gpu_png(&ring, 400, 200).unwrap();
    mount_core(&program(serde_json::json!(null))).unwrap();
    let blank = dir.join("blank.png");
    capture_gpu_png(&blank, 400, 200).unwrap();
    let (painted, control) = (red_pixels(&ring), red_pixels(&blank));
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        painted > 100,
        "SVG ring GPU frame painted {painted} red pixels"
    );
    assert!(
        control < painted / 10,
        "blank GPU frame painted {control} red pixels"
    );
}

#[test]
fn pattern_tile_canvas_mapper_paints_headless_gpu_pixels() {
    use orbital_canvas::mappers::scene;
    use orbital_canvas::Canvas;

    let registry = orbital_client_dioxus::registry::default_registry();
    assert!(registry.supports("pattern-tile"));
    assert!(registry.is_partial("pattern-tile"));
    assert_eq!(
        registry.get_renderer("pattern-tile").pattern_name(),
        "pattern-tile"
    );
    let star = scene::map_scene(
        &serde_json::json!({
            "type": "pattern-tile", "variant": "star8", "size": 120,
            "color": "#101010", "strokeWidth": 4
        }),
        None,
    )
    .unwrap()
    .packet;
    let knot = scene::map_scene(
        &serde_json::json!({
            "type": "pattern-tile", "variant": "celtic-knot", "size": 120,
            "color": "#101010", "strokeWidth": 4
        }),
        None,
    )
    .unwrap()
    .packet;
    assert!(!star.draw_commands.is_empty());
    assert!(!knot.draw_commands.is_empty());
    let mut canvas = Canvas::new_headless().expect("PatternTile GPU adapter");
    let painted = canvas
        .render_packet_to_rgba(&star)
        .expect("star8 GPU paint");
    let other = canvas
        .render_packet_to_rgba(&knot)
        .expect("celtic-knot GPU paint");
    assert_eq!(painted.len(), (star.width * star.height * 4.0) as usize);
    assert!(
        painted.chunks_exact(4).any(|pixel| pixel[3] != 0),
        "PatternTile GPU frame is blank"
    );
    assert_ne!(
        painted, other,
        "different shared variants painted identical GPU pixels"
    );
}

#[test]
fn positioned_drawer_paints_its_semantic_children_above_the_backdrop() {
    use anyrender::ImageRenderer;
    use blitz_dom::DocumentConfig;
    use blitz_paint::paint_scene;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! {
            div { style: "position:relative;display:flex;flex-direction:column;width:100%;height:100%;min-height:100vh;overflow:hidden",
                div { "Main view" }
                button { style: "position:absolute;inset:0;background:rgba(0,0,0,0.35)" }
                div { class: "drawer", style: "position:absolute;top:0;bottom:0;left:0;width:min(320px,85vw);overflow:auto;background:white",
                    button { "×" }
                    h2 { "Menu" }
                    button { "Assets" }
                }
            }
        }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Light)),
            font_ctx: Some(orbital_client_dioxus::offscreen::embedded_font_context()),
            ..Default::default()
        },
    );
    doc.initial_build();
    doc.resolve(0.0);
    let id = doc.query_selector(".drawer").unwrap().unwrap();
    let node = doc.get_node(id).unwrap();
    assert!(node.text_content().contains("Assets"));
    assert!(
        node.final_layout.size.width > 250.0,
        "drawer width: {}",
        node.final_layout.size.width
    );
    assert!(
        node.final_layout.size.height > 200.0,
        "drawer height: {}",
        node.final_layout.size.height
    );
    let mut renderer = anyrender_vello::VelloImageRenderer::new(400, 300);
    let mut pixels = vec![0u8; 400 * 300 * 4];
    renderer.render(
        |painter| paint_scene(painter, &doc, 1.0, 400, 300),
        &mut pixels,
    );
    let dark = pixels
        .chunks_exact(4)
        .enumerate()
        .filter(|(index, rgba)| {
            index % 400 < 320 && rgba[0] < 128 && rgba[1] < 128 && rgba[2] < 128
        })
        .count();
    assert!(
        dark > 100,
        "drawer semantic text failed to paint: {dark} dark pixels"
    );
}

#[test]
fn drawer_backdrop_uses_containing_block_only_when_contained() {
    use anyrender::ImageRenderer;
    use blitz_dom::DocumentConfig;
    use blitz_paint::paint_scene;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! {
            div { style: "position:relative;left:40px;top:30px;width:200px;height:160px;overflow:hidden",
                div { id: "contained-backdrop", class: "fixed inset-0 z-40 almadar-drawer-overlay-contained", style: "background:#ff0000" }
                div { id: "viewport-backdrop", class: "fixed inset-0 z-40 almadar-drawer-overlay-viewport", style: "background:#000000",
                    div { id: "nested-fixed", class: "fixed", style: "top:0;right:0;width:20px;height:20px;z-index:70;background:#0000ff" }
                }
            }
            div { style: "height:1000px" }
        }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(500, 400, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    );
    for stylesheet in orbital_client_dioxus::styles::stylesheets() {
        doc.add_user_agent_stylesheet(&stylesheet);
    }
    doc.initial_build();
    doc.resolve(0.0);
    let layout = |selector| {
        let id = doc.query_selector(selector).unwrap().unwrap();
        doc.get_node(id).unwrap().final_layout
    };
    let contained = layout("#contained-backdrop");
    let viewport = layout("#viewport-backdrop");
    assert!(
        (contained.size.width - 200.0).abs() < 1.0,
        "contained width: {}",
        contained.size.width
    );
    assert!(
        (contained.size.height - 160.0).abs() < 1.0,
        "contained height: {}",
        contained.size.height
    );
    assert!(
        (viewport.size.width - 500.0).abs() < 1.0,
        "viewport width: {}",
        viewport.size.width
    );
    assert!(
        (viewport.size.height - 400.0).abs() < 1.0,
        "viewport height: {}",
        viewport.size.height
    );
    assert!(
        viewport.location.x.abs() < 1.0,
        "viewport x: {}",
        viewport.location.x
    );
    assert!(
        viewport.location.y.abs() < 1.0,
        "viewport y: {}",
        viewport.location.y
    );
    let viewport_id = doc.query_selector("#viewport-backdrop").unwrap().unwrap();
    assert_eq!(doc.hit(450.0, 300.0).unwrap().node_id, viewport_id);
    let mut renderer = anyrender_vello_cpu::VelloCpuImageRenderer::new(500, 400);
    let mut pixels = vec![0u8; 500 * 400 * 4];
    renderer.render(
        |painter| paint_scene(painter, &doc, 1.0, 500, 400),
        &mut pixels,
    );
    let outside_parent = &pixels[(300 * 500 + 450) * 4..(300 * 500 + 451) * 4];
    assert!(
        outside_parent[0] < 32 && outside_parent[1] < 32 && outside_parent[2] < 32,
        "viewport backdrop was clipped by its ancestor: {outside_parent:?}"
    );
    let nested_id = doc.query_selector("#nested-fixed").unwrap().unwrap();
    assert_eq!(doc.hit(490.0, 10.0).unwrap().node_id, nested_id);
    doc.set_viewport_scroll(blitz_dom::Point { x: 0.0, y: 200.0 });
    assert_eq!(doc.hit(450.0, 500.0).unwrap().node_id, viewport_id);
    assert_eq!(doc.hit(490.0, 210.0).unwrap().node_id, nested_id);
    renderer.render(
        |painter| paint_scene(painter, &doc, 1.0, 500, 400),
        &mut pixels,
    );
    let scrolled_backdrop = &pixels[(300 * 500 + 450) * 4..(300 * 500 + 451) * 4];
    assert!(
        scrolled_backdrop[0] < 32 && scrolled_backdrop[1] < 32 && scrolled_backdrop[2] < 32,
        "viewport backdrop moved during scroll: {scrolled_backdrop:?}"
    );
    let scrolled_nested = &pixels[(10 * 500 + 490) * 4..(10 * 500 + 491) * 4];
    assert!(
        scrolled_nested[0] < 32 && scrolled_nested[1] < 32 && scrolled_nested[2] > 200,
        "nested fixed child moved or clipped during scroll: {scrolled_nested:?}"
    );
}

#[test]
fn live_document_replaces_text_after_signal_change() {
    use anyrender::ImageRenderer;
    use blitz_dom::{Document as _, DocumentConfig};
    use blitz_paint::paint_scene;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    thread_local! {
        static TEXT: std::cell::RefCell<Option<Signal<String>>> = const { std::cell::RefCell::new(None) };
    }

    fn app() -> Element {
        let text = use_signal(|| "No tasks yet".to_string());
        TEXT.with(|cell| *cell.borrow_mut() = Some(text));
        let current = text.read().clone();
        rsx! { div { "{current}" } }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(400, 100, 1.0, ColorScheme::Light)),
            font_ctx: Some(orbital_client_dioxus::offscreen::embedded_font_context()),
            ..Default::default()
        },
    );
    doc.add_user_agent_stylesheet("div { display: block; color: black; font-size: 20px; }");
    doc.initial_build();
    doc.resolve(0.0);
    let html_id = doc.root_node().children[0];
    assert!(doc
        .get_node(html_id)
        .unwrap()
        .text_content()
        .contains("No tasks yet"));
    let mut renderer = anyrender_vello_cpu::VelloCpuImageRenderer::new(400, 100);
    let mut before = vec![0u8; 400 * 100 * 4];
    renderer.render(
        |painter| paint_scene(painter, &doc, 1.0, 400, 100),
        &mut before,
    );
    assert!(before.iter().any(|v| *v != 0), "initial text did not paint");

    TEXT.with(|cell| cell.borrow_mut().as_mut().unwrap().set("Tasks".to_string()));
    assert!(doc.poll(None));
    doc.resolve(0.0);
    let text = doc.get_node(html_id).unwrap().text_content();
    assert!(text.contains("Tasks"), "{text}");
    assert!(!text.contains("No tasks yet"), "{text}");
    let mut after = vec![0u8; 400 * 100 * 4];
    renderer.render(
        |painter| paint_scene(painter, &doc, 1.0, 400, 100),
        &mut after,
    );
    assert!(before != after, "text changed in DOM but not in paint");
}

#[test]
fn native_css_keyframes_change_computed_opacity_over_time() {
    use blitz_dom::DocumentConfig;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! { div { class: "animated", "Animated" } }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(200, 100, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    );
    doc.add_user_agent_stylesheet(
        ".animated { display: block; width: 100px; height: 80px; background: black; animation: entry-opacity 1s linear both; } \
         @keyframes entry-opacity { from { opacity: 0; } to { opacity: 1; } }",
    );
    doc.initial_build();
    let node_id = doc.query_selector(".animated").unwrap().unwrap();
    let opacity = |doc: &DioxusDocument| {
        doc.get_node(node_id)
            .unwrap()
            .primary_styles()
            .unwrap()
            .get_effects()
            .opacity
    };

    doc.resolve(0.0);
    let beginning = opacity(&doc);
    assert!(
        doc.is_animating(),
        "running CSS keyframes must request redraws"
    );
    doc.resolve(0.5);
    let middle = opacity(&doc);
    doc.resolve(1.0);
    let end = opacity(&doc);
    assert!(beginning < 0.1, "initial opacity: {beginning}");
    assert!(middle > 0.35 && middle < 0.65, "midpoint opacity: {middle}");
    assert!(end > 0.9, "final opacity: {end}");
}

#[test]
fn native_reduced_motion_root_collapses_html_keyframes() {
    use blitz_dom::DocumentConfig;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! { div { "data-reduced-motion": "true", div { class: "animated", "Animated" } } }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(200, 100, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    );
    doc.add_user_agent_stylesheet(include_str!("../assets/css/almadar.css"));
    doc.add_user_agent_stylesheet(
        ".animated { animation: entry-opacity 1s linear both; } \
         @keyframes entry-opacity { from { opacity: 0; } to { opacity: 1; } }",
    );
    doc.initial_build();
    doc.resolve(0.0);
    doc.resolve(0.5);
    let node_id = doc.query_selector(".animated").unwrap().unwrap();
    let opacity = doc
        .get_node(node_id)
        .unwrap()
        .primary_styles()
        .unwrap()
        .get_effects()
        .opacity;
    assert!(opacity > 0.9, "reduced-motion opacity: {opacity}");
}

#[test]
fn native_css_keyframes_change_gpu_pixels_over_time() {
    use anyrender::ImageRenderer;
    use blitz_dom::DocumentConfig;
    use blitz_paint::paint_scene;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! { div { class: "animated", span { class: "tile" } } }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(160, 100, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    );
    doc.add_user_agent_stylesheet(
        ".animated { display: block; width: 100px; height: 80px; animation: entry-opacity 1s linear both; } \
         .tile { display: block; width: 100px; height: 80px; background: black; } \
         @keyframes entry-opacity { from { opacity: 0; } to { opacity: 1; } }",
    );
    doc.initial_build();
    let mut renderer = anyrender_vello::VelloImageRenderer::new(160, 100);
    let mut alpha_at = |time| {
        doc.resolve(time);
        let mut pixels = vec![0u8; 160 * 100 * 4];
        renderer.render(
            |painter| paint_scene(painter, &doc, 1.0, 160, 100),
            &mut pixels,
        );
        pixels
            .chunks_exact(4)
            .map(|pixel| u64::from(pixel[3]))
            .sum::<u64>()
    };
    let beginning = alpha_at(0.0);
    let middle = alpha_at(0.5);
    let end = alpha_at(1.0);
    assert!(
        beginning < middle,
        "GPU opacity did not rise: {beginning}, {middle}, {end}"
    );
    assert!(
        middle < end,
        "GPU opacity did not finish: {beginning}, {middle}, {end}"
    );
}

#[test]
fn native_css_keyframes_fade_the_elements_own_background_on_gpu() {
    use anyrender::ImageRenderer;
    use blitz_dom::DocumentConfig;
    use blitz_paint::paint_scene;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! { div { class: "animated" } }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(160, 100, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    );
    doc.add_user_agent_stylesheet(
        ".animated { display: block; width: 100px; height: 80px; background: black; animation: entry-opacity 1s linear both; } \
         @keyframes entry-opacity { from { opacity: 0; } to { opacity: 1; } }",
    );
    doc.initial_build();
    let mut renderer = anyrender_vello::VelloImageRenderer::new(160, 100);
    let mut alpha_at = |time| {
        doc.resolve(time);
        let mut pixels = vec![0u8; 160 * 100 * 4];
        renderer.render(
            |painter| paint_scene(painter, &doc, 1.0, 160, 100),
            &mut pixels,
        );
        pixels
            .chunks_exact(4)
            .map(|pixel| u64::from(pixel[3]))
            .sum::<u64>()
    };
    let beginning = alpha_at(0.0);
    let middle = alpha_at(0.5);
    let end = alpha_at(1.0);
    assert!(
        beginning < middle,
        "GPU background opacity did not rise: {beginning}, {middle}, {end}"
    );
    assert!(
        middle < end,
        "GPU background opacity did not finish: {beginning}, {middle}, {end}"
    );
}

#[test]
fn native_entry_delay_uses_the_shared_theme_stylesheet() {
    use blitz_dom::DocumentConfig;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! { div { class: "almadar-enter-fade almadar-enter-delay-3" } }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(160, 100, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    );
    doc.add_user_agent_stylesheet(&orbital_client_dioxus::styles::stylesheets()[0]);
    doc.initial_build();
    let node_id = doc.query_selector(".almadar-enter-fade").unwrap().unwrap();
    let opacity = |doc: &DioxusDocument| {
        doc.get_node(node_id)
            .unwrap()
            .primary_styles()
            .unwrap()
            .get_effects()
            .opacity
    };

    doc.resolve(0.0);
    let beginning = opacity(&doc);
    doc.resolve(0.1);
    let delayed = opacity(&doc);
    doc.resolve(0.3);
    let active = opacity(&doc);
    doc.resolve(0.5);
    let end = opacity(&doc);
    assert!(beginning < 0.1, "entry initial opacity: {beginning}");
    assert!(delayed < 0.1, "entry stagger delay did not hold: {delayed}");
    assert!(
        active > delayed,
        "entry did not progress: {delayed}, {active}"
    );
    assert!(end > 0.9, "entry did not finish: {end}");
}

#[test]
#[ignore = "tool: writes a PNG for a person to look at: ORB_CAPTURE_OUT=<file>"]
fn look() {
    let out = std::env::var("ORB_CAPTURE_OUT").expect("ORB_CAPTURE_OUT");
    mount_core(&program(serde_json::json!({ "type": "stack", "children": [
        { "type": "typography", "variant": "h1", "content": "Hello capture" },
        { "type": "button", "label": "Save", "event": "SAVE" }
    ] })))
    .unwrap();
    orbital_client_dioxus::app::CSS_TEXT
        .with(|c| *c.borrow_mut() = orbital_client_dioxus::styles::stylesheets());
    capture_png(std::path::Path::new(&out), 600, 300).unwrap();
}

#[test]
fn items_center_flex_column_behind_a_contents_wrapper_centres_its_children() {
    use blitz_dom::DocumentConfig;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! {
            div { class: "flex min-h-full flex-col",
                div { class: "contents",
                    div { class: "flex flex-col gap-4 items-center justify-center py-12 text-center",
                        div { id: "wrapped", class: "mb-4 rounded-full p-3", div { class: "w-8 h-8" } }
                    }
                }
                div { class: "flex flex-col gap-4 items-center justify-center py-12 text-center",
                    div { id: "direct", class: "mb-4 rounded-full p-3", div { class: "w-8 h-8" } }
                }
            }
        }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(600, 900, 1.0, ColorScheme::Light)),
            font_ctx: Some(orbital_client_dioxus::offscreen::embedded_font_context()),
            ..Default::default()
        },
    );
    doc.initial_build();
    doc.create_head_element(
        "style",
        &[],
        &Some(include_str!("../assets/css/web-client.css").into()),
    );
    doc.resolve(0.0);
    let width = |selector| {
        let id = doc.query_selector(selector).unwrap().unwrap();
        doc.get_node(id).unwrap().final_layout.size.width
    };
    let direct = width("#direct");
    let wrapped = width("#wrapped");
    assert!(direct < 100.0, "control: centred pill shrinks to its content, got {direct}");
    assert!(
        (wrapped - direct).abs() < 1.0,
        "display:contents wrapper stretched the centred pill: wrapped={wrapped} direct={direct}"
    );
}

#[test]
fn aspect_square_overflow_hidden_grid_items_give_their_rows_height_in_a_flex_column() {
    use blitz_dom::DocumentConfig;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! {
            div { class: "flex min-h-full flex-col",
                div { id: "clipped", class: "grid gap-3 grid-cols-3",
                    for i in 0..6 {
                        div { key: "{i}", class: "relative overflow-hidden aspect-square", div { class: "w-full h-full" } }
                    }
                }
                div { id: "visible", class: "grid gap-3 grid-cols-3",
                    for i in 0..6 {
                        div { key: "{i}", class: "relative aspect-square", div { class: "w-full h-full" } }
                    }
                }
            }
        }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(312, 900, 1.0, ColorScheme::Light)),
            font_ctx: Some(orbital_client_dioxus::offscreen::embedded_font_context()),
            ..Default::default()
        },
    );
    doc.initial_build();
    doc.create_head_element(
        "style",
        &[],
        &Some(include_str!("../assets/css/web-client.css").into()),
    );
    doc.resolve(0.0);
    let height = |selector| {
        let id = doc.query_selector(selector).unwrap().unwrap();
        doc.get_node(id).unwrap().final_layout.size.height
    };
    let visible = height("#visible");
    let clipped = height("#clipped");
    assert!(
        visible > 190.0,
        "control: two rows of 96px squares plus a gap, got {visible}"
    );
    assert!(
        (clipped - visible).abs() <= 1.5,
        "overflow-hidden rows collapsed: clipped={clipped} visible={visible}"
    );
}

#[test]
fn tiled_hairline_grid_gradient_keeps_the_page_white_between_lines_on_gpu() {
    use anyrender::ImageRenderer;
    use blitz_dom::DocumentConfig;
    use blitz_paint::paint_scene;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn app() -> Element {
        rsx! { div { class: "grid-page" } }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(160, 160, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    );
    doc.add_user_agent_stylesheet(
        ".grid-page { display: block; width: 160px; height: 900px; background-color: white; \
         background-image: linear-gradient(rgba(17,17,17,0.5) 1px, transparent 1px), \
         linear-gradient(90deg, rgba(17,17,17,0.5) 1px, transparent 1px); \
         background-size: 64px 64px; }",
    );
    doc.initial_build();
    doc.resolve(0.0);
    let mut renderer = anyrender_vello::VelloImageRenderer::new(160, 160);
    let mut pixels = vec![0u8; 160 * 160 * 4];
    renderer.render(
        |painter| paint_scene(painter, &doc, 1.0, 160, 160),
        &mut pixels,
    );
    let px = |x: usize, y: usize| pixels[(y * 160 + x) * 4];
    assert!(
        px(30, 30) > 250 && px(40, 100) > 250,
        "page between grid lines is not white: {}/{}",
        px(30, 30),
        px(40, 100)
    );
    assert!(
        px(30, 0) < 200 && px(0, 30) < 200,
        "control: the 1px grid lines must still paint: {}/{}",
        px(30, 0),
        px(0, 30)
    );
}

#[test]
fn card_title_beside_an_action_group_keeps_one_line_when_it_fits() {
    use blitz_dom::DocumentConfig;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus::prelude::*;
    use dioxus_native::DioxusDocument;

    fn row(prefix: &'static str, title: &'static str) -> Element {
        let title_id = format!("{prefix}-title");
        let row_id = format!("{prefix}-row");
        rsx! {
            div { class: "flex flex-row gap-2 justify-between items-start", id: "{row_id}", style: "width: 280px",
                div { class: "flex flex-col gap-1 items-stretch justify-start flex-1 min-w-0",
                    div { class: "flex flex-row gap-1 justify-start items-center min-w-0",
                        div { class: "min-w-0",
                            h4 { id: "{title_id}", class: "text-xl font-semibold min-w-0 line-clamp-2 break-words",
                                style: "font-weight: 800; letter-spacing: -0.03em", "{title}" }
                        }
                    }
                }
                div { class: "flex flex-row gap-1 items-stretch justify-start flex-shrink-0",
                    button { class: "inline-flex items-center justify-center gap-2 h-button-sm px-3 text-sm", "Send" }
                    button { class: "inline-flex items-center justify-center gap-2 h-button-sm px-3 text-sm", "..." }
                }
            }
        }
    }

    fn app() -> Element {
        rsx! {
            div { class: "flex flex-col gap-4",
                {row("fits", "Glen Ridge")}
                {row("wraps", "Estuary Juniper Meridian")}
            }
        }
    }

    let mut doc = DioxusDocument::new(
        dioxus_core::VirtualDom::new(app),
        DocumentConfig {
            viewport: Some(Viewport::new(320, 200, 1.0, ColorScheme::Light)),
            font_ctx: Some(orbital_client_dioxus::offscreen::embedded_font_context()),
            ..Default::default()
        },
    );
    doc.initial_build();
    doc.create_head_element(
        "style",
        &[],
        &Some(include_str!("../assets/css/web-client.css").into()),
    );
    doc.resolve(0.0);
    let measure = |selector: &str| {
        let id = doc.query_selector(selector).unwrap().unwrap();
        let node = doc.get_node(id).unwrap();
        let text = node
            .element_data()
            .unwrap()
            .inline_layout_data
            .as_ref()
            .unwrap();
        (
            node.final_layout.size,
            text.layout.len(),
            text.layout.width(),
        )
    };
    let (box_size, painted_lines, painted_width) = measure("#fits-title");
    let line_height = box_size.height / painted_lines.max(1) as f32;
    assert!(
        painted_lines == 1 && box_size.height < line_height * 1.5,
        "title that fits wrapped: lines={painted_lines} box={box_size:?} text_width={painted_width}"
    );
    let (wrapped_box, wrapped_lines, _) = measure("#wraps-title");
    assert!(
        wrapped_lines >= 2 && wrapped_box.height > box_size.height * 1.5,
        "control: a title that cannot fit must wrap and grow its box: lines={wrapped_lines} box={wrapped_box:?}"
    );
}
