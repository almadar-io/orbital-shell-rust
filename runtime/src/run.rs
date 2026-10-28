//! Running a booted program to its end: the kernel's boot mounts dispatched, held while it has an open keyed call, stopped
//! cleanly on SIGINT/SIGTERM. `orb run` and the generated Rust app share this body.

use super::{Host, HostError};
pub use orbital_core::protocols::ProtocolSide;

/// The entity a run mounts its traits on, as an event request naming none addresses (`"default"`).
pub const RUN_ENTITY: &str = "default";

/// Dispatch the mounts the kernel lists for a host start on `side` (EXEC-ENDPOINT-3); the host chooses none.
pub fn mount_init(host: &Host, side: ProtocolSide) -> Result<(), HostError> {
    let seeds = host.boot_mounts(side);
    if !seeds.is_empty() {
        host.dispatch_mount(&seeds, RUN_ENTITY, orbital_core::Value::Null)?;
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

/// Keep `host` running while it has an open keyed call (a listener, a watch, a pending answer). A message on (or the
/// hang-up of) `stop` cancels each open call through the `cancel-call` path and ends the run.
pub fn hold(host: &Host, stop: &std::sync::mpsc::Receiver<()>) -> RunEnd {
    const CHECK: std::time::Duration = std::time::Duration::from_millis(50);
    loop {
        if host.open_calls().is_empty() {
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

    #[test]
    fn edge_a_program_with_no_endpoint_pages_mounts_nothing() {
        let host = Host::new(Arc::new(DriverAdapter::memory()));
        mount_init(&host, ProtocolSide::Server).unwrap();
        assert!(host.boot_mounts(ProtocolSide::Server).is_empty());
    }
}
