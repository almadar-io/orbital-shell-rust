//! A `$rowSet` marker in a slot's props is resolved at paint through the kernel's one resolver
//! (`ClientCore::resolve_rows`), and a rows-changed repaint re-resolves it. The painted contents are what the
//! root `App` component hands to the pattern renderers.

use orbital_client_dioxus::app::{painted_slot_contents, APP};
use orbital_client_dioxus::shell::{mount_circuit, with_core};
use orbital_core::client::bridge::{KernelBridge, ServerBridge};
use orbital_core::client::client_core::ClientCore;
use orbital_core::client::client_kernel::ClientKernel;
use orbital_core::runtime::row_query::{row_set_marker, RowSetRef};
use orbital_core::{OrbitalSchema, Value};
use serde_json::json;
use std::sync::Arc;

fn schema() -> OrbitalSchema {
    serde_json::from_value(json!({
        "name": "rows", "orbitals": [{
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
                        { "from": "idle", "to": "idle", "event": "LISTED", "effects": [["render-ui", "main", { "type": "data-list", "items": "@payload.data" }]] },
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

fn row(id: &str, title: &str, owner: &str) -> Value {
    serde_json::from_value(json!({ "id": id, "title": title, "owner": owner })).unwrap()
}

fn mount() -> Arc<ClientKernel> {
    let schema = schema();
    let server = KernelBridge::new(schema.clone()).unwrap();
    server.seed("Ticket", vec![row("t1", "alpha-row", "u1")]);
    let bridge: Arc<dyn ServerBridge> = Arc::new(server);
    let kernel = Arc::new(ClientKernel::new(schema.clone(), Some(bridge)).unwrap());
    kernel.set_current_user(serde_json::from_value(json!({ "id": "u1" })).unwrap());
    kernel.seed(
        "Ticket",
        vec![
            row("t1", "alpha-row", "u1"),
            row("t2", "hidden-row", "u2"),
            row("t3", "gamma-row", "u1"),
        ],
    );
    mount_circuit(ClientCore::with_client_kernel(schema, kernel.clone()));
    kernel
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

fn painted_items() -> Vec<String> {
    APP.with(|cell| {
        let borrow = cell.borrow();
        let core = &borrow.as_ref().unwrap().core;
        let contents = painted_slot_contents(core, "main");
        assert_eq!(contents.len(), 1);
        let items = contents[0]
            .props
            .as_object()
            .unwrap()
            .get("items")
            .cloned()
            .unwrap_or(Value::Null);
        match items {
            Value::Array(rows) => rows
                .iter()
                .map(|r| {
                    r.as_object()
                        .unwrap()
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap()
                        .to_string()
                })
                .collect(),
            other => panic!("items is not resolved to rows: {other:?}"),
        }
    })
}

// One test: the mounted app is a thread-local cell, so the cases run in sequence on one thread.
#[test]
fn rows_by_reference_paint_resolved_hide_unreadable_rows_and_re_resolve_on_a_rows_changed_repaint()
{
    let kernel = mount();
    list_all();
    assert_eq!(
        painted_items(),
        vec!["alpha-row", "gamma-row"],
        "a marker paints the resolved rows; u2's row is omitted"
    );

    // the server runs the persist; the row it wrote reaches the client's rows before the repaint paints
    kernel.seed("Ticket", vec![row("t1", "renamed", "u1")]);
    drive("RENAME", Value::Null);
    assert_eq!(
        painted_items(),
        vec!["renamed", "gamma-row"],
        "the rows-changed repaint re-resolves the unchanged reference"
    );

    mount();
    drive("OTHER", Value::Null);
    APP.with(|cell| {
        let borrow = cell.borrow();
        let core = &borrow.as_ref().unwrap().core;
        let contents = painted_slot_contents(core, "main");
        assert_eq!(
            contents[0].props,
            core.slot_contents("main")[0].props,
            "control: no marker, props unchanged"
        );
    });
}
