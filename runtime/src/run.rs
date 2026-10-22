//! Running a booted program to its end: `INIT` mounted on every trait, held while it has an open keyed call, stopped
//! cleanly on SIGINT/SIGTERM. `orb run` and the generated Rust app share this body.

use super::{Host, HostError};

/// The entity a run mounts its traits on, as an event request naming none addresses (`"default"`).
pub const RUN_ENTITY: &str = "default";

/// Mount `INIT` on every trait of every registered orbital.
pub fn mount_init(host: &Host) -> Result<(), HostError> {
    let seeds: Vec<orbital_core::effects::MountSeed> = host
        .list_orbitals()
        .iter()
        .filter_map(|orbital| host.get_orbital(&orbital.name))
        .flat_map(|detail| detail.traits)
        .map(|name| orbital_core::effects::MountSeed {
            r#trait: name,
            event: orbital_core::effects::LifecycleEvent::Init,
        })
        .collect();
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
