//! The offscreen capture paints the mounted app to a PNG with no window.
#![cfg(all(feature = "native", not(target_arch = "wasm32")))]

use orbital_client_dioxus::offscreen::capture_png;
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
#[ignore = "writes a PNG for a person to look at: ORB_CAPTURE_OUT=<file>"]
fn look() {
    let out = std::env::var("ORB_CAPTURE_OUT").expect("ORB_CAPTURE_OUT");
    mount_core(&program(serde_json::json!({ "type": "stack", "children": [
        { "type": "typography", "variant": "h1", "content": "Hello capture" },
        { "type": "button", "label": "Save", "event": "SAVE" }
    ] }))).unwrap();
    orbital_client_dioxus::app::CSS_TEXT.with(|c| *c.borrow_mut() = orbital_client_dioxus::styles::stylesheets());
    capture_png(std::path::Path::new(&out), 600, 300).unwrap();
}
