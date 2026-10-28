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
    let schema: OrbitalSchema = serde_json::from_value(serde_json::json!({ "name": "t", "typeContractVersion": 1, "orbitals": orbitals })).unwrap();
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

mod rows_by_reference {
    use super::*;
    use orbital_core::client::bridge::KernelBridge;
    use orbital_core::client::client_core::ClientCore;
    use orbital_core::client::client_kernel::ClientKernel;
    use orbital_core::runtime::row_query::{row_set_marker, RowSetRef};
    use serde_json::json;

    fn schema() -> OrbitalSchema {
        serde_json::from_value(json!({
            "name": "rows", "typeContractVersion": 1, "orbitals": [{
                "name": "Main",
                "entity": {
                    "name": "Ticket", "persistence": "runtime",
                    "fields": [{ "name": "id", "type": "string" }, { "name": "title", "type": "string" }, { "name": "owner", "type": "string" }],
                    "read_policy": ["=", ["object/get", "@entity", "owner"], "@user.id"]
                },
                "traits": [{
                    "name": "Tickets", "linkedEntity": "Ticket",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": "no list yet" }]] },
                            { "from": "idle", "to": "idle", "event": "LISTED", "effects": [["render-ui", "main", { "type": "table-view", "items": "@payload.data" }]] },
                            { "from": "idle", "to": "idle", "event": "RENAME", "effects": [["persist", "update", "Ticket", { "id": "t1", "title": "renamed" }]] },
                            { "from": "idle", "to": "idle", "event": "OTHER", "effects": [["render-ui", "main", { "type": "typography", "content": "plain" }]] }
                        ]
                    }
                }],
                "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Tickets" }] }]
            }]
        }))
        .unwrap()
    }

    fn row(id: &str, title: &str, owner: &str) -> orbital_core::Value {
        serde_json::from_value(json!({ "id": id, "title": title, "owner": owner })).unwrap()
    }

    fn app() -> (TuiApp, Arc<ClientKernel>) {
        let schema = schema();
        let server = KernelBridge::new(schema.clone()).unwrap();
        server.seed("Ticket", vec![row("t1", "alpha-row", "u1")]).unwrap();
        let bridge: Arc<dyn ServerBridge> = Arc::new(server);
        let kernel = Arc::new(ClientKernel::new(schema.clone(), Some(bridge)).unwrap());
        kernel.set_current_user(serde_json::from_value(json!({ "id": "u1" })).unwrap());
        kernel.seed("Ticket", vec![row("t1", "alpha-row", "u1"), row("t2", "hidden-row", "u2"), row("t3", "gamma-row", "u1")]).unwrap();
        let app = TuiApp::with_core(ClientCore::with_client_kernel(schema, kernel.clone()), Vec::new());
        (app, kernel)
    }

    fn list_all(app: &mut TuiApp) {
        let reference = RowSetRef { entity_type: "Ticket".into(), ids: vec!["t1".into(), "t2".into(), "t3".into()], total: 3, query: Default::default() };
        let data = serde_json::to_value(row_set_marker(&reference)).unwrap();
        app.send_event("LISTED", serde_json::from_value(json!({ "data": data, "totalCount": 3 })).unwrap());
    }

    #[test]
    fn a_marker_paints_the_rows_the_kernel_resolves_and_a_hidden_row_is_omitted() {
        let (mut app, _) = app();
        list_all(&mut app);
        let text = app.render_to_text(80, 12).unwrap();
        assert!(text.contains("alpha-row") && text.contains("gamma-row"), "{text}");
        assert!(!text.contains("hidden-row"), "u2's row is not the viewer's to read: {text}");
        assert!(!text.contains("$rowSet"), "{text}");
    }

    #[test]
    fn a_rows_changed_repaint_re_resolves_the_reference() {
        let (mut app, kernel) = app();
        list_all(&mut app);
        assert!(app.render_to_text(80, 12).unwrap().contains("alpha-row"));
        // the server runs the persist; the row it wrote reaches the client's rows before the repaint paints
        kernel.seed("Ticket", vec![row("t1", "renamed", "u1")]).unwrap();
        app.send_event("RENAME", orbital_core::Value::Null);
        let text = app.render_to_text(80, 12).unwrap();
        assert!(text.contains("renamed") && !text.contains("alpha-row"), "{text}");
        assert!(text.contains("gamma-row"), "{text}");
    }

    #[test]
    fn control_a_slot_without_a_marker_paints_as_before() {
        let (mut app, _) = app();
        app.send_event("OTHER", orbital_core::Value::Null);
        assert!(app.render_to_text(80, 12).unwrap().contains("plain"));
    }
}
