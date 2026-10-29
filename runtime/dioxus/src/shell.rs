//! Backend-neutral mount/drive/snapshot core shared by the web (wasm) and
//! native (Blitz) shells. ONE schema pipeline, ONE event circuit
//! (`KernelBridge` + `ClientCore`), ONE pattern registry — the shells only
//! differ in how the Dioxus vdom is painted (DOM vs wgpu) and how user
//! events arrive (JS bridge vs winit). Extracted from `web.rs` so neither
//! shell carries a copy.

use std::sync::Arc;

use orbital_core::client::bridge::{KernelBridge, ServerBridge};
use orbital_core::client::ClientKernel;
use orbital_core::{EntityRef, OrbitalSchema, Value};

use crate::app::{AppState, APP};
use crate::client_core::ClientCore;

/// Log namespace — mirrors `@almadar/logger`'s `almadar:ui-wasm`
/// (Rust `tracing` targets use `::`).
pub const NS: &str = "almadar::ui_wasm_dioxus";

/// Seed one `singleton` row per inline entity with no authored `instances`,
/// from each field's literal `default` — identical to the egui web shell so
/// `@entity.X` bindings resolve for instance-scoped atoms (e.g. the counter).
fn seed_singletons(
    seed: &dyn Fn(&str, Vec<Value>) -> Result<(), String>,
    schema: &OrbitalSchema,
) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for orbital in &schema.orbitals {
        let mut refs: Vec<&EntityRef> = vec![&orbital.entity];
        if let Some(aux) = &orbital.auxiliary_entities {
            refs.extend(aux.iter());
        }
        for er in refs {
            let EntityRef::Inline(def) = er else { continue };
            if def.instances.as_ref().is_some_and(|v| !v.is_empty()) {
                continue;
            }
            if !seen.insert(def.name.clone()) {
                continue;
            }
            let mut row = orbital_core::ValueMap::new();
            for f in &def.fields {
                let Some(name) = f.name.as_ref() else {
                    continue;
                };
                if name == "id" {
                    continue;
                }
                let v = f
                    .default
                    .as_ref()
                    .map(|d| Value::from(d.clone()))
                    .unwrap_or(Value::Null);
                row.insert(name.clone().into(), v);
            }
            let defaulted = row.len();
            seed(&def.name, vec![Value::Object(row.into())])?;
            tracing::debug!(target: NS, entity = %def.name, defaulted, "seeded singleton entity");
        }
    }
    Ok(())
}

/// Register a program `orb` resolved (`orb resolve`), build the circuit and
/// registry, and store them in the shared [`APP`] cell. Returns
/// `(orbital_count, trait_count)` for the caller's log line. Backend-neutral:
/// no DOM, no window.
pub fn mount_core(schema_json: &str) -> Result<(usize, usize), String> {
    let schema = orbital_core::schema::parse_schema(schema_json).map_err(|e| e.to_string())?;
    let server = KernelBridge::new(schema.clone()).map_err(|e| e.to_string())?;
    seed_singletons(&|entity, rows| server.seed(entity, rows).map_err(|e| e.to_string()), &schema)?;
    let kernel = ClientKernel::new(schema.clone(), Some(Arc::new(server) as Arc<dyn ServerBridge>)).map_err(|e| e.to_string())?;
    seed_singletons(&|entity, rows| kernel.seed(entity, rows), &schema)?;
    Ok(mount_circuit(ClientCore::with_client_kernel(schema, Arc::new(kernel))))
}

/// [`mount_core`] over a caller's bridge (a live server, or a seeded kernel): the circuit is a client-role kernel
/// whose server legs go to `bridge`, so its ticks run through the kernel's frame loop.
pub fn mount(schema: OrbitalSchema, bridge: Arc<dyn ServerBridge>) -> Result<(usize, usize), String> {
    let kernel = ClientKernel::new(schema.clone(), Some(bridge)).map_err(|e| e.to_string())?;
    Ok(mount_circuit(ClientCore::with_client_kernel(schema, Arc::new(kernel))))
}

/// Mount a circuit the caller already wired. A core built with a client-role kernel
/// ([`ClientCore::with_client_kernel`]) is the one that resolves row-set markers at paint.
pub fn mount_circuit(mut core: ClientCore) -> (usize, usize) {
    let trait_count: usize = core.schema.orbitals.iter().map(|o| o.traits.len()).sum();
    let orbital_count = core.schema.orbitals.len();

    core.mount_page_blocking();
    core.settle(8);
    tracing::info!(
        target: NS,
        state = %core.current_state,
        ui_slots = ?core.ui_slots.keys().collect::<Vec<_>>(),
        trait_content = ?core.trait_content.keys().collect::<Vec<_>>(),
        "init done"
    );

    let registry = crate::registry::default_registry();

    APP.with(|cell| {
        *cell.borrow_mut() = Some(AppState {
            core,
            registry,
        });
    });
    (orbital_count, trait_count)
}

/// Run `f` against the mounted circuit.
pub fn with_core<R>(f: impl FnOnce(&mut ClientCore) -> R) -> Result<R, String> {
    APP.with(|cell| cell.borrow_mut().as_mut().map(|state| f(&mut state.core)))
        .ok_or_else(|| "the app is not mounted".to_string())
}

/// Drive one event synchronously through the circuit. Empty / `null` payload
/// is allowed. The caller pokes the paint layer to re-render afterwards
/// (`poke_rerender` on web; Blitz invalidates through the same signal).
pub fn drive_event(event: &str, payload_json: &str) -> Result<(), String> {
    let payload: Value = if payload_json.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(payload_json).map_err(|e| e.to_string())?
    };
    let mut drove = false;
    APP.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            state.core.drive_blocking(event, None, payload);
            state.core.settle(8);
            drove = true;
        }
    });
    if !drove {
        return Err(format!(
            "send_event('{event}') before start — app not mounted"
        ));
    }
    tracing::info!(target: NS, event, "event dispatched");
    Ok(())
}

/// Snapshot current render state as JSON (`current_state`, `slots`,
/// `trait_content`) — the same shape the TS bridge consumes on web.
pub fn snapshot_json() -> Result<String, String> {
    let mut out: Option<String> = None;
    APP.with(|cell| {
        if let Some(state) = cell.borrow().as_ref() {
            let core = &state.core;
            let slots: Vec<&String> = core.ui_slots.keys().collect();
            let snap = serde_json::json!({
                "current_state": core.current_state,
                "slots": slots,
                "trait_content": core.trait_content,
            });
            out = serde_json::to_string(&snap).ok();
        }
    });
    out.ok_or_else(|| "app not mounted".to_string())
}

/// Current active state-machine state.
pub fn current_state_string() -> Result<String, String> {
    let mut state: Option<String> = None;
    APP.with(|cell| {
        if let Some(s) = cell.borrow().as_ref() {
            state = Some(s.core.current_state.clone());
        }
    });
    state.ok_or_else(|| "app not mounted".to_string())
}

/// Unmount: drop the app state (the vdom renders the empty branch).
pub fn unmount() {
    APP.with(|cell| *cell.borrow_mut() = None);
}

/// Fire every due scheduled tick through the circuit — the native frame-hook
/// driver (vendored `dioxus-native` patch invokes this on `RedrawRequested`,
/// the vsync cadence). Returns true when any tick fired, so the caller pokes a
/// re-render only when state actually advanced.
pub fn drive_ticks() -> bool {
    let mut fired = false;
    APP.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            if state.core.drive_ticks_blocking() {
                state.core.settle(8);
                fired = true;
            }
        }
    });
    fired
}

/// `orb resolve <path>`: a `.lolo` or `.orb` program as the resolved `.orb`
/// JSON [`mount_core`] takes. `ORB_BIN` names the binary, else `orb` on PATH.
#[cfg(not(target_arch = "wasm32"))]
pub fn orb_resolve(path: &std::path::Path) -> Result<String, String> {
    let orb = std::env::var("ORB_BIN").unwrap_or_else(|_| "orb".to_string());
    let out = std::process::Command::new(&orb)
        .arg("resolve")
        .arg(path)
        .output()
        .map_err(|e| format!("{orb} resolve {}: {e}", path.display()))?;
    if !out.status.success() {
        return Err(format!("{orb} resolve {} exited {}: {}", path.display(), out.status, String::from_utf8_lossy(&out.stderr)));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("{orb} resolve printed non-UTF-8: {e}"))
}
