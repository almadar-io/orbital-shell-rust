//! The renderer mounts a program `orb` already resolved; it never resolves one.

use orbital_client_dioxus::shell::{mount_core, snapshot_json};

fn program(traits: serde_json::Value) -> String {
    serde_json::json!({
        "name": "counter",
        "orbitals": [{
            "name": "Main",
            "entity": { "name": "Counter", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
            "traits": traits,
            "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Show" }] }]
        }]
    })
    .to_string()
}

#[test]
fn a_resolved_program_mounts_and_renders_its_main_slot() {
    let resolved = program(serde_json::json!([{
        "name": "Show", "linkedEntity": "Counter",
        "stateMachine": {
            "states": [{ "name": "idle", "isInitial": true }],
            "transitions": [{ "from": "idle", "to": "idle", "event": "INIT",
                "effects": [["render-ui", "main", { "type": "typography", "content": "hello" }]] }]
        }
    }]));
    let (orbitals, traits) = mount_core(&resolved).expect("mount");
    assert_eq!((orbitals, traits), (1, 1));
    assert!(snapshot_json().unwrap().contains("hello"));
}

#[test]
fn control_malformed_json_is_refused_not_resolved() {
    assert!(mount_core("{ not json").is_err());
}
