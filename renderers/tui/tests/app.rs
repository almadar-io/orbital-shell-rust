//! The terminal renderer draws the shared client circuit (`ClientCore`).

use orbital_client_tui::{PatternRegistry, TuiApp};
use orbital_core::client::bridge::{KernelBridge, ServerBridge};
use orbital_core::OrbitalSchema;
use std::sync::Arc;

fn orbital(name: &str, text: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "entity": { "name": format!("{name}Row"), "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
        "traits": [{
            "name": format!("{name}View"), "linkedEntity": format!("{name}Row"),
            "stateMachine": {
                "states": [{ "name": "idle", "isInitial": true }],
                "transitions": [{ "from": "idle", "to": "idle", "event": "INIT",
                    "effects": [["render-ui", "main", text]] }]
            }
        }],
        "pages": [{ "name": format!("{name}Home"), "path": format!("/{}", name.to_lowercase()), "traits": [{ "ref": format!("{name}View") }] }]
    })
}

fn app(orbitals: Vec<serde_json::Value>) -> TuiApp {
    let schema: OrbitalSchema = serde_json::from_value(serde_json::json!({ "name": "t", "orbitals": orbitals })).unwrap();
    let bridge: Arc<dyn ServerBridge> = Arc::new(KernelBridge::new(schema.clone()).unwrap());
    TuiApp::new(schema, Some(bridge), Vec::new()).unwrap()
}

#[test]
fn the_first_page_mounts_through_the_kernel_and_draws_its_main_slot() {
    let mut app = app(vec![orbital("Notes", &serde_json::json!({ "type": "typography", "content": "hello notes" }))]);
    assert!(app.core().slot_contents("main").iter().any(|c| c.pattern == "typography"));
    assert!(app.render_to_text(100, 20).unwrap().contains("hello notes"));
}

#[test]
fn control_a_pattern_without_a_terminal_renderer_draws_the_declared_unsupported_answer() {
    let mut app = app(vec![orbital("Notes", &serde_json::json!({ "type": "zz-no-such-pattern" }))]);
    assert!(!app.registry().supports("zz-no-such-pattern"));
    let text = app.render_to_text(100, 20).unwrap();
    assert!(!text.contains("No content to display"), "{text}");
}

#[test]
fn the_registry_covers_the_core_patterns() {
    let registry = PatternRegistry::new();
    assert!(registry.coverage().len() > 200, "{}", registry.coverage().len());
    assert!(registry.supports("typography") && registry.supports("button"));
}

#[test]
fn edge_an_empty_main_slot_says_so() {
    let mut app = app(vec![orbital("Notes", &serde_json::json!(null))]);
    assert!(app.render_to_text(80, 10).unwrap().contains("No content to display"));
}
