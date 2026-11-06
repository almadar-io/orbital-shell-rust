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
        }, {
            "name": "Second",
            "entity": { "name": "Note", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
            "traits": [{ "name": "OtherView", "linkedEntity": "Note", "stateMachine": {
                "states": [{ "name": "idle", "isInitial": true }],
                "transitions": [
                    { "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": "Other page" }]] }
                ]
            } }],
            "pages": [{ "name": "Other", "path": "/other", "traits": [{ "ref": "OtherView" }] }]
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
    let png = files(request(Path::new(BINARY), DesktopRequest::Screenshot { renderer: "dioxus".into(), item: "t".into(), schema: Box::new(program()), out_dir: out.clone(), route: None, server: None, program_path: None }).unwrap());
    let txt = files(request(Path::new(BINARY), DesktopRequest::Screenshot { renderer: "tui".into(), item: "t".into(), schema: Box::new(program()), out_dir: out.clone(), route: None, server: None, program_path: None }).unwrap());
    assert_eq!(png, vec![out.join("t.orbital0.png"), out.join("t.orbital1.png")]);
    assert!(std::fs::read_to_string(&txt[0]).unwrap().contains("Hello frames"));
    std::fs::remove_dir_all(&out).unwrap();
}

fn route_shot(route: &str, out: &Path) -> String {
    let txt = files(request(Path::new(BINARY), DesktopRequest::Screenshot { renderer: "tui".into(), item: "r".into(), schema: Box::new(program()), out_dir: out.to_path_buf(), route: Some(route.into()), server: None, program_path: None }).unwrap());
    assert_eq!(txt, vec![out.join("r.txt")]);
    std::fs::read_to_string(&txt[0]).unwrap()
}

#[test]
fn a_route_screenshot_captures_the_page_that_route_mount_opens() {
    let out = std::env::temp_dir().join(format!("orb-desktop-route-{}", std::process::id()));
    let other = route_shot("/other", &out);
    assert!(other.contains("Other page") && !other.contains("Hello frames"), "{other}");
    let home = route_shot("/", &out);
    assert!(home.contains("Hello frames") && !home.contains("Other page"), "{home}");
    std::fs::remove_dir_all(&out).unwrap();
}

fn boot_only_program() -> OrbitalSchema {
    serde_json::from_value(serde_json::json!({
        "name": "b",
        "typeContractVersion": 1,
        "orbitals": [{
            "name": "Boot",
            "entity": { "name": "Run", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
            "traits": [{ "name": "Work", "linkedEntity": "Run", "stateMachine": {
                "states": [{ "name": "idle", "isInitial": true }],
                "transitions": [{ "from": "idle", "to": "idle", "event": "INIT", "effects": [] }]
            } }],
            "pages": [{ "name": "Start", "path": "", "protocol": "boot", "traits": [{ "ref": "Work" }] }]
        }]
    }))
    .unwrap()
}

fn served_shot(schema: OrbitalSchema, server: Option<&str>, out: &Path) -> Result<String, String> {
    let shot = DesktopRequest::Screenshot { renderer: "tui".into(), item: "s".into(), schema: Box::new(schema), out_dir: out.to_path_buf(), route: Some("/".into()), server: server.map(str::to_string), program_path: None };
    request(Path::new(BINARY), shot).map(|reply| std::fs::read_to_string(&files(reply)[0]).unwrap()).map_err(|e| e.message)
}

#[test]
fn a_route_capture_of_a_boot_only_program_against_a_server_paints_the_empty_client_page() {
    let out = std::env::temp_dir().join(format!("orb-desktop-boot-{}", std::process::id()));
    served_shot(boot_only_program(), Some("http://127.0.0.1:9"), &out).unwrap();
    std::fs::remove_dir_all(&out).unwrap();
}

#[test]
fn control_a_program_with_a_route_page_still_paints_it_against_a_server() {
    let out = std::env::temp_dir().join(format!("orb-desktop-boot-control-{}", std::process::id()));
    let painted = served_shot(program(), Some("http://127.0.0.1:9"), &out).unwrap();
    assert!(painted.contains("Hello frames"), "{painted}");
    std::fs::remove_dir_all(&out).unwrap();
}
