//! The built `orb-desktop` speaks the desktop protocol across a real process boundary.
#![cfg(feature = "desktop")]

use orbital_core::client::render_probe::GameCanvasOutcome;
use orbital_core::desktop_protocol::{request, DesktopReply, DesktopRequest};
use orbital_core::OrbitalSchema;
use std::path::Path;

const BINARY: &str = env!("CARGO_BIN_EXE_orb-desktop");

fn program() -> OrbitalSchema {
    serde_json::from_value(serde_json::json!({
        "name": "t",
        "typeContractVersion": 1,
        "orbitals": [{
            "name": "Main",
            "entity": { "name": "Row", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "title", "type": "string" }] },
            "traits": [{ "name": "View", "linkedEntity": "Row", "stateMachine": {
                "states": [{ "name": "idle", "isInitial": true }, { "name": "open" }],
                "transitions": [
                    { "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": "Hello frames" }]] },
                    { "from": "idle", "to": "open", "event": "OPEN", "effects": [["render-ui", "main", { "type": "typography", "content": "Opened" }]] }
                ]
            } }],
            "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "View" }] }]
        }]
    }))
    .unwrap()
}

fn files(reply: DesktopReply) -> Vec<std::path::PathBuf> {
    match reply {
        DesktopReply::Files(paths) => paths,
        other => panic!("expected files, got {other:?}"),
    }
}

#[test]
fn dioxus_coverage_lists_supported_patterns_and_the_declared_stubs() {
    let DesktopReply::Coverage { supported, stubbed } = request(Path::new(BINARY), DesktopRequest::Coverage { renderer: "dioxus".into() }).unwrap() else { panic!() };
    assert!(supported.iter().any(|p| p == "typography"));
    assert_eq!(stubbed.len(), orbital_client_dioxus::registry::STUBBED_PATTERNS.len());
}

#[test]
fn control_the_terminal_renderer_declares_no_stubs() {
    let DesktopReply::Coverage { supported, stubbed } = request(Path::new(BINARY), DesktopRequest::Coverage { renderer: "tui".into() }).unwrap() else { panic!() };
    assert!(supported.len() > 200 && stubbed.is_empty());
}

#[test]
fn edge_an_unknown_renderer_is_refused_by_name() {
    let err = request(Path::new(BINARY), DesktopRequest::Coverage { renderer: "egui".into() }).unwrap_err();
    assert!(err.message.contains("unknown renderer \"egui\""), "{}", err.message);
}

#[test]
fn the_terminal_cannot_paint_a_canvas_and_says_so() {
    let reply = request(Path::new(BINARY), DesktopRequest::PaintCanvas { renderer: "tui".into(), pattern: "mini-map".into(), props: orbital_core::Value::Null }).unwrap();
    assert!(matches!(reply, DesktopReply::Canvas(GameCanvasOutcome::Skip(_))));
}

#[test]
fn screenshots_write_one_png_per_orbital_and_text_for_the_terminal() {
    let out = std::env::temp_dir().join(format!("orb-desktop-shots-{}", std::process::id()));
    let png = files(request(Path::new(BINARY), DesktopRequest::Screenshot { renderer: "dioxus".into(), item: "t".into(), schema: Box::new(program()), out_dir: out.clone() }).unwrap());
    let txt = files(request(Path::new(BINARY), DesktopRequest::Screenshot { renderer: "tui".into(), item: "t".into(), schema: Box::new(program()), out_dir: out.clone() }).unwrap());
    assert_eq!(png, vec![out.join("t.orbital0.png")]);
    assert!(std::fs::read_to_string(&txt[0]).unwrap().contains("Hello frames"));
    std::fs::remove_dir_all(&out).unwrap();
}
