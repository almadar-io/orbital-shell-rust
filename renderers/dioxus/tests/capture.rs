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
    buf[..info.buffer_size()].chunks(4).filter(|px| px[0] < 128 && px[1] < 128 && px[2] < 128).count()
}

#[test]
fn text_on_the_main_slot_paints_dark_pixels_and_a_blank_page_does_not() {
    let dir = std::env::temp_dir().join(format!("dioxus-capture-{}", std::process::id()));
    mount_core(&program(serde_json::json!({ "type": "typography", "variant": "h1", "content": "Hello capture" }))).unwrap();
    let text = dir.join("text.png");
    capture_png(&text, 400, 200).unwrap();
    mount_core(&program(serde_json::json!(null))).unwrap();
    let blank = dir.join("blank.png");
    capture_png(&blank, 400, 200).unwrap();
    let (with_text, without) = (dark_pixels(&text), dark_pixels(&blank));
    std::fs::remove_dir_all(&dir).ok();
    assert!(with_text > 50, "text painted {with_text} dark pixels");
    assert!(without < with_text / 10, "control: blank page painted {without}");
}

#[test]
fn gpu_paints_the_same_slot_with_visible_text() {
    let dir = std::env::temp_dir().join(format!("dioxus-gpu-capture-{}", std::process::id()));
    mount_core(&program(serde_json::json!({ "type": "typography", "variant": "h1", "content": "GPU TODO" }))).unwrap();
    let text = dir.join("text.png");
    capture_gpu_png(&text, 400, 200).unwrap();
    mount_core(&program(serde_json::json!(null))).unwrap();
    let blank = dir.join("blank.png");
    capture_gpu_png(&blank, 400, 200).unwrap();
    let (with_text, without) = (dark_pixels(&text), dark_pixels(&blank));
    std::fs::remove_dir_all(&dir).ok();
    assert!(with_text > 50, "GPU painted {with_text} dark pixels");
    assert!(without < with_text / 10, "control: blank page painted {without}");
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

    let mut doc = DioxusDocument::new(dioxus_core::VirtualDom::new(app), DocumentConfig {
        viewport: Some(Viewport::new(400, 100, 1.0, ColorScheme::Light)),
        font_ctx: Some(orbital_client_dioxus::offscreen::embedded_font_context()),
        ..Default::default()
    });
    doc.add_user_agent_stylesheet("div { display: block; color: black; font-size: 20px; }");
    doc.initial_build();
    doc.resolve(0.0);
    let html_id = doc.root_node().children[0];
    assert!(doc.get_node(html_id).unwrap().text_content().contains("No tasks yet"));
    let mut renderer = anyrender_vello_cpu::VelloCpuImageRenderer::new(400, 100);
    let mut before = vec![0u8; 400 * 100 * 4];
    renderer.render(|painter| paint_scene(painter, &doc, 1.0, 400, 100), &mut before);
    assert!(before.iter().any(|v| *v != 0), "initial text did not paint");

    TEXT.with(|cell| cell.borrow_mut().as_mut().unwrap().set("Tasks".to_string()));
    assert!(doc.poll(None));
    doc.resolve(0.0);
    let text = doc.get_node(html_id).unwrap().text_content();
    assert!(text.contains("Tasks"), "{text}");
    assert!(!text.contains("No tasks yet"), "{text}");
    let mut after = vec![0u8; 400 * 100 * 4];
    renderer.render(|painter| paint_scene(painter, &doc, 1.0, 400, 100), &mut after);
    assert!(before != after, "text changed in DOM but not in paint");
}

#[test]
#[ignore = "tool: writes a PNG for a person to look at: ORB_CAPTURE_OUT=<file>"]
fn look() {
    let out = std::env::var("ORB_CAPTURE_OUT").expect("ORB_CAPTURE_OUT");
    mount_core(&program(serde_json::json!({ "type": "stack", "children": [
        { "type": "typography", "variant": "h1", "content": "Hello capture" },
        { "type": "button", "label": "Save", "event": "SAVE" }
    ] }))).unwrap();
    orbital_client_dioxus::app::CSS_TEXT.with(|c| *c.borrow_mut() = orbital_client_dioxus::styles::stylesheets());
    capture_png(std::path::Path::new(&out), 600, 300).unwrap();
}
