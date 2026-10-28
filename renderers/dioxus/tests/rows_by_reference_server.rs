//! `mount` over a server bridge (no client-role kernel): a `$rowSet` marker the server holds is resolved at paint
//! through the bridge's resolve route, with the viewer's `@read` applied there. Twin of `rows_by_reference.rs`, whose
//! rows live in a client-role kernel. Controls: a bridge with no resolve route paints an empty list, never the marker.

use orbital_client_dioxus::app::{painted_slot_contents, APP};
use orbital_client_dioxus::shell::{mount, with_core};
use orbital_core::client::bridge::{KernelBridge, ServerBridge};
use orbital_core::client::http::BridgeError;
use orbital_core::runtime::row_query::{row_set_marker, RowSetRef};
use orbital_core::{EventResponse, OrbitalSchema, Value};
use serde_json::json;
use std::sync::Arc;

fn schema() -> OrbitalSchema {
    serde_json::from_value(json!({
        "name": "rows", "typeContractVersion": 1, "orbitals": [{
            "name": "Main",
            "entity": {
                "name": "Ticket", "persistence": "persistent",
                "fields": [{ "name": "id", "type": "string" }, { "name": "title", "type": "string" }, { "name": "owner", "type": "string" }],
                "read_policy": ["=", ["object/get", "@entity", "owner"], "@user.id"]
            },
            "traits": [{
                "name": "Tickets", "linkedEntity": "Ticket",
                "stateMachine": {
                    "states": [{ "name": "idle", "isInitial": true }],
                    "transitions": [
                        { "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": "no list yet" }]] },
                        { "from": "idle", "to": "idle", "event": "LISTED", "effects": [["render-ui", "main", { "type": "data-list", "items": "@payload.data" }]] },
                        { "from": "idle", "to": "idle", "event": "RENAME", "effects": [["persist", "update", "Ticket", { "id": "t1", "title": "renamed" }]] }
                    ]
                }
            }],
            "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Tickets" }] }]
        }]
    }))
    .unwrap()
}

fn row(id: &str, title: &str, owner: &str) -> Value {
    serde_json::from_value(json!({ "id": id, "title": title, "owner": owner })).unwrap()
}

/// A server holding three tickets, two of them u1's, bound to viewer u1.
fn server() -> KernelBridge {
    let server = KernelBridge::new(schema()).unwrap();
    server.seed(
        "Ticket",
        vec![
            row("t1", "alpha-row", "u1"),
            row("t2", "hidden-row", "u2"),
            row("t3", "gamma-row", "u1"),
        ],
    ).unwrap();
    server.set_current_user(serde_json::from_value(json!({ "id": "u1" })).unwrap());
    server
}

/// Forwards events to a real server and refuses to resolve, as a bridge with no resolve route does.
struct EventsOnly(KernelBridge);

impl ServerBridge for EventsOnly {
    fn send_event_with_options(
        &self,
        orbital: &str,
        event: &str,
        entity_id: Option<&str>,
        trait_name: Option<&str>,
        payload: Value,
    ) -> Result<EventResponse, BridgeError> {
        self.0
            .send_event_with_options(orbital, event, entity_id, trait_name, payload)
    }
}

fn drive(event: &str, payload: Value) {
    with_core(|core| {
        core.drive_blocking(event, None, payload);
        core.settle(8);
    })
    .unwrap();
}

fn list_all() {
    let reference = RowSetRef {
        entity_type: "Ticket".into(),
        ids: vec!["t1".into(), "t2".into(), "t3".into()],
        total: 3,
        query: Default::default(),
    };
    let data = serde_json::to_value(row_set_marker(&reference)).unwrap();
    drive(
        "LISTED",
        serde_json::from_value(json!({ "data": data, "totalCount": 3 })).unwrap(),
    );
}

fn painted_items() -> Option<Vec<String>> {
    APP.with(|cell| {
        let borrow = cell.borrow();
        let core = &borrow.as_ref().unwrap().core;
        let contents = painted_slot_contents(core, "main");
        assert_eq!(contents.len(), 1);
        match contents[0]
            .props
            .as_object()
            .unwrap()
            .get("items")
            .cloned()?
        {
            Value::Array(rows) => Some(
                rows.iter()
                    .map(|r| {
                        r.as_object()
                            .unwrap()
                            .get("title")
                            .and_then(|t| t.as_str())
                            .unwrap()
                            .to_string()
                    })
                    .collect(),
            ),
            other => panic!("items is not resolved to rows: {other:?}"),
        }
    })
}

// One test: the mounted app is a thread-local cell, so the cases run in sequence on one thread.
#[test]
fn server_hosted_rows_paint_resolved_hide_unreadable_rows_and_follow_the_server_after_a_write() {
    mount(schema(), Arc::new(server()));
    list_all();
    assert_eq!(
        painted_items(),
        Some(vec!["alpha-row".to_string(), "gamma-row".to_string()]),
        "the marker paints the rows the server resolves; u2's row is omitted"
    );

    drive("RENAME", Value::Null);
    assert_eq!(
        painted_items(),
        Some(vec!["renamed".to_string(), "gamma-row".to_string()]),
        "a paint after the server's write resolves the unchanged reference afresh"
    );

    mount(schema(), Arc::new(EventsOnly(server())));
    list_all();
    assert_eq!(
        painted_items(),
        Some(Vec::new()),
        "control: a bridge that cannot resolve paints an empty list, never the raw marker"
    );
}
