//! Running a booted program to its end: the kernel's boot mounts dispatched, held while it has an open keyed call, stopped
//! cleanly on SIGINT/SIGTERM. `orb run` and the generated Rust app share this body.

use super::{Host, HostError};
pub use orbital_core::protocols::ProtocolSide;

/// The entity a run mounts its traits on, as an event request naming none addresses: the kernel's singleton frame, the
/// one a `[runtime]` entity's tick runs on when it has no rows (EXEC-TICK-5), never a name the host picks.
pub const RUN_ENTITY: &str = orbital_core::RUNTIME_SINGLETON;

/// Dispatch the mounts the kernel lists for a host start on `side` (EXEC-ENDPOINT-3); the host chooses none.
pub fn mount_init(host: &Host, side: ProtocolSide) -> Result<(), HostError> {
    let seeds = host.boot_mounts(side);
    if !seeds.is_empty() {
        host.dispatch_mount(None, &seeds, RUN_ENTITY, orbital_core::Value::Null)?;
    }
    Ok(())
}

/// How a run ended.
#[derive(Debug, PartialEq, Eq)]
pub enum RunEnd {
    /// No keyed call was left open.
    Idle,
    /// A stop request arrived (SIGINT/SIGTERM); every open call was cancelled.
    Stopped,
}

/// Keep `host` running while it has an open keyed call (a listener, a watch, a pending answer) or a pending
/// `[background]` tick, running the due `[background]` ticks every check (EXEC-TICK-6). A message on (or the
/// hang-up of) `stop` cancels each open call through the `cancel-call` path and ends the run.
pub fn hold(host: &Host, stop: &std::sync::mpsc::Receiver<()>) -> RunEnd {
    const CHECK: std::time::Duration = std::time::Duration::from_millis(50);
    loop {
        host.tick_pass(host.monotonic_ms(), orbital_core::Calendar::local_now());
        if host.open_calls().is_empty() && host.kernel().pending_background_ticks().is_empty() {
            return RunEnd::Idle;
        }
        match stop.recv_timeout(CHECK) {
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                host.cancel_open_calls();
                return RunEnd::Stopped;
            }
        }
    }
}

/// A channel that receives one message when the process gets SIGINT or SIGTERM.
#[cfg(unix)]
pub fn stop_on_signal() -> anyhow::Result<std::sync::mpsc::Receiver<()>> {
    use tokio::signal::unix::{signal, SignalKind};
    let (sender, receiver) = std::sync::mpsc::channel();
    let signals = tokio::runtime::Builder::new_current_thread().enable_io().build()?;
    let (mut interrupt, mut terminate) = signals.block_on(async {
        Ok::<_, std::io::Error>((signal(SignalKind::interrupt())?, signal(SignalKind::terminate())?))
    })?;
    std::thread::Builder::new().name("orb-run-signals".to_string()).spawn(move || {
        signals.block_on(async {
            tokio::select! {
                _ = interrupt.recv() => {}
                _ = terminate.recv() => {}
            }
        });
        let _ = sender.send(());
    })?;
    Ok(receiver)
}

#[cfg(not(unix))]
pub fn stop_on_signal() -> anyhow::Result<std::sync::mpsc::Receiver<()>> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let signals = tokio::runtime::Builder::new_current_thread().enable_io().build()?;
    std::thread::Builder::new().name("orb-run-signals".to_string()).spawn(move || {
        let _ = signals.block_on(tokio::signal::ctrl_c());
        let _ = sender.send(());
    })?;
    Ok(receiver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::store::DriverAdapter;
    use std::sync::Arc;

    fn host() -> Host {
        let traits: Vec<serde_json::Value> = ["ViewT", "BootT", "HttpT", "Orphan"]
            .iter()
            .map(|name| {
                serde_json::json!({
                    "name": name,
                    "linkedEntity": "Note",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }, { "name": "ready" }],
                        "events": [{ "key": "INIT", "name": "INIT" }],
                        "transitions": [{ "from": "idle", "to": "ready", "event": "INIT", "effects": [] }]
                    }
                })
            })
            .collect();
        let program: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Endpoints",
            "version": "1.0.0",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Main",
                "entity": { "name": "Note", "collection": "notes", "fields": [{ "name": "id", "type": "string", "primaryKey": true }] },
                "traits": traits,
                "pages": [
                    { "name": "Home", "path": "/", "traits": [{ "ref": "ViewT" }] },
                    { "name": "Bg", "path": "", "protocol": "boot", "traits": [{ "ref": "BootT" }] },
                    { "name": "Api", "path": "", "protocol": "http", "traits": [{ "ref": "HttpT" }] }
                ]
            }]
        }))
        .unwrap();
        let host = Host::new(Arc::new(DriverAdapter::memory()));
        host.register_schema(program).unwrap();
        host
    }

    fn state(host: &Host, name: &str) -> Option<String> {
        host.get_state(name, RUN_ENTITY).map(|s| s.current_state)
    }

    #[test]
    fn the_server_side_mounts_exactly_the_kernels_boot_mounts_and_never_a_route_trait() {
        let host = host();
        mount_init(&host, ProtocolSide::Server).unwrap();
        assert_eq!(state(&host, "BootT").as_deref(), Some("ready"));
        assert_eq!(state(&host, "HttpT").as_deref(), Some("ready"));
        assert_ne!(state(&host, "ViewT").as_deref(), Some("ready"));
        assert_ne!(state(&host, "Orphan").as_deref(), Some("ready"));
    }

    #[test]
    fn control_the_process_side_mounts_boot_but_not_the_http_trait() {
        let host = host();
        mount_init(&host, ProtocolSide::Process).unwrap();
        assert_eq!(state(&host, "BootT").as_deref(), Some("ready"));
        assert_ne!(state(&host, "HttpT").as_deref(), Some("ready"));
    }

    /// A boot trait on a `[runtime]` entity: `INIT` stamps `due` and waits; a `[background]` tick in `waiting` sees it and
    /// emits `DUE`, whose own arm clears it and moves on (G-CROSS-538/539).
    fn ticking_host() -> Host {
        ticking_host_with(true)
    }

    fn ticking_host_with(background: bool) -> Host {
        let program: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Ticking", "version": "1.0.0", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Main",
                "entity": { "name": "Flow", "persistence": "runtime", "fields": [
                    { "name": "id", "type": "string" }, { "name": "due", "type": "boolean", "nullable": true }
                ] },
                "traits": [{
                    "name": "Starter", "linkedEntity": "Flow",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }, { "name": "waiting" }, { "name": "started" }],
                        "events": [{ "key": "INIT", "name": "INIT" }, { "key": "DUE", "name": "DUE" }],
                        "transitions": [
                            { "from": "idle", "to": "waiting", "event": "INIT", "effects": [["set", "@entity.due", true]] },
                            { "from": "waiting", "to": "started", "event": "DUE", "effects": [["set", "@entity.due", null]] }
                        ]
                    },
                    "ticks": [{ "name": "beat", "interval": 10, "runsInBackground": background, "appliesTo": ["waiting"],
                        "guard": ["=", "@entity.due", true], "effects": [["emit", "DUE", {}]] }],
                    "emits": [{ "event": "DUE" }]
                }],
                "pages": [{ "name": "Bg", "path": "", "protocol": "boot", "traits": [{ "ref": "Starter" }] }]
            }]
        }))
        .unwrap();
        let host = Host::new(Arc::new(DriverAdapter::memory()));
        host.register_schema(program).unwrap();
        host
    }

    #[test]
    fn a_boot_mount_and_a_background_tick_share_the_runtime_entity_s_frame() {
        let host = ticking_host();
        mount_init(&host, ProtocolSide::Server).unwrap();
        assert_eq!(state(&host, "Starter").as_deref(), Some("waiting"));
        let start = host.monotonic_ms();
        for step in 0..5 {
            host.tick_pass(start + step * 20, orbital_core::Calendar::local_now());
        }
        assert_eq!(state(&host, "Starter").as_deref(), Some("started"), "the tick read the frame INIT wrote");
    }

    #[test]
    fn a_boot_trait_s_mount_scoped_tick_runs_on_the_host_clock_with_no_client() {
        let host = ticking_host_with(false);
        mount_init(&host, ProtocolSide::Server).unwrap();
        let start = host.monotonic_ms();
        for step in 0..5 {
            host.tick_pass(start + step * 20, orbital_core::Calendar::local_now());
        }
        assert_eq!(state(&host, "Starter").as_deref(), Some("started"), "a boot trait is endpoint-mounted (EXEC-ENDPOINT-3)");
    }

    #[test]
    fn hold_runs_due_background_ticks_until_none_is_pending() {
        let host = ticking_host();
        mount_init(&host, ProtocolSide::Server).unwrap();
        let (_keep, stop) = std::sync::mpsc::channel::<()>();
        assert_eq!(hold(&host, &stop), RunEnd::Idle);
        assert_eq!(state(&host, "Starter").as_deref(), Some("started"));
    }

    #[test]
    fn control_hold_with_no_pending_tick_and_no_open_call_ends_at_once() {
        let host = host();
        mount_init(&host, ProtocolSide::Server).unwrap();
        let (_keep, stop) = std::sync::mpsc::channel::<()>();
        assert_eq!(hold(&host, &stop), RunEnd::Idle);
    }

    /// Two boot traits: `Starter`'s `INIT` emits `GO`, which `Worker` hears; `Worker`'s own `INIT` arm runs only from
    /// `idle`. `order` is the boot page's trait order (G-CROSS-540).
    fn boot_order_host(order: [&str; 2]) -> Host {
        let worker_init = serde_json::json!({ "from": "idle", "to": "idle", "event": "INIT", "effects": [["set", "@entity.inited", true]] });
        let program: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "BootOrder", "version": "1.0.0", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Main",
                "entity": { "name": "Box", "persistence": "runtime", "fields": [
                    { "name": "id", "type": "string" }, { "name": "inited", "type": "boolean", "default": false }
                ] },
                "traits": [{
                    "name": "Starter", "linkedEntity": "Box",
                    "stateMachine": {
                        "states": [{ "name": "ready", "isInitial": true }],
                        "events": [{ "key": "INIT", "name": "INIT" }],
                        "transitions": [{ "from": "ready", "to": "ready", "event": "INIT", "effects": [["emit", "GO", {}]] }]
                    },
                    "emits": [{ "event": "GO" }]
                }, {
                    "name": "Worker", "linkedEntity": "Box",
                    "listens": [{ "event": "GO", "triggers": "GO", "source": { "kind": "trait", "trait": "Starter" } }],
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }, { "name": "busy" }],
                        "events": [{ "key": "INIT", "name": "INIT" }, { "key": "GO", "name": "GO" }],
                        "transitions": [worker_init, { "from": "idle", "to": "busy", "event": "GO" }]
                    }
                }],
                "pages": [{ "name": "Bg", "path": "", "protocol": "boot", "traits": [{ "ref": order[0] }, { "ref": order[1] }] }]
            }]
        }))
        .unwrap();
        let host = Host::new(Arc::new(DriverAdapter::memory()));
        host.register_schema(program).unwrap();
        host
    }

    fn inited(host: &Host) -> serde_json::Value {
        serde_json::to_value(&host.kernel().snapshot_trait("Worker", Some(RUN_ENTITY)).entity).unwrap()["inited"].clone()
    }

    #[test]
    fn every_boot_init_runs_before_an_event_an_earlier_init_emitted_is_delivered() {
        let host = boot_order_host(["Starter", "Worker"]);
        mount_init(&host, ProtocolSide::Server).unwrap();
        assert_eq!(state(&host, "Worker").as_deref(), Some("busy"));
        assert_eq!(inited(&host), serde_json::json!(true), "Worker's INIT ran before it heard GO");
    }

    #[test]
    fn control_a_listener_booted_first_inits_then_hears_the_event() {
        let host = boot_order_host(["Worker", "Starter"]);
        mount_init(&host, ProtocolSide::Server).unwrap();
        assert_eq!(state(&host, "Worker").as_deref(), Some("busy"));
        assert_eq!(inited(&host), serde_json::json!(true));
    }

    /// A route trait `Asker` emits `GO`; `Worker`, on a boot-only orbital, hears it only when its boot `INIT` has run on
    /// the instance the step reaches (G-CROSS: endpoint traits are program-wide).
    fn shared_worker_host() -> Host {
        let program: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Shared", "version": "1.0.0", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Ui",
                "entity": { "name": "Form", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{
                    "name": "Asker", "linkedEntity": "Form",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "INIT", "name": "INIT" }, { "key": "ASK", "name": "ASK" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "INIT" },
                            { "from": "idle", "to": "idle", "event": "ASK", "effects": [["emit", "GO", {}]] }
                        ]
                    },
                    "emits": [{ "event": "GO" }]
                }],
                "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Asker" }] }]
            }, {
                "name": "Jobs",
                "entity": { "name": "Job", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "booted", "type": "boolean", "default": false }] },
                "traits": [{
                    "name": "Worker", "linkedEntity": "Job",
                    "listens": [{ "event": "GO", "triggers": "GO", "source": { "kind": "orbital", "orbital": "Ui", "trait": "Asker" } }],
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }, { "name": "ready" }, { "name": "worked" }],
                        "events": [{ "key": "INIT", "name": "INIT" }, { "key": "GO", "name": "GO" }],
                        "transitions": [
                            { "from": "idle", "to": "ready", "event": "INIT", "effects": [["set", "@entity.booted", true]] },
                            { "from": "ready", "to": "worked", "event": "GO", "guard": ["=", "@entity.booted", true] }
                        ]
                    }
                }],
                "pages": [{ "name": "Bg", "path": "", "protocol": "boot", "traits": [{ "ref": "Worker" }] }]
            }, {
                "name": "Api",
                "entity": { "name": "Call", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{
                    "name": "Binder", "linkedEntity": "Call",
                    "listens": [{ "event": "GO", "triggers": "GO", "source": { "kind": "orbital", "orbital": "Ui", "trait": "Asker" } }],
                    "stateMachine": {
                        "states": [{ "name": "open", "isInitial": true }, { "name": "bound" }],
                        "events": [{ "key": "GO", "name": "GO" }],
                        "transitions": [{ "from": "open", "to": "bound", "event": "GO" }]
                    }
                }],
                "pages": [{ "name": "Http", "path": "", "protocol": "http", "traits": [{ "ref": "Binder" }] }]
            }]
        }))
        .unwrap();
        let host = Host::new(Arc::new(DriverAdapter::memory()));
        host.register_schema(program).unwrap();
        host
    }

    #[test]
    fn a_viewer_s_step_reaches_the_boot_trait_s_one_program_wide_instance() {
        let host = shared_worker_host();
        mount_init(&host, ProtocolSide::Server).unwrap();
        {
            let _tab = host.kernel().enter_session("tab-1");
            host.dispatch("Asker", RUN_ENTITY, "ASK", orbital_core::Value::Null).unwrap();
        }
        assert_eq!(state(&host, "Worker").as_deref(), Some("worked"), "the boot instance heard GO");
    }

    #[test]
    fn control_an_http_endpoint_trait_keeps_one_instance_per_viewer() {
        let host = shared_worker_host();
        mount_init(&host, ProtocolSide::Server).unwrap();
        {
            let _tab = host.kernel().enter_session("tab-1");
            host.dispatch("Asker", RUN_ENTITY, "ASK", orbital_core::Value::Null).unwrap();
            assert_eq!(state(&host, "Binder").as_deref(), Some("bound"), "the viewer's own instance stepped");
        }
        assert_ne!(state(&host, "Binder").as_deref(), Some("bound"), "the program session's instance did not step");
    }

    #[test]
    fn control_a_route_trait_keeps_one_instance_per_viewer() {
        let host = shared_worker_host();
        mount_init(&host, ProtocolSide::Server).unwrap();
        {
            let _tab = host.kernel().enter_session("tab-1");
            host.dispatch("Asker", RUN_ENTITY, "INIT", orbital_core::Value::Null).unwrap();
        }
        assert_eq!(state(&host, "Worker").as_deref(), Some("ready"), "an INIT of the route trait reaches no boot trait");
    }

    #[test]
    fn edge_a_program_with_no_endpoint_pages_mounts_nothing() {
        let host = Host::new(Arc::new(DriverAdapter::memory()));
        mount_init(&host, ProtocolSide::Server).unwrap();
        assert!(host.boot_mounts(ProtocolSide::Server).is_empty());
    }
}
