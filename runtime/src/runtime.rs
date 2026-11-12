//! The native program host: one `orbital_core::RuntimeKernel` with the native ports a program asks for, the host every
//! native consumer boots a program on (`orb run`, `cli-*` programs, `orb mcp --program`, the add-ons, the generated Rust app).

#[path = "trace.rs"]
mod trace;
use trace::TraceJournal;
pub use orbital_core::runtime::dispatch_trace::{TraceEffect, TraceEmit, TraceTransition};

use orbital_core::effects::{DeliveryRecord, DispatchLog};
use orbital_core::runtime::credentials::{credentials_with_rows, Credentials};
use orbital_core::runtime::kernel::guest_hosting::GuestIntegrations;
use orbital_core::runtime::native_host::NativeHost;
use orbital_core::runtime::os_host::completion::{
    CompletionAnswerer, CompletionDone, KernelCompletionSink,
};
use orbital_core::runtime::os_host::OsHandles;
use orbital_core::runtime::resumable::{
    drive_concurrent, CallSite, DispatchError, DispatchOutcomes, IoAnswer, IoRequest, Progress,
};
use orbital_core::runtime::entity_source::QueryAnswer;
use orbital_core::runtime::row_query::{
    RowIdPage, RowPage, RowQuery, RowSetRef, RowWriteBatch, RowWriteOp, RowWriteOutcome,
};
pub use orbital_core::runtime::{
    EntitySummary, OrbitalDetail, OrbitalInfo, ProcessEventResult, RuntimeStats,
};
use orbital_core::ValueMap;
use orbital_core::{
    runtime::{EntitySource, KernelConfig, KernelError, RuntimeKernel},
    LiveBroadcastItem, RuntimeEnvironment, SharedEventBus, TraitState, Value,
};
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Ring buffer size for the `/api/events` live-broadcast channel. A lagging
/// subscriber drops the oldest items rather than blocking dispatch (`broadcast_live`
/// never awaits a slow reader) — mirrors the TS live-push channel's fire-and-forget
/// fan-out (`Host.ts` `liveBroadcastSink`).
const LIVE_BROADCAST_CAPACITY: usize = 256;

use super::error::HostError;
use super::store::PersistenceAdapter;

/// Host configuration
#[derive(Clone)]
pub struct HostConfig {
    /// Maximum entities per collection
    pub max_entities_per_collection: usize,
    /// Enable event history
    pub enable_event_history: bool,
    /// The rows of `[runtime]` entities live in the kernel and are never an adapter call (EXEC-TICK-5, D-5).
    pub runtime_rows_in_kernel: bool,
    /// Maximum event history size
    pub max_event_history: usize,
    /// The host of the substrate operators (`program/*`, `behavior/*`): `orb` installs its own; without one they fail.
    pub substrate: Option<SubstratePort>,
    /// The dispatch stamp source every `@now` reads (`KernelConfig::clock`); `None` is the wall clock.
    pub clock: Option<orbital_core::runtime::DispatchClock>,
    /// Where the program's `cwd`/`home`/`exe-dir`/`env` grants and paths are anchored; `None` is this process's.
    pub anchors: Option<orbital_core::runtime::os_host::OsAnchors>,
}

impl std::fmt::Debug for HostConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostConfig")
            .field("max_entities_per_collection", &self.max_entities_per_collection)
            .field("enable_event_history", &self.enable_event_history)
            .field("runtime_rows_in_kernel", &self.runtime_rows_in_kernel)
            .field("max_event_history", &self.max_event_history)
            .field("substrate", &self.substrate)
            .field("clock", &self.clock.as_ref().map(|_| "pinned"))
            .field("anchors", &self.anchors)
            .finish()
    }
}

/// A substrate handler a host installs (`orbital_core` `SubstrateHandler`).
#[derive(Clone)]
pub struct SubstratePort(pub Arc<dyn orbital_core::effects::executor::SubstrateHandler>);

impl std::fmt::Debug for SubstratePort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SubstratePort")
    }
}

/// The server's substrate host: it answers `program/dispatch` itself (EXEC-AGENT-1: the declared input runs through
/// this server's input channel in the session of the dispatch that asked, on the asking thread) and hands every other
/// op to the host `orb` installs ([`HostConfig::substrate`]).
struct SelfSubstrate {
    runtime: std::sync::OnceLock<std::sync::Weak<Host>>,
    host: Option<Arc<dyn orbital_core::effects::executor::SubstrateHandler>>,
}

impl SelfSubstrate {
    fn shared(&self) -> Option<Arc<Host>> {
        self.runtime.get().and_then(std::sync::Weak::upgrade)
    }

    /// The work of a deferred keyed op (EXEC-QUEUE-13), built on the asking thread: it runs in the session that thread
    /// entered, so a deferred `program/dispatch` keeps the asking session's identity (EXEC-AGENT-1), and for the tab that
    /// thread served, so what that dispatch defers in turn completes on the asking tab (EXEC-QUEUE-10).
    fn later(self: &Arc<Self>, op: &str, args: &[Value]) -> orbital_core::runtime::native_host::SubstrateWork {
        let (this, op, args) = (Arc::clone(self), op.to_string(), args.to_vec());
        let session = this.shared().and_then(|runtime| runtime.kernel.thread_session());
        // A completion's thread serves no tab of its own; the client session it runs in is the asking tab's (EXEC-DIST-9).
        let origin = this
            .shared()
            .and_then(|runtime| runtime.origin_by_thread.read().get(&std::thread::current().id()).cloned())
            .or_else(|| session.as_deref().map(str::to_string));
        Box::new(move || {
            let _session = this.shared().zip(session).map(|(runtime, session)| runtime.kernel.enter_session(&session));
            let _origin = this.shared().map(|runtime| runtime.enter_origin(origin.as_deref()));
            orbital_core::effects::executor::SubstrateHandler::run(this.as_ref(), &op, &args)
        })
    }

    /// `program/live { clientId? }`: this host's live broadcast, as `orb serve`'s `/api/events` carries it. An item goes
    /// to the stream when it would go to that connection (`LiveBroadcastItem::delivered_to`): a peer's row only when the
    /// identity bound to the `clientId` session (the anonymous viewer when none is) may read it, never the stream's own
    /// `clientId`'s.
    fn live_source(&self, op: &str, args: &[Value]) -> Option<orbital_core::runtime::native_host::SubstrateSource> {
        use orbital_core::runtime::os_host::completion::StreamPoll;
        if op != "program/live" {
            return None;
        }
        let runtime = self.shared()?;
        let options = match args.first() {
            Some(Value::Object(options)) => options.clone(),
            _ => ValueMap::new().into(),
        };
        if options.get("kind").and_then(Value::as_str) == Some("trace") {
            return Some(runtime.trace.source(&options));
        }
        if options.get("kind").and_then(Value::as_str) == Some("guest") {
            let guest = options.get("guest").and_then(Value::as_str).unwrap_or_default().to_string();
            let host = Arc::downgrade(&runtime);
            return Some(Box::new(move || {
                let Some(runtime) = host.upgrade() else { return StreamPoll::Ended };
                match runtime.kernel.guest_tick_pass(&guest) {
                    Ok(orbital_core::runtime::kernel::GuestTicks::Painted(answer)) => StreamPoll::Item(answer),
                    Ok(orbital_core::runtime::kernel::GuestTicks::Idle) => StreamPoll::Idle,
                    Ok(orbital_core::runtime::kernel::GuestTicks::Closed) => StreamPoll::Ended,
                    Err(error) => StreamPoll::Failed(error),
                }
            }));
        }
        let client_id = options.get("clientId").and_then(Value::as_str).map(str::to_string);
        let mut receiver = runtime.subscribe_live();
        let host = Arc::downgrade(&runtime);
        let _stream = client_id.as_deref().map(|session| runtime.kernel.open_session_stream(session));
        Some(Box::new(move || loop {
            let _held = &_stream;
            let Some(runtime) = host.upgrade() else { return StreamPoll::Ended };
            let item = match receiver.try_recv() {
                Ok(item) => item,
                Err(broadcast::error::TryRecvError::Empty) => return StreamPoll::Idle,
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(broadcast::error::TryRecvError::Closed) => return StreamPoll::Ended,
            };
            let readable = |item: &LiveBroadcastItem| {
                let _session = client_id.as_deref().map(|session| runtime.kernel.enter_session(session));
                runtime.kernel.session_viewer().is_ok_and(|viewer| runtime.live_item_visible_to(item, &viewer))
            };
            if item.delivered_to(client_id.as_deref(), readable) {
                return StreamPoll::Item(live_item_value(&item));
            }
        }))
    }
}

/// One live item as a `program/live` stream event: `{ event, payload?, source }`, the `/api/events` frame without its
/// transport fields, or `{ type: dispatch, orbitalName, request, response }` for a dispatch the host ran on its own.
fn live_item_value(item: &LiveBroadcastItem) -> Value {
    let mut fields = ValueMap::new();
    if let Some(dispatch) = &item.dispatch {
        fields.insert("type".to_string().into(), Value::string("dispatch"));
        fields.insert("orbitalName".to_string().into(), Value::string(dispatch.orbital_name.clone()));
        fields.insert("request".to_string().into(), dispatch.request.clone());
        let response = serde_json::to_value(&dispatch.response).ok().and_then(|json| serde_json::from_value::<Value>(json).ok());
        fields.insert("response".to_string().into(), response.unwrap_or(Value::Null));
        return Value::object(fields);
    }
    fields.insert("event".to_string().into(), Value::string(item.event.clone()));
    if let Some(payload) = &item.payload {
        fields.insert("payload".to_string().into(), payload.clone());
    }
    let source = serde_json::to_value(&item.source).ok().and_then(|json| serde_json::from_value::<Value>(json).ok());
    fields.insert("source".to_string().into(), source.unwrap_or(Value::Null));
    Value::object(fields)
}

impl orbital_core::effects::executor::SubstrateHandler for SelfSubstrate {
    fn run(&self, op: &str, args: &[Value]) -> Result<Value, orbital_core::effects::executor::SubstrateFailure> {
        use orbital_core::effects::executor::SubstrateFailure;
        let failure = |message: String| SubstrateFailure { message, errors: Vec::new() };
        if op == "program/live" {
            return Err(failure("program/live streams to its onMessage route: give it a key and an onMessage route".to_string()));
        }
        if !matches!(op, "program/dispatch" | "program/inputs" | "program/tick" | "program/resolve-rows") {
            return match &self.host {
                Some(host) => host.run(op, args),
                None => Err(failure(format!("no substrate host configured for {op}"))),
            };
        }
        let runtime = self
            .runtime
            .get()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| failure(format!("{op}: this host is not shared (Host::shared)")))?;
        match op {
            "program/tick" => return runtime.tick_answer(args).map_err(failure),
            "program/resolve-rows" => {
                return runtime.resolve_rows_answer(args).map_err(|(message, status)| SubstrateFailure {
                    errors: status
                        .map(|status| vec![serde_json::from_value(serde_json::json!({ "code": "INVALID_REQUEST", "message": message, "status": status })).unwrap_or(Value::Null)])
                        .unwrap_or_default(),
                    message,
                })
            }
            "program/inputs" => return runtime.inputs_answer(args).map_err(failure),
            _ => {}
        }
        if let [Value::Object(ask)] = args {
            return runtime.inbound_answer(ask).map_err(failure);
        }
        if let [_, payload, Value::Object(options)] = args {
            return runtime.request_answer(payload, options).map_err(|(message, code, status)| SubstrateFailure {
                errors: vec![serde_json::from_value(serde_json::json!({ "code": code, "message": message, "status": status })).unwrap_or(Value::Null)],
                message,
            });
        }
        let (Some(Value::String(address)), Some(payload)) = (args.first(), args.get(1)) else {
            return Err(failure("program/dispatch: the host is asked for a qualified address and a payload".to_string()));
        };
        let mut parts = address.rsplitn(3, '.');
        let (Some(event), Some(trait_name)) = (parts.next(), parts.next()) else {
            return Err(failure(format!("program/dispatch: '{address}' is not a qualified address")));
        };
        let outcomes = runtime
            .dispatch_continuing(trait_name, orbital_core::RUNTIME_SINGLETON, event, payload.clone(), None, None)
            .map_err(|e| failure(e.to_string()))?;
        let response = orbital_core::EventResponse::from_process_results(&outcomes, trait_name, event);
        let asker = runtime.kernel.thread_session();
        runtime.broadcast_persisted(&response, asker.as_deref());
        let mut pushed = response.clone();
        runtime.kernel.stamp_emitted_sources(&mut pushed);
        runtime.kernel.seal_outcomes(&mut pushed, &outcomes, None, false);
        let ran = HostRan { orbital: None, trait_name, event, payload, entity_id: orbital_core::RUNTIME_SINGLETON };
        push_host_dispatch(&runtime.live_tx, Some(&runtime.kernel), &ran, pushed, asker.as_deref());
        serde_json::to_value(&response)
            .and_then(serde_json::from_value)
            .map_err(|e| failure(format!("program/dispatch: the response does not serialize: {e}")))
    }
}

/// Hosts every guest this host's kernel opens as a host of its own over in-memory rows (EXEC-CONC-7), so a guest's self
/// `program/dispatch`, keyed asks and route mounts are answered as a served program's are, under the grants it was passed.
/// A guest opened with `program: true` is answered by `substrate`, this host's own program port.
struct HostGuests {
    substrate: Option<SubstratePort>,
}

impl orbital_core::runtime::kernel::guest_hosting::GuestHosting for HostGuests {
    fn open(&self, integrations: GuestIntegrations, cwd: Option<&str>, env: &[(String, Option<String>)], program: bool) -> Arc<dyn orbital_core::runtime::kernel::guest_hosting::GuestHost> {
        let mut config = HostConfig::default();
        if program {
            config.substrate = self.substrate.clone();
        }
        if let Some(dir) = cwd {
            let mut anchors = orbital_core::runtime::os_host::OsAnchors::of_process();
            let home = anchors.home.clone();
            anchors.start = Some(match (dir.strip_prefix("~/"), home) {
                (Some(rest), Some(home)) => home.join(rest),
                _ => std::path::PathBuf::from(dir),
            });
            config.anchors = Some(anchors);
        }
        let host = Host::with_config(Arc::new(super::store::DriverAdapter::memory()), config);
        let host = match integrations {
            GuestIntegrations::Live => host,
            GuestIntegrations::Mock => host.holding_no_credentials(),
        };
        for (name, value) in env {
            host.handles.set_env(name, value.clone());
        }
        host.tap_completions();
        Arc::new(GuestOf(host.shared()))
    }
}

struct GuestOf(Arc<Host>);

impl orbital_core::runtime::kernel::guest_hosting::GuestHost for GuestOf {
    fn kernel(&self) -> Arc<RuntimeKernel> {
        Arc::clone(&self.0.kernel)
    }

    fn environment_of(&self, name: &str) -> Option<String> {
        self.0.handles.env_value(name)
    }

    fn credential_of(&self, name: &str) -> Option<String> {
        use orbital_core::runtime::secret_source::SecretSource as _;
        self.0.secrets.secret(name).ok().flatten()
    }

    fn deferred_denials(&self) -> Vec<(String, u64)> {
        self.0.handles.completions.denials()
    }

    fn register(&self, schema: orbital_core::OrbitalSchema) -> Result<(), String> {
        self.0.register_schema(schema).map_err(|e| e.to_string())
    }

    fn mount(&self, route: Option<&str>, seeds: &[orbital_core::effects::MountSeed], payload: Value) -> Result<DispatchOutcomes, String> {
        let host = &self.0;
        host.kernel
            .run_route_mount(route, seeds, None, payload, |started| host.finish(started))
            .map_err(|e| e.to_string())
    }

    fn dispatch(&self, trait_name: &str, event: &str, payload: Value) -> Result<DispatchOutcomes, String> {
        let host = &self.0;
        host.finish(host.kernel.start_dispatch(trait_name, None, event, payload, None, None))
            .map_err(|e| e.to_string())
    }

    fn tick_pass(&self) -> Result<DispatchOutcomes, String> {
        let host = &self.0;
        let mut outcomes: DispatchOutcomes = host.tick_pass_moving(host.monotonic_ms(), orbital_core::Calendar::local_now()).into_iter().flat_map(|(_, moved)| moved).collect();
        outcomes.extend(host.drain_completions().into_iter().filter_map(|completion| completion.outcomes.ok()).flatten());
        Ok(outcomes)
    }
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            max_entities_per_collection: 10000,
            enable_event_history: true,
            runtime_rows_in_kernel: true,
            max_event_history: 1000,
            substrate: None,
            clock: None,
            anchors: None,
        }
    }
}

impl From<HostConfig> for KernelConfig {
    fn from(config: HostConfig) -> Self {
        Self {
            environment: RuntimeEnvironment::Server,
            max_entities_per_collection: config.max_entities_per_collection,
            enable_event_history: config.enable_event_history,
            max_event_history: config.max_event_history,
            // A Server-env kernel never delegates (persist/fetch/call-service
            // execute locally) — only a client-role kernel is configured with
            // `runtime::server_leg::ServerLegCollector` (orbital-core B3/B4).
            delegate: None,
            service_handler: None,
            substrate_handler: None,
            clock: config.clock,
            unit: None,
            runtime_rows_in_kernel: config.runtime_rows_in_kernel,
        }
    }
}

/// Forgets the tab a request thread served.
pub struct OriginGuard {
    slot: Arc<RwLock<std::collections::HashMap<std::thread::ThreadId, String>>>,
    thread: std::thread::ThreadId,
    outer: Option<String>,
}

impl Drop for OriginGuard {
    fn drop(&mut self) {
        let mut slot = self.slot.write();
        match self.outer.take() {
            Some(outer) => slot.insert(self.thread, outer),
            None => slot.remove(&self.thread),
        };
    }
}

/// A dispatch's stream items in execution order: each outcome's effect results in declaration order, a client effect or
/// an emitted event each, then the whole response (`complete`), as orbital-server's `?stream` route replays them.
fn stream_items(outcomes: &[(String, ProcessEventResult)], response: &orbital_core::EventResponse) -> Vec<orbital_core::StreamItem> {
    let mut items = orbital_core::StreamItem::of_outcomes(outcomes);
    items.push(orbital_core::StreamItem::complete(response.clone()));
    items
}

/// The most dispatches the trace keeps; the oldest drop first.
const TRACE_CAPACITY: usize = 200;

/// One dispatch as the kernel ran it: each trait's transition with its guards and effects, and every ask the host answered.
/// Asks and answers are the kernel's own `Debug` forms, which show no credential, random byte or `os/` payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceEntry {
    pub seq: u64,
    pub dispatch_id: u64,
    pub at_ms: i64,
    pub seed: String,
    pub seed_entity_id: String,
    pub seed_traits: Vec<String>,
    pub instance: String,
    pub program_version: u64,
    pub outcomes: Vec<TraceTransition>,
    pub asks: Vec<TraceAsk>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceAsk {
    pub request: String,
    pub answer: String,
}

/// Fan the `persist` effect's success emits of `response` out to every other connected `/api/events` client.
/// A dispatch the host ran on its own: a deferred completion, or the input a `program/dispatch` named.
struct HostRan<'a> {
    orbital: Option<String>,
    trait_name: &'a str,
    event: &'a str,
    payload: &'a Value,
    entity_id: &'a str,
}

/// Send the tab whose request caused `ran` the client view of its sealed `response`, as a dispatch it folds
/// (EXEC-QUEUE-10, EXEC-AGENT-1). With no asking tab, or a trait of no orbital, nothing is sent.
fn push_host_dispatch(
    tx: &broadcast::Sender<LiveBroadcastItem>,
    kernel: Option<&RuntimeKernel>,
    ran: &HostRan<'_>,
    response: orbital_core::EventResponse,
    client_id: Option<&str>,
) {
    let orbital = ran.orbital.clone().or_else(|| kernel.and_then(|k| k.orbital_of_trait(ran.trait_name)));
    let (Some(client_id), Some(orbital)) = (client_id, orbital) else {
        return;
    };
    let (response, payload) = match kernel {
        Some(kernel) => (kernel.client_view(response), kernel.browser_completion_payload(ran.trait_name, ran.payload)),
        None => (response, ran.payload.clone()),
    };
    let _ = tx.send(LiveBroadcastItem::dispatched(orbital, ran.trait_name, ran.event, &payload, orbital_core::public_entity_id(ran.entity_id), response, client_id));
}

fn broadcast_persisted(
    tx: &broadcast::Sender<LiveBroadcastItem>,
    response: &orbital_core::EventResponse,
    client_id: Option<&str>,
) {
    for item in LiveBroadcastItem::persisted(response, client_id) {
        let _ = tx.send(item);
    }
}

fn host_of(
    kernel: &RuntimeKernel,
    source: &Arc<dyn EntitySource>,
    secrets: &Arc<Credentials>,
    handles: &Arc<OsHandles>,
    live: &broadcast::Sender<LiveBroadcastItem>,
    origins: &Arc<RwLock<std::collections::HashMap<std::thread::ThreadId, String>>>,
    substrate: &Arc<SelfSubstrate>,
) -> NativeHost {
    let live = live.clone();
    let origins = Arc::clone(origins);
    let on_event = move |site: Option<&CallSite>, payload: Value| {
        let Some(site) = site else { return };
        let Some(event) = site.emit.on_message.clone() else {
            return;
        };
        let origin = origins.read().get(&std::thread::current().id()).cloned();
        let _ = live.send(LiveBroadcastItem::call_message(
            event,
            payload,
            site.trait_name.clone(),
            origin,
        ));
    };
    NativeHost::new(
        source.clone(),
        secrets.clone(),
        kernel.os_grant_table(),
        handles.clone(),
        Arc::new(on_event),
    )
    .with_substrate(Some(Arc::clone(substrate) as Arc<dyn orbital_core::effects::executor::SubstrateHandler>))
    .with_substrate_later(Some({
        let substrate = Arc::clone(substrate);
        Arc::new(move |op: &str, args: &[Value]| substrate.later(op, args)) as Arc<orbital_core::runtime::native_host::SubstrateLater>
    }))
    .with_substrate_streams(Some({
        let substrate = Arc::clone(substrate);
        Arc::new(move |op: &str, args: &[Value]| substrate.live_source(op, args)) as Arc<orbital_core::runtime::native_host::SubstrateStreams>
    }))
}

/// Adapter that exposes a `PersistenceAdapter` as an `EntitySource` for the kernel.
struct PersistenceEntitySource {
    adapter: Arc<dyn PersistenceAdapter>,
}

impl PersistenceEntitySource {
    fn new(adapter: Arc<dyn PersistenceAdapter>) -> Self {
        Self { adapter }
    }
}

impl EntitySource for PersistenceEntitySource {
    fn read(&self, entity_type: &str, id: &str) -> Result<Arc<Value>, String> {
        self.adapter.read(entity_type, id).map(Arc::new)
    }

    fn find(&self, entity_type: &str, id: &str) -> Result<Option<Arc<Value>>, String> {
        self.adapter
            .find(entity_type, id)
            .map(|row| row.map(Arc::new))
    }

    fn write(&self, entity_type: &str, id: &str, data: &Value) -> Result<Value, String> {
        let update = RowWriteOp::Update {
            entity_type: entity_type.to_string(),
            id: id.to_string(),
            data: data.clone(),
            guard: None,
        };
        match self.adapter.write_where(&update) {
            Some(Ok(RowWriteOutcome::Updated { row, .. })) => return Ok(row),
            Some(Ok(RowWriteOutcome::Failed { message })) => return Err(message),
            Some(Err(e)) => return Err(e),
            Some(Ok(_)) => {}
            None => {
                if self.adapter.update(entity_type, id, data).is_ok() {
                    return self.adapter.read(entity_type, id);
                }
            }
        }
        let mut data_with_id = data.clone();
        if let Some(obj) = data_with_id.as_object_mut() {
            obj.insert("id".to_string().into(), Value::string(id.to_string()));
        }
        self.adapter.create(entity_type, &data_with_id)
    }

    fn query_page(&self, entity_type: &str, query: &RowQuery) -> QueryAnswer<RowPage> {
        self.adapter.query_page(entity_type, query)
    }

    fn query_ids(&self, entity_type: &str, query: &RowQuery) -> QueryAnswer<RowIdPage> {
        self.adapter.query_ids(entity_type, query)
    }

    fn write_where(&self, op: &RowWriteOp) -> Option<Result<RowWriteOutcome, String>> {
        self.adapter.write_where(op)
    }

    fn write_batch(&self, batch: &RowWriteBatch) -> Option<Result<Vec<RowWriteOutcome>, String>> {
        self.adapter.write_batch(batch)
    }

    fn create(&self, entity_type: &str, data: &Value) -> Result<Value, String> {
        self.adapter.create(entity_type, data)
    }

    fn list(&self, entity_type: &str) -> Result<Vec<Arc<Value>>, String> {
        self.adapter
            .list_filtered(entity_type, &std::collections::HashMap::new())
            .map(|rows| rows.into_iter().map(Arc::new).collect())
    }

    fn delete(&self, entity_type: &str, id: &str) -> Result<(), String> {
        self.adapter.delete(entity_type, id)
    }

    fn bind_collection(&self, entity_type: &str, collection: &str) {
        self.adapter.bind_collection(entity_type, collection);
    }

    fn clear(&self) {
        for entity_type in self.adapter.entity_types() {
            let _ = self.adapter.delete_all(&entity_type);
        }
    }

    fn count(&self) -> usize {
        self.adapter
            .entity_types()
            .iter()
            .map(|t| self.adapter.count(t).unwrap_or(0))
            .sum()
    }
}


/// The native program host.
///
/// Manages schema registration, state machine execution, and effect handling by
/// delegating to `orbital_core::RuntimeKernel`.
pub struct Host {
    #[allow(dead_code)]
    config: HostConfig,
    kernel: Arc<RuntimeKernel>,
    /// The one native host every dispatch's asks go to; rebuilt when a program registers, since its egress and grants change.
    host: Arc<RwLock<Arc<NativeHost>>>,
    secrets: Arc<Credentials>,
    handles: Arc<OsHandles>,
    /// Orders a deferred completion after the dispatch that deferred its key, and after no other.
    sink: Arc<KernelCompletionSink>,
    trace: Arc<TraceJournal>,
    _trace_lifetime: Arc<()>,
    /// The tab each request thread serves (`EventRequest.client_id`): where a running call's live messages go.
    origin_by_thread: Arc<RwLock<std::collections::HashMap<std::thread::ThreadId, String>>>,
    persistence: Arc<dyn PersistenceAdapter>,
    /// The same `EntitySource` the kernel reads through.
    entity_source: Arc<dyn EntitySource>,
    /// The `/api/events` live-broadcast channel (`routes::live_events`). One sender
    /// shared by every subscriber connection; dropped receivers are pruned by `tokio`
    /// on the next send.
    live_tx: broadcast::Sender<LiveBroadcastItem>,
    /// Hook deliveries waiting for an answer a later completion dispatch carries ([`Self::deliver_inbound_awaiting`]).
    inbound_waiters: InboundWaiters,
    /// Background ticks between their dispatch and the hand-off of their outcomes to `inbound_waiters`: a delivery waiting on a tick is not unanswered while its answer is in flight.
    ticks_in_flight: std::sync::atomic::AtomicUsize,
    substrate: Arc<SelfSubstrate>,
    /// Why the encrypted credential store a master key asks for could not be opened (the environment answers instead).
    credential_store_failure: Option<String>,
    /// Which declared ticks are due on each pass (`TickClock`), rebuilt when a program registers.
    ticks: Mutex<orbital_core::TickClock>,
    /// The clock tick passes read: milliseconds since this runtime was made.
    started: std::time::Instant,
    /// The completions delivered since [`Self::tap_completions`], for a host that reports them (`program/step` live).
    completion_tap: Arc<Mutex<Option<Vec<orbital_core::step_replay::StepCompletion>>>>,
}

/// Hand a later dispatch's outcomes to the hook deliveries waiting for an answer: a delivery the outcomes answer
/// is answered and leaves, the others collect the cascade's failures.
fn feed_inbound_waiters(waiters: &InboundWaiters, outcomes: &DispatchOutcomes) {
    waiters.lock().retain(|(route, answer, failures)| match RuntimeKernel::inbound_late_answer(route, outcomes) {
        Some(late) => {
            let _ = answer.send(late);
            false
        }
        None => {
            failures.lock().extend(RuntimeKernel::cascade_failures(outcomes));
            true
        }
    });
}

type InboundWaiters = Arc<
    Mutex<
        Vec<(
            orbital_core::runtime::inbound::InboundRoute,
            std::sync::mpsc::Sender<orbital_core::runtime::inbound::InboundAnswer>,
            Arc<Mutex<Vec<String>>>,
        )>,
    >,
>;

fn declare_persistence_capabilities(
    kernel: &RuntimeKernel,
    persistence: &dyn PersistenceAdapter,
    schema: &orbital_core::OrbitalSchema,
) {
    let query = persistence.query_capabilities();
    let write = persistence.write_capabilities();
    if query.is_none() && write.is_none() {
        return;
    }
    for orbital in &schema.orbitals {
        let entities =
            std::iter::once(&orbital.entity).chain(orbital.auxiliary_entities.iter().flatten());
        for entity in entities {
            let orbital_core::EntityRef::Inline(def) = entity else {
                continue;
            };
            if let Some(c) = &query {
                kernel.set_query_capabilities(&def.name, c.clone());
            }
            if let Some(c) = &write {
                kernel.set_write_capabilities(&def.name, c.clone());
            }
        }
    }
}

/// The host's error for a kernel refusal.
fn host_error(e: KernelError) -> HostError {
    match e {
        KernelError::TraitNotFound(name) => HostError::TraitNotFound(name),
        KernelError::Internal(msg) => HostError::Internal(msg),
        KernelError::PayloadInvalid(msg) => HostError::Validation(msg),
        e @ (KernelError::LaneHeldByParkedDispatch { .. }
        | KernelError::UnresolvedProgram { .. }
        | KernelError::LazyLoad { .. }
        | KernelError::LanguageData(_)) => {
            HostError::Internal(e.to_string())
        }
    }
}

impl Host {
    /// Create a new runtime with the given persistence adapter
    pub fn new<T>(adapter: Arc<T>) -> Self
    where
        T: PersistenceAdapter + 'static,
    {
        Self::with_config(adapter, HostConfig::default())
    }

    /// Create a new runtime with custom configuration
    pub fn with_config<T>(adapter: Arc<T>, config: HostConfig) -> Self
    where
        T: PersistenceAdapter + 'static,
    {
        Self::with_persistence(adapter, config)
    }

    /// Create a runtime over a store chosen at run time.
    pub fn with_persistence(
        persistence: Arc<dyn PersistenceAdapter>,
        config: HostConfig,
    ) -> Self {
        let entity_source: Arc<dyn EntitySource> =
            Arc::new(PersistenceEntitySource::new(persistence.clone()));
        let substrate = Arc::new(SelfSubstrate {
            runtime: std::sync::OnceLock::new(),
            host: config.substrate.clone().map(|port| port.0),
        });
        let mut kernel_config: KernelConfig = config.clone().into();
        kernel_config.substrate_handler =
            Some(Arc::clone(&substrate) as Arc<dyn orbital_core::effects::executor::SubstrateHandler>);

        let kernel = Arc::new(RuntimeKernel::new(
            RuntimeEnvironment::Server,
            entity_source.clone(),
            kernel_config,
        ));
        kernel.set_stateful(true);
        kernel.set_guest_hosting(Arc::new(HostGuests { substrate: config.substrate.clone() }));
        let (live_tx, _) = broadcast::channel(LIVE_BROADCAST_CAPACITY);
        let rows = persistence.durable().then(|| {
            Arc::new(orbital_core::runtime::credential_store::StoreCredentialRows(persistence.clone()))
                as Arc<dyn orbital_core::runtime::credential_store::CredentialRows>
        });
        let (secrets, credential_store_failure) = credentials_with_rows(rows);
        let egress_of = Arc::clone(&kernel);
        let secrets = Arc::new(secrets.with_grant_transport(Arc::new(
            orbital_core::runtime::credential_transport::TlsFormTransport::new(Arc::new(move || egress_of.egress())),
        )));
        let origin_by_thread: Arc<
            RwLock<std::collections::HashMap<std::thread::ThreadId, String>>,
        > = Arc::default();
        let mut os_handles = OsHandles::default();
        if let Some(anchors) = config.anchors.clone() {
            os_handles.anchors = anchors;
        }
        #[cfg(feature = "gpu")]
        os_handles.install_gpu(Arc::new(orbital_canvas::GpuHost::new()));
        let handles = Arc::new(os_handles);
        let host = Arc::new(RwLock::new(Arc::new(host_of(
            &kernel,
            &entity_source,
            &secrets,
            &handles,
            &live_tx,
            &origin_by_thread,
            &substrate,
        ))));
        let trace_lifetime = Arc::new(());
        let trace = Arc::new(TraceJournal::new(Arc::downgrade(&trace_lifetime)));
        let enabled_trace = Arc::downgrade(&trace);
        let observed_trace = Arc::downgrade(&trace);
        kernel.set_dispatch_observer(Some(Arc::new(orbital_core::runtime::dispatch_trace::DispatchObserver {
            enabled: Box::new(move || enabled_trace.upgrade().is_some_and(|trace| trace.needs_completion())),
            completed: Box::new(move |observed| {
                if let Some(trace) = observed_trace.upgrade() {
                    trace.record(observed);
                }
            }),
        })));
        let answer: Arc<CompletionAnswerer> = {
            let slot = Arc::downgrade(&host);
            let trace = trace.clone();
            Arc::new(move |request: &IoRequest| match slot.upgrade() {
                Some(host) => {
                    let current = host.read().clone();
                    let answer = current.answer(request);
                    trace.record_ask(request, &answer);
                    answer
                }
                None => IoAnswer::Service(Err("the host has stopped".to_string())),
            })
        };
        let inbound_waiters: InboundWaiters = Arc::default();
        let completion_tap: Arc<Mutex<Option<Vec<orbital_core::step_replay::StepCompletion>>>> = Arc::default();
        let done: Arc<CompletionDone> = {
            let tx = live_tx.clone();
            let stamping = Arc::downgrade(&kernel);
            let waiters = inbound_waiters.clone();
            let substrate = substrate.clone();
            let tap = completion_tap.clone();
            Arc::new(
                move |site: &CallSite,
                      event: &str,
                      payload: &Value,
                      client_id: Option<&str>,
                      result: Result<DispatchOutcomes, DispatchError>| {
                    if let Some(taken) = tap.lock().as_mut() {
                        taken.push(orbital_core::step_replay::StepCompletion {
                            event: event.to_string(),
                            payload: payload.clone(),
                            failed: site.emit.failure.as_deref() == Some(event),
                            outcomes: result.as_ref().map(Clone::clone).map_err(|e| e.to_string()),
                        });
                    }
                    match result
                {
                    Ok(outcomes) => {
                        feed_inbound_waiters(&waiters, &outcomes);
                        let mut response = orbital_core::EventResponse::from_process_results(
                            &outcomes,
                            &site.trait_name,
                            event,
                        );
                        let kernel = stamping.upgrade();
                        if let Some(kernel) = &kernel {
                            kernel.stamp_emitted_sources(&mut response);
                            let _session = client_id.map(|client| kernel.enter_session(client));
                            kernel.seal_outcomes(&mut response, &outcomes, None, false);
                        }
                        broadcast_persisted(&tx, &response, client_id);
                        if let Some(kernel) = &kernel {
                            for item in LiveBroadcastItem::program_wide(kernel.program_wide_emits(&response), client_id) {
                                let _ = tx.send(item);
                            }
                        }
                        let ran = HostRan { orbital: site.orbital.clone(), trait_name: &site.trait_name, event, payload, entity_id: &site.entity_id };
                        push_host_dispatch(&tx, kernel.as_deref(), &ran, response, client_id);
                    }
                    Err(error) => {
                        tracing::warn!(
                            target: orbital_core::logging::ns::SERVER_DATA,
                            trait_name = %site.trait_name,
                            event,
                            %error,
                            "deferred completion dispatch failed",
                        );
                        for (_, _, failures) in waiters.lock().iter() {
                            failures.lock().push(format!("{} {event}: {error}", site.trait_name));
                        }
                    }
                }
                    if let Some(host) = substrate.shared() {
                        host.kernel.free_quiet_sessions();
                    }
                },
            )
        };
        let sink = Arc::new(KernelCompletionSink::new(kernel.clone(), answer, done));
        handles.completions.set_sink(sink.clone());
        kernel.set_session_pending(Arc::new(orbital_core::runtime::os_host::completion::OsSessionPending::new(
            handles.completions.clone(),
            sink.clone(),
        )));

        Self {
            config,
            kernel,
            host,
            secrets,
            handles,
            sink,
            trace,
            _trace_lifetime: trace_lifetime,
            origin_by_thread,
            persistence,
            entity_source,
            live_tx,
            inbound_waiters,
            ticks_in_flight: std::sync::atomic::AtomicUsize::new(0),
            substrate,
            credential_store_failure,
            ticks: Mutex::new(orbital_core::TickClock::from_orbitals(&[])),
            started: std::time::Instant::now(),
            completion_tap,
        }
    }

    /// This runtime behind the `Arc` every host serves it from. A `program/dispatch` reaches the server's input channel
    /// only through a shared runtime.
    pub fn shared(self) -> Arc<Self> {
        let shared = Arc::new(self);
        let _ = shared.substrate.runtime.set(Arc::downgrade(&shared));
        shared
    }

    /// Subscribe to the `/api/events` live-broadcast channel — one receiver per
    /// connected client (`routes::live_events`).
    pub fn subscribe_live(&self) -> broadcast::Receiver<LiveBroadcastItem> {
        self.live_tx.subscribe()
    }

    /// Fan an emitted event out to every OTHER connected `/api/events` client. A
    /// no-op when nobody is subscribed (`send` errors only on zero receivers, which
    /// this ignores — mirrors the TS sink's fire-and-forget `liveBroadcastSink?.(...)`).
    pub fn broadcast_live(&self, item: LiveBroadcastItem) {
        let _ = self.live_tx.send(item);
    }

    /// `reference` resolved against the store as it is now, for the identity bound to the session this thread is
    /// in (the anonymous viewer when none is): the one resolver every renderer reaches, `@read` applied inside it.
    pub fn resolve_rows(&self, reference: &RowSetRef) -> Result<RowPage, String> {
        self.kernel.resolve_rows(reference)
    }

    /// Evaluate the request's dispatches as the viewer's `locale` until the guard drops.
    pub fn enter_locale(
        &self,
        locale: Option<&str>,
    ) -> Result<orbital_core::runtime::kernel::LocaleGuard, HostError> {
        self.kernel
            .enter_locale(locale)
            .map_err(|e| HostError::Internal(e.to_string()))
    }

    /// Register a schema with the runtime
    /// The program registered under `name`, shared with the kernel rather than copied.
    pub fn registered_schema(&self, name: &str) -> Option<Arc<orbital_core::OrbitalSchema>> {
        self.kernel.registered_schema(name)
    }

    /// This host answers every credential ask as not held: a guest opened with mocked integrations.
    fn holding_no_credentials(mut self) -> Self {
        self.secrets = Arc::new(Credentials::default());
        *self.host.write() = Arc::new(host_of(
            &self.kernel,
            &self.entity_source,
            &self.secrets,
            &self.handles,
            &self.live_tx,
            &self.origin_by_thread,
            &self.substrate,
        ));
        self
    }

    pub fn register_schema(&self, schema: orbital_core::OrbitalSchema) -> Result<(), HostError> {
        self.declare_capabilities(&schema);
        self.kernel
            .register_schema(schema)
            .map_err(|e| HostError::Internal(e.to_string()))?;
        *self.ticks.lock() = orbital_core::TickClock::from_programs(&self.kernel.registered_programs());
        self.trace.version.fetch_add(1, Ordering::SeqCst);
        *self.host.write() = Arc::new(host_of(
            &self.kernel,
            &self.entity_source,
            &self.secrets,
            &self.handles,
            &self.live_tx,
            &self.origin_by_thread,
            &self.substrate,
        ));
        Ok(())
    }

    /// Fan the `persist` success emits of `response` out to every other `/api/events` client (see [`Self::broadcast_live`]).
    pub fn broadcast_persisted(
        &self,
        response: &orbital_core::EventResponse,
        client_id: Option<&str>,
    ) {
        broadcast_persisted(&self.live_tx, response, client_id);
        for item in LiveBroadcastItem::program_wide(self.kernel.program_wide_emits(response), client_id) {
            let _ = self.live_tx.send(item);
        }
    }

    /// Why the credential store `ALMADAR_CREDENTIAL_MASTER_KEY` asks for could not be opened, when it could not.
    pub fn credential_store_failure(&self) -> Option<&str> {
        self.credential_store_failure.as_deref()
    }

    /// Milliseconds since this runtime was made: the monotonic clock of [`Self::tick_pass`].
    pub fn monotonic_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// One tick pass at `now_ms` (a monotonic clock): every due `[background]` tick runs over its rows, selected as
    /// `EmbeddedServer::tick_pass` selects it. A tick that is not `[background]` runs only where a client mounts its trait (the client's frame
    /// clock asks for it), so this server, which tracks no mounts, never fires one; a guest's host, whose kernel runs its
    /// `[client]` traits itself (EXEC-ENDPOINT-3a), fires those of the traits its page holds. Answers the `(trait, tick)` pairs that ran.
    pub fn tick_pass(&self, now_ms: u64, calendar: orbital_core::Calendar) -> Vec<(String, String)> {
        self.tick_pass_moving(now_ms, calendar).into_iter().map(|(ran, _)| ran).collect()
    }

    /// [`Self::tick_pass`], with what each tick that ran moved.
    fn tick_pass_moving(&self, now_ms: u64, calendar: orbital_core::Calendar) -> Vec<((String, String), DispatchOutcomes)> {
        let due = self.ticks.lock().due(now_ms, || calendar);
        let mut ran = Vec::new();
        for (trait_name, tick) in due {
            if !self.kernel.tick_runs(&trait_name, &tick) {
                continue;
            }
            self.ticks_in_flight.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let started = self.kernel.start_dispatch_tick(&trait_name, None, &tick);
            let finished = self.finish(started);
            if let Ok(outcomes) = &finished {
                feed_inbound_waiters(&self.inbound_waiters, outcomes);
            }
            self.ticks_in_flight.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            match finished {
                Ok(outcomes) => {
                    let response = orbital_core::EventResponse::from_process_results(&outcomes, "", &orbital_core::tick_event_key(&tick));
                    self.broadcast_persisted(&response, None);
                    ran.push(((trait_name, tick), outcomes));
                }
                Err(error) => tracing::error!(target: orbital_core::logging::ns::SERVER_DATA, trait_name = %trait_name, tick = %tick, %error, "tick failed"),
            }
        }
        ran
    }

    /// `program/tick`: one [`Self::tick_pass`] now, answered `{ ran: [{ trait, tick }] }`.
    fn tick_answer(&self, args: &[Value]) -> Result<Value, String> {
        if !args.is_empty() {
            return Err("program/tick: takes no arguments, only its { emit } options".to_string());
        }
        let ran = self.tick_pass(self.monotonic_ms(), orbital_core::Calendar::local_now());
        let ran: Vec<serde_json::Value> =
            ran.into_iter().map(|(trait_name, tick)| serde_json::json!({ "trait": trait_name, "tick": tick })).collect();
        serde_json::from_value(serde_json::json!({ "ran": ran })).map_err(|e| e.to_string())
    }

    /// A `program/inputs` asked of this host. The kernel's own `{ trace: n }` ask answers the last `n` traced dispatch
    /// steps, oldest first. Any other ask (a kernel that does not answer the agent ports itself) answers the reflection
    /// the kernel answers: the declared inputs, the readable entities and the `select`ed inputs, plus `trace` when
    /// asked. `state` and `stats` are read by the kernel only.
    fn inputs_answer(&self, args: &[Value]) -> Result<Value, String> {
        use orbital_core::runtime::agent_ports::{inputs_reads, plan_inputs};
        let reads = inputs_reads(args)?;
        let trace = || -> Result<Value, String> {
            let trace = self.trace();
            let skip = trace.len().saturating_sub(reads.trace.unwrap_or(0));
            serde_json::to_value(&trace[skip..]).and_then(serde_json::from_value).map_err(|e| e.to_string())
        };
        if matches!(args, [Value::Object(ask)] if ask.len() == 1 && ask.contains_key("trace")) {
            return trace();
        }
        if reads.state.is_some() || reads.stats {
            return Err("program/inputs: state and stats are read by the kernel, not asked of the host".to_string());
        }
        let reflection = plan_inputs(args, &self.external_inputs(), &self.kernel.readable_entities(), None, "")
            .map_err(|failure| failure.error)?;
        match (reflection, reads.trace) {
            (Value::Object(mut answer), Some(_)) => {
                answer.insert("trace".to_string().into(), trace()?);
                Ok(Value::Object(answer))
            }
            (reflection, _) => Ok(reflection),
        }
    }

    /// The `{ inbound, body, headers }` ask of a `program/dispatch`: [`Self::deliver_inbound`], answered `{ status, body }`.
    /// The delivery runs inside the asking dispatch, so an answer only a later completion carries is not waited for.
    fn inbound_answer(&self, ask: &ValueMap) -> Result<Value, String> {
        let shape = "program/dispatch: the host is asked for { inbound, body, headers }";
        let (Some(Value::String(name)), Some(Value::String(body)), Some(Value::Object(headers))) =
            (ask.get("inbound"), ask.get("body"), ask.get("headers"))
        else {
            return Err(shape.to_string());
        };
        let headers: Vec<(&str, &str)> =
            headers.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.as_str(), v))).collect();
        let answer = self.deliver_inbound(name, headers, body);
        let mut out = ValueMap::new();
        out.insert("status".to_string().into(), Value::Int(answer.status.into()));
        out.insert("body".to_string().into(), Value::string(answer.body));
        Ok(Value::object(out))
    }

    /// A `program/dispatch` event request (owner, 2026-10-21): each option goes onto the seam the events and inputs
    /// routes used. `clientId` names the session (so `@user` is its bound identity) and its origin, `locale` its locale; `input: true` admits
    /// only a declared external input; `mount` runs the lifecycle seeds, no `targetTrait`
    /// discovers the handlers; the host holds the circuit state, so the kernel refuses carried `traits` outside a delegated leg (EXEC-DIST-11); `delivery`/`dispatchLog` continue a client-begun dispatch. The host answers the raw
    /// `EventResponse` (plus `items` for `stream`); the kernel shapes what a model is told. An empty text, record or mount list is absent.
    fn request_answer(&self, payload: &Value, options: &ValueMap) -> Result<Value, (String, &'static str, u16)> {
        use orbital_core::EventResponse;
        let json = |key: &str| options.get(key).map(orbital_core::json_value::value_to_json);
        let text = |key: &str| json(key).and_then(|v| v.as_str().filter(|s| !s.is_empty()).map(str::to_string));
        let flag = |key: &str| json(key).and_then(|v| v.as_bool()).unwrap_or(false);
        let request = orbital_core::EventRequest::from_dispatch(orbital_core::json_value::value_to_json(payload), &json)
            .map_err(|message| (message, "INVALID_REQUEST", 400))?;
        let Some(orbital) = text("orbital") else {
            return Err(("program/dispatch: an event request names its orbital".to_string(), "INVALID_REQUEST", 400));
        };
        if let Some(refused) = self.kernel.refusal(&request) {
            let locale = refused.rejections.as_ref().and_then(|r| r.first()).is_some_and(|r| r.code == orbital_core::effects::response::TransitionRejectionCode::LocaleNotDeclared);
            if locale {
                return Err((refused.error.unwrap_or_default(), "LOCALE_NOT_DECLARED", 400));
            }
            return serde_json::to_value(&refused)
                .and_then(serde_json::from_value)
                .map_err(|e| (e.to_string(), "EVENT_PROCESSING_ERROR", 500));
        }
        let entity_id = request.entity_id.clone().unwrap_or_else(|| orbital_core::RUNTIME_SINGLETON.to_string());
        let payload = request.payload.clone().unwrap_or(Value::Null);
        let failed = |e: HostError| (e.to_string(), "EVENT_PROCESSING_ERROR", e.status_code());
        let _origin = self.enter_origin(request.client_id.as_deref());
        let mounting = request.mount.as_deref().is_some_and(|seeds| !seeds.is_empty());
        let admitted = self
            .kernel
            .admit_request(request.client_id.as_deref(), mounting, flag("wire"))
            .map_err(|ended| (ended.to_string(), "SESSION_ENDED", 409))?;
        let restored = match &admitted {
            orbital_core::runtime::kernel::RequestSession::Client { restored, .. } => restored.clone(),
            _ => Default::default(),
        };
        let _session = match &admitted {
            orbital_core::runtime::kernel::RequestSession::Client { active, .. } => Some(self.kernel.enter_session(active.session())),
            _ => None,
        };
        if let Some(since) = request.frames_since {
            let answer = match self.kernel.frames_since(since) {
                Some(frames) => EventResponse { success: true, frames: Some(frames), ..Default::default() },
                None => EventResponse::error("this host keeps no frames for the session: mount again"),
            };
            let answer = if flag("wire") { self.kernel.client_view(answer) } else { answer };
            return serde_json::to_value(&answer)
                .and_then(serde_json::from_value)
                .map_err(|e| (e.to_string(), "EVENT_PROCESSING_ERROR", 500));
        }
        self.kernel.ensure_orbital_loaded(&orbital).map_err(|e| (e.to_string(), "EVENT_PROCESSING_ERROR", 500))?;
        let Some(detail) = self.get_orbital(&orbital) else {
            return Err((format!("Orbital not found: {orbital}"), "ORBITAL_NOT_FOUND", 404));
        };
        let respond = |outcomes: &[(String, ProcessEventResult)], mut response: EventResponse| -> Result<Value, (String, &'static str, u16)> {
            self.kernel.stamp_emitted_sources(&mut response);
            let items = if flag("stream") {
                {
                let mut items = stream_items(outcomes, &response);
                self.kernel.redact_stream_items(&mut items);
                Some(serde_json::to_value(items).map_err(|e| (e.to_string(), "EVENT_PROCESSING_ERROR", 500))?)
            }
            } else {
                None
            };
            let response = if flag("wire") { self.kernel.client_view(response) } else { response };
            let mut answer = serde_json::to_value(&response).map_err(|e| (e.to_string(), "EVENT_PROCESSING_ERROR", 500))?;
            if let (Some(items), serde_json::Value::Object(fields)) = (items, &mut answer) {
                fields.insert("items".to_string(), items);
            }
            serde_json::from_value(answer).map_err(|e| (e.to_string(), "EVENT_PROCESSING_ERROR", 500))
        };
        type Ran = (Vec<(String, ProcessEventResult)>, EventResponse);
        let targeted = |trait_name: &str, delivery: Option<&DeliveryRecord>, log: Option<&DispatchLog>| -> Result<Ran, (String, &'static str, u16)> {
            let outcomes = self.dispatch_continuing(trait_name, &entity_id, &request.event, payload.clone(), delivery, log).map_err(failed)?;
            let mut response = EventResponse::from_process_results(&outcomes, trait_name, &request.event);
            self.kernel.seal_outcomes(&mut response, &outcomes, Some(&entity_id), false);
            self.echo_held_states(&mut response, trait_name, &entity_id);
            Ok((outcomes, response))
        };
        if flag("input") {
            let Some(trait_name) = request.target_trait.as_deref() else {
                return Err(("program/dispatch: an input request names its trait".to_string(), "NO_TRAIT", 400));
            };
            let declared = self.external_inputs().iter().any(|i| i.orbital == orbital && i.trait_name == trait_name && i.event == request.event);
            if !declared {
                return respond(&[], EventResponse::not_an_external_input(trait_name, &request.event));
            }
            let (outcomes, response) = targeted(trait_name, None, None)?;
            self.broadcast_persisted(&response, None);
            return respond(&outcomes, response);
        }
        if let Some(trait_name) = self.inbound_only_trait(&orbital, request.target_trait.as_deref(), &request.event) {
            return respond(&[], EventResponse::inbound_only(&trait_name, &request.event));
        }
        let _locale = self.enter_locale(request.locale.as_deref()).map_err(failed)?;
        let client_id = request.client_id.as_deref();
        let (outcomes, response): Ran = if let Some(seeds) = request.mount.as_deref().filter(|seeds| !seeds.is_empty()) {
            let plan = self.kernel.prepare_mount(&restored, seeds, request.traits.as_deref(), request.entity_by_trait.as_ref(), Some(&entity_id));
            let outcomes = if plan.unseeded.is_empty() { Vec::new() } else { self.dispatch_mount(request.route.as_deref(), &plan.unseeded, &entity_id, payload.clone()).map_err(failed)? };
            let mut response = EventResponse::from_process_results(&outcomes, &seeds[0].r#trait, seeds[0].event.as_str());
            let mut writes = self.kernel.frame_writes(&outcomes, Some(&entity_id), true);
            writes.extend(plan.restored_rows.iter().filter_map(|(trait_name, row)| {
                Some((self.kernel.trait_frame_key(trait_name), orbital_core::runtime::kernel::FrameWrite::Fields(row.as_object()?.clone())))
            }));
            self.kernel.seal_frames(&mut response, writes);
            self.echo_held_states(&mut response, &seeds[0].r#trait, &entity_id);
            (outcomes, response)
        } else if let Some(trait_name) = request.target_trait.as_deref() {
            targeted(trait_name, request.delivery.as_ref(), request.dispatch_log.as_ref())?
        } else {
            let handlers = self.discover_handlers(&detail.traits, &entity_id, &request.event);
            let held = self.held_states(&detail.traits, &entity_id);
            let outcomes = if handlers.is_empty() { Vec::new() } else { self.dispatch_each(&handlers, &entity_id, &request.event, payload.clone()).map_err(failed)? };
            let mut response = EventResponse::from_discovery(&outcomes, &handlers, &request.event, held);
            self.kernel.seal_outcomes(&mut response, &outcomes, Some(&entity_id), false);
            (outcomes, response)
        };
        self.broadcast_persisted(&response, client_id);
        respond(&outcomes, response)
    }

    /// `program/resolve-rows`: a row-set reference resolved for the identity bound to the asking session, `@read`
    /// applied inside the kernel's one resolver; answered `{ rows, total }`.
    fn resolve_rows_answer(&self, args: &[Value]) -> Result<Value, (String, Option<u16>)> {
        let (reference, options) = match args {
            [reference] => (reference, None),
            [reference, Value::Object(options)] => (reference, Some(options)),
            _ => return Err(("program/resolve-rows: the host is asked for a row-set reference and optionally `{ clientId }`".to_string(), None)),
        };
        let mut client = None;
        for (name, value) in options.into_iter().flat_map(|options| options.iter()) {
            match (name.as_str(), value) {
                ("clientId", Value::String(id)) if !id.is_empty() => client = Some(id.to_string()),
                _ => return Err(("program/resolve-rows: the only option is `clientId`, a non-empty session id".to_string(), Some(400))),
            }
        }
        let _admitted = match client.as_deref() {
            Some(id) => Some(self.kernel.admit_request(Some(id), false, false).map_err(|ended| (ended.to_string(), Some(409)))?),
            None => None,
        };
        let _session = _admitted.as_ref().and_then(|admitted| match admitted {
            orbital_core::runtime::kernel::RequestSession::Client { active, .. } => Some(self.kernel.enter_session(active.session())),
            _ => None,
        });
        let reference: RowSetRef = serde_json::from_value(orbital_core::json_value::value_to_json(reference))
            .map_err(|e| (format!("program/resolve-rows: not a row-set reference: {e}"), Some(400)))?;
        let page = self.kernel.resolve_rows(&reference).map_err(|e| (e, None))?;
        serde_json::to_value(page).and_then(serde_json::from_value).map_err(|e| (e.to_string(), None))
    }

    /// The process credentials: the TTL cache the token verifiers keep their key sets in.
    pub fn credentials(&self) -> Arc<Credentials> {
        Arc::clone(&self.secrets)
    }

    /// Whether `viewer` may read the row a live item carries: the `@read` policy of the entity the emitting trait is bound
    /// to, through the kernel's one row-access owner (the JS twin: `isRowVisibleToViewer`). An item whose entity does not
    /// resolve, or whose entity declares no policy, is visible to everyone.
    pub fn live_item_visible_to(&self, item: &LiveBroadcastItem, viewer: &Value) -> bool {
        self.kernel.live_item_visible_to(item, viewer)
    }

    /// Serve `client_id`'s tab from this thread until the guard drops: the live messages of the calls it runs go to that tab.
    pub fn enter_origin(&self, client_id: Option<&str>) -> OriginGuard {
        let thread = std::thread::current().id();
        let outer = self.origin_by_thread.read().get(&thread).cloned();
        if let Some(client_id) = client_id {
            self.origin_by_thread
                .write()
                .insert(thread, client_id.to_string());
        }
        OriginGuard {
            slot: Arc::clone(&self.origin_by_thread),
            thread,
            outer,
        }
    }

    /// Record every dispatch's transitions, guards, effects and asks with their answers (`GET /api/trace`).
    pub fn set_trace(&self, on: bool) {
        self.trace.on.store(on, Ordering::SeqCst);
    }

    /// Trace every dispatch and also append each entry to `path` as one JSON line: the history a restart keeps,
    /// beyond the last dispatches the memory trace holds.
    pub fn set_trace_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        *self.trace.file.lock() = Some((path.to_path_buf(), file));
        self.set_trace(true);
        Ok(())
    }

    /// Whether dispatches are traced.
    pub fn tracing(&self) -> bool {
        self.trace.enabled()
    }

    /// The file traced dispatches are appended to, when one is set.
    pub fn trace_file_path(&self) -> Option<std::path::PathBuf> {
        self.trace.file
            .lock()
            .as_ref()
            .map(|(path, _)| path.clone())
    }

    /// The recorded dispatches, oldest first.
    pub fn trace(&self) -> Vec<TraceEntry> {
        self.trace.log.lock().iter().cloned().collect()
    }

    /// Run a started dispatch to its end with the native host, recording its trace when tracing is on.
    fn finish(
        &self,
        started: Result<Progress<DispatchOutcomes>, DispatchError>,
    ) -> Result<DispatchOutcomes, HostError> {
        let host = self.host.read().clone();
        let origin = self.origin_by_thread.read().get(&std::thread::current().id()).cloned();
        let scope = self.sink.begin_dispatch_for(origin.as_deref());
        let answer = |request: &IoRequest| {
            let answered = scope.answer(&|request| host.answer(request), request);
            self.trace.record_ask(request, &answered);
            answered
        };
        let result = started.and_then(|progress| drive_concurrent(&self.kernel, progress, &answer));
        result.map_err(|e| match e {
            DispatchError::Kernel(e) => host_error(e),
            DispatchError::Resume(e) => HostError::Internal(e.to_string()),
        })
    }

    /// Tell the kernel what the adapter's push-down honours, for each inline entity the schema declares.
    fn declare_capabilities(&self, schema: &orbital_core::OrbitalSchema) {
        declare_persistence_capabilities(&self.kernel, self.persistence.as_ref(), schema);
    }

    /// Load this program's lazy orbitals through `source` (EXEC-LOAD-2): install it before the program is registered. A lazy
    /// orbital the kernel registers later gets the persistence capabilities the booted program's entities got.
    pub fn set_lazy_source(&self, source: Arc<dyn orbital_core::runtime::LazySource>) {
        self.kernel.set_lazy_source(source);
        let persistence = self.persistence.clone();
        self.kernel.on_lazy_loaded(Arc::new(move |kernel, schema| {
            declare_persistence_capabilities(kernel, persistence.as_ref(), schema)
        }));
    }

    /// Process an event for a specific entity
    pub fn process_event(
        &self,
        trait_name: &str,
        entity_id: &str,
        event_name: &str,
        payload: Value,
    ) -> Result<ProcessEventResult, HostError> {
        self.kernel
            .process_event(trait_name, Some(entity_id), event_name, payload)
            .map_err(host_error)
    }

    /// An inbound delivery (`/api/hooks/<name>`): the kernel decides whether the name is served, runs
    /// the handler and answers; this host only carries the bytes (EXEC-INBOUND-1).
    pub fn deliver_inbound<'a>(
        &self,
        name: &str,
        headers: impl IntoIterator<Item = (&'a str, &'a str)>,
        raw_body: &str,
    ) -> orbital_core::runtime::inbound::InboundAnswer {
        use orbital_core::runtime::inbound::dispatch_failed;
        let route = match self.kernel.inbound_route(name) {
            Ok(route) => route,
            Err(refused) => return refused,
        };
        let credential = match RuntimeKernel::inbound_credential(&route, self.secrets.as_ref()) {
            Ok(credential) => credential,
            Err(failed) => return failed,
        };
        let started = self
            .kernel
            .start_inbound(&route, headers, raw_body, credential.as_deref());
        match self.finish(started) {
            Ok(outcomes) => RuntimeKernel::inbound_answer(&route, &outcomes),
            Err(error) => dispatch_failed(&error.to_string()),
        }
    }

    /// [`Self::deliver_inbound`] for a host that serves one caller at a time (the `orb` command runner): a delivery
    /// whose handler asked for keyed I/O and did not answer in the delivering dispatch waits for the answer a later
    /// dispatch of the handler carries, as its completions arrive, up to `timeout` (`None` waits as long as a call is open;
    /// the host runs tick passes itself while it waits (the command runner starts no tick driver); with none left, and no `[background]` tick waiting in a state it names, the delivery is `inbound-unanswered`, `RuntimeKernel::await_inbound_answer`).
    pub fn deliver_inbound_awaiting<'a>(
        &self,
        name: &str,
        headers: impl IntoIterator<Item = (&'a str, &'a str)>,
        raw_body: &str,
        timeout: Option<std::time::Duration>,
    ) -> orbital_core::runtime::inbound::InboundAnswer {
        use orbital_core::runtime::inbound::dispatch_failed;
        let route = match self.kernel.inbound_route(name) {
            Ok(route) => route,
            Err(refused) => return refused,
        };
        let credential = match RuntimeKernel::inbound_credential(&route, self.secrets.as_ref()) {
            Ok(credential) => credential,
            Err(failed) => return failed,
        };
        let (sender, receiver) = std::sync::mpsc::channel();
        let failures: Arc<Mutex<Vec<String>>> = Arc::default();
        self.inbound_waiters.lock().push((route.clone(), sender, failures.clone()));
        let started = self
            .kernel
            .start_inbound(&route, headers, raw_body, credential.as_deref());
        let outcomes = match self.finish(started) {
            Ok(outcomes) => outcomes,
            Err(error) => {
                self.inbound_waiters
                    .lock()
                    .retain(|(waiting, _, _)| waiting != &route);
                return dispatch_failed(&error.to_string());
            }
        };
        if RuntimeKernel::inbound_answered(&route, &outcomes) {
            self.inbound_waiters
                .lock()
                .retain(|(waiting, _, _)| waiting != &route);
            return RuntimeKernel::inbound_answer(&route, &outcomes);
        }
        let stop = std::sync::atomic::AtomicBool::new(false);
        let answer = std::thread::scope(|scope| {
            scope.spawn(|| {
                while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                    self.tick_pass(self.monotonic_ms(), orbital_core::Calendar::local_now());
                    std::thread::sleep(std::time::Duration::from_millis(16));
                }
            });
            let answer = RuntimeKernel::await_inbound_answer(
                &route,
                &outcomes,
                &receiver,
                || !self.open_calls().is_empty() || self.ticks_wanted(),
                &|| failures.lock().clone(),
                timeout,
            );
            stop.store(true, std::sync::atomic::Ordering::SeqCst);
            answer
        });
        self.inbound_waiters
            .lock()
            .retain(|(waiting, _, _)| waiting != &route);
        answer
    }

    /// The trait that handles `event` as an inbound hook in `orbital`, when a client route's request names
    /// it: such a request is refused, only the hook intake delivers it.
    pub fn inbound_only_trait(
        &self,
        orbital: &str,
        trait_name: Option<&str>,
        event: &str,
    ) -> Option<String> {
        self.kernel.inbound_only_trait(orbital, trait_name, event)
    }

    /// Dispatch an event with full cross-trait listener fan-out (the canonical
    /// `RuntimeKernel::dispatch`): the target trait plus every trait that listens for
    /// the event (and any it emits). Returns one outcome per processed trait so the
    /// route merges them into a single response — so a button's `CREATE` reaches the
    /// modal trait that listens for it.
    pub fn dispatch(
        &self,
        trait_name: &str,
        entity_id: &str,
        event_name: &str,
        payload: Value,
    ) -> Result<Vec<(String, ProcessEventResult)>, HostError> {
        self.dispatch_continuing(trait_name, entity_id, event_name, payload, None, None)
    }

    /// [`Self::dispatch`] for a request carrying a client-begun dispatch's `delivery` / `dispatch_log`.
    pub fn dispatch_continuing(
        &self,
        trait_name: &str,
        entity_id: &str,
        event_name: &str,
        payload: Value,
        delivery: Option<&DeliveryRecord>,
        log: Option<&DispatchLog>,
    ) -> Result<Vec<(String, ProcessEventResult)>, HostError> {
        let started = self.kernel.start_dispatch(
            trait_name,
            Some(entity_id),
            event_name,
            payload,
            delivery,
            log,
        );
        self.finish(started)
    }

    /// The traits of `candidates` whose held state handles `event` (`RuntimeKernel::discover_handlers`).
    pub fn discover_handlers(
        &self,
        candidates: &[String],
        entity_id: &str,
        event_name: &str,
    ) -> Vec<String> {
        self.kernel
            .discover_handlers(candidates, Some(entity_id), event_name)
    }

    /// The held state of each of `traits` (`RuntimeKernel::held_states`).
    pub fn held_states(
        &self,
        traits: &[String],
        entity_id: &str,
    ) -> std::collections::HashMap<String, String> {
        self.kernel.held_states(traits, Some(entity_id))
    }

    /// The whole orbital's held states under `states` (`OrbitalEventResponse.states` is the whole-orbital snapshot): every trait of
    /// the orbital `seed_trait` belongs to, at its current state or its initial one while nothing has touched it; the states the
    /// dispatch produced stay as they are. The twin of the embedded server's `echo_held_states`.
    pub fn echo_held_states(
        &self,
        response: &mut orbital_core::EventResponse,
        seed_trait: &str,
        entity_id: &str,
    ) {
        let Some(orbital) = self.kernel.orbital_of_trait(seed_trait) else {
            return;
        };
        let Some(detail) = self.kernel.get_orbital(&orbital) else {
            return;
        };
        for (name, state) in self.kernel.held_states(&detail.traits, Some(entity_id)) {
            response.states.entry(name).or_insert(state);
        }
    }

    /// [`Self::dispatch`] to each of `traits` as its own seed — see `RuntimeKernel::dispatch_each`.
    pub fn dispatch_each(
        &self,
        traits: &[String],
        entity_id: &str,
        event_name: &str,
        payload: Value,
    ) -> Result<Vec<(String, ProcessEventResult)>, HostError> {
        let started = self.kernel.start_dispatch_each(
            traits,
            Some(entity_id),
            event_name,
            payload,
            None,
            None,
        );
        self.finish(started)
    }

    /// What mounts when this host starts on `side` (EXEC-ENDPOINT-3); the host dispatches the list and computes none.
    pub fn boot_mounts(&self, side: orbital_core::protocols::ProtocolSide) -> Vec<orbital_core::effects::MountSeed> {
        self.kernel.boot_mounts(side)
    }

    /// A mount batch (`EventRequest.mount`) opening `route` when it names one: the route's `$MOUNT` listeners run
    /// first (EXEC-SEED-7) — see `RuntimeKernel::dispatch_route_mount`.
    pub fn dispatch_mount(
        &self,
        route: Option<&str>,
        seeds: &[orbital_core::effects::MountSeed],
        entity_id: &str,
        payload: Value,
    ) -> Result<Vec<(String, ProcessEventResult)>, HostError> {
        self.kernel
            .run_route_mount(route, seeds, Some(entity_id), payload, |started| self.finish(started))
    }

    /// A `[background]` tick still wants to run (its trait waits in a state it names) or is mid-dispatch.
    fn ticks_wanted(&self) -> bool {
        self.ticks_in_flight.load(std::sync::atomic::Ordering::SeqCst) > 0 || !self.kernel.pending_background_ticks().is_empty()
    }

    /// The keyed calls whose completion is still wanted: open streams (a listener, a watch) and deferred answers.
    pub fn open_calls(&self) -> Vec<String> {
        self.handles.completions.live_keys()
    }

    /// Start recording the completions this host delivers; [`Self::take_completions`] returns and stops the record.
    pub fn tap_completions(&self) {
        *self.completion_tap.lock() = Some(Vec::new());
    }

    /// The completions delivered since the last drain, in delivery order, recording on.
    pub fn drain_completions(&self) -> Vec<orbital_core::step_replay::StepCompletion> {
        self.completion_tap.lock().as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// The completions delivered since [`Self::tap_completions`], in delivery order.
    pub fn take_completions(&self) -> Vec<orbital_core::step_replay::StepCompletion> {
        self.completion_tap.lock().take().unwrap_or_default()
    }

    /// Cancel every open call as `cancel-call` of its key does; a cancelled stream stops delivering.
    pub fn cancel_open_calls(&self) {
        for key in self.open_calls() {
            self.handles.cancel_completion(&key);
        }
    }

    /// Get the current state for an entity
    pub fn get_state(&self, trait_name: &str, entity_id: &str) -> Option<TraitState> {
        self.kernel.get_state(trait_name, Some(entity_id))
    }

    /// Get all registered orbitals
    pub fn list_orbitals(&self) -> Vec<OrbitalInfo> {
        self.kernel.list_orbitals()
    }

    /// Get orbital details
    pub fn get_orbital(&self, orbital_name: &str) -> Option<OrbitalDetail> {
        self.kernel.get_orbital(orbital_name)
    }

    /// Declared external inputs of every registered orbital.
    pub fn external_inputs(&self) -> Vec<orbital_core::runtime::external_inputs::ExternalInput> {
        self.kernel.external_inputs()
    }

    /// The kernel this host runs its program on.
    pub fn kernel(&self) -> &Arc<RuntimeKernel> {
        &self.kernel
    }

    /// Get the persistence adapter
    pub fn persistence(&self) -> &Arc<dyn PersistenceAdapter> {
        &self.persistence
    }

    /// Get the event bus
    pub fn event_bus(&self) -> &SharedEventBus {
        self.kernel.event_bus()
    }

    /// Get state manager stats
    pub fn stats(&self) -> RuntimeStats {
        self.kernel.stats()
    }

    /// Get the initial state for a trait
    pub fn get_initial_state(&self, trait_name: &str) -> Option<String> {
        self.kernel.get_initial_state(trait_name)
    }

    /// Reset state for an entity to initial state
    pub fn reset_state(&self, trait_name: &str, entity_id: &str) {
        self.kernel.reset_state(trait_name, Some(entity_id));
    }
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("config", &self.config)
            .field("kernel", &self.kernel)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::store::DriverAdapter;
    use orbital_core::Value;

    const EMULATOR_ISSUER: &str = "firebase-auth-emulator@example.com";

    /// A program with a `[runtime]` `[identity]` entity and a trait whose `SIGN_IN` runs `jws/verify` over an unsigned
    /// emulator token (EXEC-IDENTITY-5) into `identity/sign-in`.
    fn signing_program() -> orbital_core::OrbitalSchema {
        serde_json::from_value(serde_json::json!({
            "name": "TestIdentity",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "TestIdentityOrbital",
                "entity": { "name": "TestPrincipal", "identity": true, "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{
                    "name": "TestSigner", "linkedEntity": "TestPrincipal",
                    "stateMachine": { "states": [{ "name": "idle", "isInitial": true }],
                        "transitions": [{ "from": "idle", "to": "idle", "event": "SIGN_IN", "effects": [
                            ["let", [["v", ["jws/verify", "@payload.token", [], { "issuer": EMULATOR_ISSUER, "audience": "test", "allowUnsigned": true }]]],
                                ["identity/sign-in", "@v"]]] }] }
                }]
            }]
        }))
        .expect("the signing program parses")
    }

    /// Sign the session `client` of `runtime` in as `subject`.
    fn sign_in(runtime: &Host, client: &str, subject: &str) {
        if runtime.kernel.trait_dispatch_mode("TestSigner").is_none() {
            runtime.register_schema(signing_program()).unwrap();
        }
        let header = orbital_core::codec::base64url_encode(br#"{"alg":"none","typ":"JWT"}"#);
        let claims = serde_json::json!({ "iss": EMULATOR_ISSUER, "aud": "test", "sub": subject, "exp": 4_000_000_000u64 });
        let token = format!("{header}.{}.", orbital_core::codec::base64url_encode(claims.to_string().as_bytes()));
        let payload: Value = serde_json::from_value(serde_json::json!({ "token": token })).unwrap();
        let _session = runtime.kernel.enter_session(client);
        runtime.kernel.dispatch("TestSigner", None, "SIGN_IN", payload).expect("the sign-in dispatches");
    }

    /// `options` with its `viewer` replaced by what now makes one: the session `clientId` names (else `viewer-<id>`) is
    /// signed in as the viewer.
    fn as_signed_in(runtime: &Host, mut options: serde_json::Value) -> serde_json::Value {
        let Some(viewer) = options.as_object_mut().and_then(|fields| fields.remove("viewer")) else {
            return options;
        };
        let Some(id) = viewer["id"].as_str() else {
            return options;
        };
        let client = options["clientId"].as_str().map_or_else(|| format!("viewer-{id}"), str::to_string);
        sign_in(runtime, &client, id);
        options["clientId"] = serde_json::json!(client);
        options
    }

    fn create_test_schema() -> orbital_core::OrbitalSchema {
        serde_json::from_str(
            r#"{
            "name": "TestApp",
            "version": "1.0.0",
            "typeContractVersion": 1,
            "orbitals": [
                {
                    "name": "User Onboarding",
                    "entity": {
                        "name": "User",
                        "collection": "users",
                        "fields": [
                            { "name": "id", "type": "string", "primaryKey": true },
                            { "name": "email", "type": "string", "required": true },
                            { "name": "name", "type": "string" },
                            { "name": "status", "type": "enum", "values": ["pending", "active"], "default": "pending" }
                        ]
                    },
                    "traits": [
                        {
                            "name": "UserManagement",
                            "linkedEntity": "User",
                            "stateMachine": {
                                "states": [
                                    { "name": "browsing", "isInitial": true },
                                    { "name": "registering" },
                                    { "name": "viewing" }
                                ],
                                "transitions": [
                                    {
                                        "from": "browsing",
                                        "to": "registering",
                                        "event": "REGISTER",
                                        "effects": []
                                    },
                                    {
                                        "from": "registering",
                                        "to": "viewing",
                                        "event": "SAVE",
                                        "guard": ["and", ["validate/required", "@payload.data.email"], ["validate/required", "@payload.data.name"]],
                                        "effects": [
                                            ["persist", "create", "User", "@payload.data"],
                                            ["emit", "USER_REGISTERED", { "userId": "@payload.data.id" }]
                                        ]
                                    },
                                    {
                                        "from": "registering",
                                        "to": "browsing",
                                        "event": "CANCEL",
                                        "effects": []
                                    }
                                ]
                            }
                        }
                    ]
                }
            ]
        }"#,
        )
        .unwrap()
    }

    #[test]
    fn test_runtime_creation() {
        let persistence = Arc::new(DriverAdapter::memory());
        let runtime = Host::new(persistence);

        let stats = runtime.stats();
        assert_eq!(stats.registered_traits, 0);
        assert_eq!(stats.active_instances, 0);
    }

    #[test]
    fn test_schema_registration() {
        let persistence = Arc::new(DriverAdapter::memory());
        let runtime = Host::new(persistence);

        let schema = create_test_schema();
        runtime.register_schema(schema).unwrap();

        let orbitals = runtime.list_orbitals();
        assert_eq!(orbitals.len(), 1);
        assert_eq!(orbitals[0].name, "User Onboarding");
        assert_eq!(orbitals[0].entity_name, "User");

        let stats = runtime.stats();
        assert_eq!(stats.registered_traits, 1);
    }

    #[test]
    fn test_process_event() {
        let persistence = Arc::new(DriverAdapter::memory());
        let runtime = Host::new(persistence);

        let schema = create_test_schema();
        runtime.register_schema(schema).unwrap();

        let result = runtime
            .process_event("UserManagement", "user-1", "REGISTER", Value::Null)
            .unwrap();

        assert!(result.executed);
        assert_eq!(result.previous_state, "browsing");
        assert_eq!(result.new_state, "registering");
    }

    #[test]
    fn test_guard_blocks_transition() {
        let persistence = Arc::new(DriverAdapter::memory());
        let runtime = Host::new(persistence);

        let schema = create_test_schema();
        runtime.register_schema(schema).unwrap();

        runtime
            .process_event("UserManagement", "user-1", "REGISTER", Value::Null)
            .unwrap();

        let result = runtime
            .process_event(
                "UserManagement",
                "user-1",
                "SAVE",
                Value::Object(Default::default()),
            )
            .unwrap();

        assert!(!result.executed);
        assert_eq!(result.new_state, "registering");
    }

    #[test]
    fn test_guard_allows_transition() {
        let persistence = Arc::new(DriverAdapter::memory());
        let runtime = Host::new(persistence);

        let schema = create_test_schema();
        runtime.register_schema(schema).unwrap();

        runtime
            .process_event("UserManagement", "user-1", "REGISTER", Value::Null)
            .unwrap();

        let mut inner_data = ValueMap::new();
        inner_data.insert("id".to_string().into(), Value::String(("user-1".to_string()).into()));
        inner_data.insert(
            "email".to_string().into(),
            Value::String(("test@example.com".to_string()).into()),
        );
        inner_data.insert("name".to_string().into(), Value::String(("Test User".to_string()).into()));

        let mut payload = ValueMap::new();
        payload.insert("data".to_string().into(), Value::Object(inner_data.into()));

        let result = runtime
            .process_event("UserManagement", "user-1", "SAVE", Value::Object(payload.into()))
            .unwrap();

        assert!(result.executed);
        assert_eq!(result.new_state, "viewing");
        assert!(result.effects_executed > 0);
    }

    #[test]
    fn test_get_state() {
        let persistence = Arc::new(DriverAdapter::memory());
        let runtime = Host::new(persistence);

        let schema = create_test_schema();
        runtime.register_schema(schema).unwrap();

        runtime
            .process_event("UserManagement", "user-1", "REGISTER", Value::Null)
            .unwrap();

        let state = runtime.get_state("UserManagement", "user-1");
        assert!(state.is_some());
        assert_eq!(state.unwrap().current_state, "registering");
    }

    #[test]
    fn an_orbital_detail_carries_the_entity_fields_and_none_of_its_rows() {
        let schema: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "App",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Main",
                "entity": { "name": "Row", "persistence": "persistent", "collection": "rows",
                    "fields": [{ "name": "id", "type": "string" }, { "name": "title", "type": "string" }],
                    "instances": [{ "id": "r1", "title": "ROW-MARKER-ONE" }, { "id": "r2", "title": "ROW-MARKER-TWO" }] },
                "traits": []
            }]
        }))
        .unwrap();
        let runtime = Host::new(Arc::new(DriverAdapter::memory()));
        runtime.register_schema(schema).unwrap();
        let detail = runtime.get_orbital("Main").unwrap();
        assert_eq!(detail.entity.name, "Row");
        assert_eq!(detail.entity.fields.len(), 2);
        let shown = format!("{detail:?}");
        assert!(
            !shown.contains("ROW-MARKER"),
            "the detail copied the entity's authored rows: {shown}"
        );
    }

    #[test]
    fn test_get_orbital() {
        let persistence = Arc::new(DriverAdapter::memory());
        let runtime = Host::new(persistence);

        let schema = create_test_schema();
        runtime.register_schema(schema).unwrap();

        let orbital = runtime.get_orbital("User Onboarding");
        assert!(orbital.is_some());

        let orbital = orbital.unwrap();
        let entity_name = orbital.entity.name.as_str();
        assert_eq!(entity_name, "User");
        assert_eq!(orbital.traits.len(), 1);
        assert_eq!(orbital.traits[0], "UserManagement");
    }

    fn dispatching_program() -> orbital_core::OrbitalSchema {
        program_whose_go_runs(serde_json::json!(["program/dispatch", "Notes.ADD_NOTE", { "title": "from the agent" }, { "emit": { "success": "DONE", "failure": "FAILED" } }]))
    }

    fn program_whose_go_runs(first: serde_json::Value) -> orbital_core::OrbitalSchema {
        let machine = |events: serde_json::Value, transitions: serde_json::Value| {
            serde_json::json!({ "states": [{ "name": "idle", "isInitial": true }, { "name": "done" }, { "name": "failed" }], "events": events, "transitions": transitions })
        };
        serde_json::from_value(serde_json::json!({
            "name": "Desk",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Desk",
                "entity": { "name": "Note", "persistence": "persistent", "access_waivers": { "read": "test" }, "fields": [
                    { "name": "id", "type": "string" }, { "name": "title", "type": "string" }, { "name": "got", "type": "string" }] },
                "traits": [{
                    "name": "Agent", "linkedEntity": "Note",
                    "stateMachine": machine(
                        serde_json::json!([{ "key": "GO", "name": "Go" }, { "key": "DONE", "name": "Done" }, { "key": "FAILED", "name": "Failed" }, { "key": "POSTED", "name": "Posted" }]),
                        serde_json::json!([
                            { "from": "idle", "to": "idle", "event": "GO", "effects": [
                                first,
                                ["program/dispatch", "Notes.ADD_NOTE", { "title": "second" }, { "emit": { "success": "POSTED", "failure": "POSTED" } }]] },
                            { "from": "idle", "to": "idle", "event": "POSTED", "effects": [["set", "@entity.title", "@payload.text"]] },
                            { "from": "done", "to": "done", "event": "POSTED", "effects": [["set", "@entity.title", "@payload.text"]] },
                            { "from": "failed", "to": "failed", "event": "POSTED", "effects": [["set", "@entity.title", "@payload.text"]] },
                            { "from": "idle", "to": "done", "event": "DONE", "effects": [["set", "@entity.got", "@payload.text"]] },
                            { "from": "idle", "to": "failed", "event": "FAILED", "effects": [["set", "@entity.got", "@payload.error"]] }]))
                }, {
                    "name": "Notes", "linkedEntity": "Note",
                    "stateMachine": machine(
                        serde_json::json!([{ "key": "ADD_NOTE", "name": "Add note", "input": true,
                            "payloadSchema": [{ "name": "title", "type": "string", "required": true }] }]),
                        serde_json::json!([{ "from": "idle", "to": "idle", "event": "ADD_NOTE", "guard": ["=", "@user.id", "u1"],
                            "effects": [["persist", "create", "Note", { "title": "@payload.title" }], ["emit", "NOTED", {}]] }]))
                }]
            }]
        }))
        .expect("the program parses")
    }

    /// `GO` as `viewer`, on a thread the test abandons after 20 s so a nested dispatch that deadlocks fails instead of hanging.
    fn go_as(viewer: &str) -> (Arc<Host>, String, String) {
        let (runtime, state, got, _) = go_as_with_title(viewer);
        (runtime, state, got)
    }

    fn go_as_with_title(viewer: &str) -> (Arc<Host>, String, String, String) {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(dispatching_program()).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let (inner, id) = (Arc::clone(&runtime), viewer.to_string());
        std::thread::spawn(move || {
            sign_in(&inner, &id, &id);
            let _session = inner.kernel.enter_session(&id);
            let ran = inner.dispatch_continuing("Agent", "default", "GO", Value::Null, None, None).map(|_| ());
            let _ = tx.send(ran.map(|()| inner.kernel.snapshot_trait("Agent", Some("default"))));
        });
        let snapshot = rx.recv_timeout(std::time::Duration::from_secs(20)).expect("the nested dispatch returns").unwrap();
        let entity = serde_json::to_value(&snapshot.entity).unwrap();
        let text = |field: &str| entity[field].as_str().unwrap_or_default().to_string();
        (runtime, snapshot.state, text("got"), text("title"))
    }

    #[test]
    #[ignore = "G-ORBITAL-SHELL-RUST-039: an unkeyed nested program/dispatch returns no reply text to its route"]
    fn a_second_dispatch_in_the_same_step_keeps_its_own_route_after_the_first_nested_one() {
        let (runtime, state, got, title) = go_as_with_title("u1");
        assert_eq!(state, "done", "{got}");
        let second: serde_json::Value = serde_json::from_str(&title).unwrap_or_default();
        assert_eq!(second["ok"], true, "the second dispatch answered its own route: {title}");
        let titles: Vec<serde_json::Value> =
            runtime.persistence.list("Note").unwrap().iter().map(|row| serde_json::to_value(row).unwrap()["title"].clone()).collect();
        assert!(titles.contains(&"from the agent".into()) && titles.contains(&"second".into()), "both inputs ran once: {titles:?}");
        assert_eq!(titles.iter().filter(|t| *t == "from the agent").count(), 1, "the first dispatch is not replayed when the step resumes");
    }

    #[test]
    #[ignore = "G-ORBITAL-SHELL-RUST-039: an unkeyed nested program/dispatch returns no reply text to its route"]
    fn a_program_dispatch_runs_the_declared_input_through_the_server_as_the_asking_viewer() {
        let (runtime, state, got) = go_as("u1");
        assert_eq!(state, "done", "{got}");
        let answer: serde_json::Value = serde_json::from_str(&got).unwrap();
        assert_eq!(answer["ok"], true, "{answer}");
        let rows = runtime.persistence.list("Note").unwrap();
        assert!(rows.iter().any(|row| serde_json::to_value(row).unwrap()["title"] == "from the agent"), "the input persisted its row");
    }

    #[test]
    #[ignore = "G-ORBITAL-SHELL-RUST-039: an unkeyed nested program/dispatch returns no reply text to its route"]
    fn control_the_nested_dispatch_keeps_the_asking_viewer_so_the_inputs_guard_refuses_another() {
        let (runtime, state, got) = go_as("u2");
        assert_eq!(state, "done", "a refused dispatch is still an answer a model can be told");
        let answer: serde_json::Value = serde_json::from_str(&got).unwrap();
        assert_eq!(answer["ok"], false, "{answer}");
        assert!(answer["error"].as_str().unwrap_or_default().starts_with("guard failed"), "{answer}");
        assert!(runtime.persistence.list("Note").unwrap().iter().all(|row| serde_json::to_value(row).unwrap()["title"] != "from the agent"));
    }

    fn live_program() -> orbital_core::OrbitalSchema {
        serde_json::from_value(serde_json::json!({
            "name": "Live",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Live",
                "entity": { "name": "Feed", "persistence": "runtime", "fields": [
                    { "name": "id", "type": "string" }, { "name": "seen", "type": "string", "default": "" }, { "name": "error", "type": "string" }] },
                "traits": [{
                    "name": "Watch", "linkedEntity": "Feed",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "OPEN", "name": "Open" }, { "key": "OPEN_UNKEYED", "name": "Open unkeyed" }, { "key": "STOP", "name": "Stop" },
                            { "key": "ITEM", "name": "Item" }, { "key": "LIVE_FAILED", "name": "Live failed" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "OPEN", "effects": [["program/live", {}, { "key": "feed", "emit": { "onMessage": "ITEM", "failure": "LIVE_FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "OPEN_UNKEYED", "effects": [["program/live", {}, { "emit": { "onMessage": "ITEM", "failure": "LIVE_FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "STOP", "effects": [["cancel-call", "feed"]] },
                            { "from": "idle", "to": "idle", "event": "ITEM", "effects": [["set", "@entity.seen", ["str/concat", "@entity.seen", "@payload.event"]]] },
                            { "from": "idle", "to": "idle", "event": "LIVE_FAILED", "effects": [["set", "@entity.error", "@payload.error"]] }
                        ]
                    }
                }]
            }]
        }))
        .expect("the program parses")
    }

    fn broadcast(runtime: &Host, event: &str) {
        runtime.broadcast_live(LiveBroadcastItem {
            event: event.to_string(),
            payload: None,
            source: orbital_core::effects::BusEventSource::from_trait("Elsewhere".to_string()),
            origin_client_id: None,
            target: orbital_core::effects::LiveTarget::Peers,
            dispatch: None,
        });
    }

    fn feed(runtime: &Host, until: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let entity = serde_json::to_value(&runtime.kernel.snapshot_trait("Watch", Some("default")).entity).unwrap();
            if until(&entity) || std::time::Instant::now() > deadline {
                return entity;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[test]
    fn a_keyed_live_subscribe_receives_each_broadcast_in_order_until_cancelled() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(live_program()).unwrap();
        runtime.dispatch_continuing("Watch", "default", "OPEN", Value::Null, None, None).unwrap();
        broadcast(&runtime, "E1");
        broadcast(&runtime, "E2");
        let entity = feed(&runtime, |e| e["seen"] == "E1E2");
        assert_eq!(entity["seen"], "E1E2", "both broadcasts arrived, in order: {entity}");
        runtime.dispatch_continuing("Watch", "default", "STOP", Value::Null, None, None).unwrap();
        broadcast(&runtime, "E3");
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(feed(&runtime, |_| true)["seen"], "E1E2", "cancel-call stopped the stream");
    }

    fn trace_poll(source: &mut orbital_core::runtime::native_host::SubstrateSource) -> serde_json::Value {
        match source() {
            orbital_core::runtime::os_host::completion::StreamPoll::Item(item) => serde_json::to_value(item).unwrap(),
            _ => panic!("expected trace item"),
        }
    }

    fn trace_source(runtime: &Host, options: serde_json::Value) -> orbital_core::runtime::native_host::SubstrateSource {
        let Value::Object(mut options) = serde_json::from_value(options).unwrap() else { panic!("options") };
        options.insert("kind".into(), Value::string("trace"));
        runtime.substrate.live_source("program/live", &[Value::Object(options)]).expect("shared trace source")
    }

    #[test]
    fn trace_stream_selects_the_instance_traits_and_seed_frame_without_losing_history() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(live_program()).unwrap();
        let mut source = trace_source(&runtime, serde_json::json!({ "traits": ["Watch"], "seedEntityId": "selected" }));
        let attached = trace_poll(&mut source);
        assert_eq!(attached["type"], "trace-attached");
        assert_eq!(attached["programVersion"].as_f64(), Some(1.0));
        runtime.dispatch_continuing("Watch", "other", "STOP", Value::Null, None, None).unwrap();
        assert!(matches!(source(), orbital_core::runtime::os_host::completion::StreamPoll::Idle));
        for _ in 0..205 {
            runtime.dispatch_continuing("Watch", "selected", "STOP", Value::Null, None, None).unwrap();
        }
        for expected in 1..206 {
            let item = trace_poll(&mut source);
            assert_eq!(item["type"], "trace");
            assert_eq!(item["instance"], attached["instance"]);
            assert_eq!(item["dispatch"]["seq"].as_f64(), Some(f64::from(expected)));
            assert_eq!(item["dispatch"]["seedEntityId"], "selected");
            assert_eq!(item["dispatch"]["outcomes"][0]["trait"], "Watch");
        }
        assert_eq!(runtime.trace().len(), TRACE_CAPACITY, "snapshot retention does not truncate the transport");
        let other = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let mut wrong = trace_source(&other, serde_json::json!({ "traits": ["Watch"], "instance": attached["instance"] }));
        assert_eq!(trace_poll(&mut wrong)["type"], "trace-disconnected");
        drop(source);
        assert!(!runtime.trace.enabled(), "disconnect removes subscriber and ends its recording lease");
    }

    #[test]
    fn trace_stream_records_actual_completion_hops_and_ends_when_the_host_stops() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(live_program()).unwrap();
        let mut source = trace_source(&runtime, serde_json::json!({ "traits": ["Watch"] }));
        trace_poll(&mut source);
        runtime.dispatch_continuing("Watch", "default", "OPEN", Value::Null, None, None).unwrap();
        let open = trace_poll(&mut source);
        assert_eq!(open["dispatch"]["outcomes"][0]["event"], "OPEN");
        assert_eq!(open["dispatch"]["asks"].as_array().unwrap().len(), 1, "the opening dispatch keeps its own I/O ask");
        broadcast(&runtime, "E1");
        assert_eq!(feed(&runtime, |e| e["seen"] == "E1")["seen"], "E1");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let completion = loop {
            if let orbital_core::runtime::os_host::completion::StreamPoll::Item(item) = source() {
                break serde_json::to_value(item).unwrap();
            }
            assert!(std::time::Instant::now() < deadline, "completion trace was not recorded");
            std::thread::yield_now();
        };
        assert_eq!(completion["dispatch"]["outcomes"][0]["event"], "ITEM");
        assert_eq!(completion["dispatch"]["outcomes"][0]["executed"], true);
        assert_eq!(completion["dispatch"]["outcomes"][0]["effects"][0]["status"], "Executed");
        runtime.cancel_open_calls();
        drop(runtime);
        assert_eq!(trace_poll(&mut source)["type"], "trace-disconnected");
        assert!(matches!(source(), orbital_core::runtime::os_host::completion::StreamPoll::Ended));
    }

    #[test]
    fn trace_stream_preserves_guard_refusals_and_effect_failures() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let mut schema = serde_json::to_value(live_program()).unwrap();
        let machine = &mut schema["orbitals"][0]["traits"][0]["stateMachine"];
        machine["events"].as_array_mut().unwrap().extend([
            serde_json::json!({ "key": "REFUSED", "name": "Refused" }),
            serde_json::json!({ "key": "FAIL", "name": "Fail" }),
        ]);
        machine["transitions"].as_array_mut().unwrap().extend([
            serde_json::json!({ "from": "idle", "to": "idle", "event": "REFUSED", "guard": ["=", 1, 2] }),
            serde_json::json!({ "from": "idle", "to": "idle", "event": "FAIL", "effects": [["set", "@entity.seen", ["array/append", null, 1]], ["set", "@entity.seen", "never"]] }),
        ]);
        runtime.register_schema(serde_json::from_value(schema).unwrap()).unwrap();
        let mut source = trace_source(&runtime, serde_json::json!({ "traits": ["Watch"] }));
        trace_poll(&mut source);
        runtime.dispatch_continuing("Watch", "default", "REFUSED", Value::Null, None, None).unwrap();
        let refused = trace_poll(&mut source);
        assert_eq!(refused["dispatch"]["outcomes"][0]["executed"], false);
        assert_eq!(refused["dispatch"]["outcomes"][0]["firedArm"], serde_json::Value::Null);
        assert_eq!(refused["dispatch"]["outcomes"][0]["guardResults"][0]["passed"], false);
        runtime.dispatch_continuing("Watch", "default", "FAIL", Value::Null, None, None).unwrap();
        let failed = trace_poll(&mut source);
        let outcome = &failed["dispatch"]["outcomes"][0];
        assert_eq!(outcome["executed"], true, "transition commit survives failed effects");
        assert!(outcome["failure"].is_string(), "{outcome}");
        assert_eq!(outcome["effects"][0]["status"], "Failed");
        assert_eq!(outcome["effects"][1]["status"], "Skipped");
    }

    #[test]
    fn trace_stream_records_under_the_kernel_lane_before_a_delayed_host_finish() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let mut schema = serde_json::to_value(live_program()).unwrap();
        schema["orbitals"][0]["id"] = serde_json::json!("o-trace");
        schema["orbitals"][0]["traits"][0]["id"] = serde_json::json!("t-watch");
        schema["orbitals"][0]["traits"][0]["sourceDeclaration"] = serde_json::json!({ "traitId": "t-generic", "fingerprint": "original-fingerprint" });
        schema["orbitals"][0]["traits"][0]["stateMachine"]["events"].as_array_mut().unwrap().push(serde_json::json!({ "key": "NOOP", "name": "Noop" }));
        schema["orbitals"][0]["traits"][0]["stateMachine"]["transitions"].as_array_mut().unwrap().push(serde_json::json!({ "from": "idle", "to": "idle", "event": "NOOP" }));
        runtime.register_schema(serde_json::from_value(schema).unwrap()).unwrap();
        let mut source = trace_source(&runtime, serde_json::json!({ "traits": ["Watch"] }));
        trace_poll(&mut source);
        let delayed = runtime.kernel.start_dispatch("Watch", Some("A"), "NOOP", Value::Null, None, None);
        assert!(matches!(&delayed, Ok(orbital_core::runtime::resumable::Progress::Done(_))), "NOOP must complete before another dispatch enters the lane");
        runtime.dispatch_continuing("Watch", "B", "NOOP", Value::Null, None, None).unwrap();
        runtime.finish(delayed).unwrap();
        let first = trace_poll(&mut source);
        let second = trace_poll(&mut source);
        assert_eq!(first["dispatch"]["seq"].as_f64(), Some(0.0));
        assert_eq!(second["dispatch"]["seq"].as_f64(), Some(1.0));
        assert_eq!(first["dispatch"]["outcomes"][0]["entityId"], "A");
        assert_eq!(second["dispatch"]["outcomes"][0]["stateEntityId"], "B");
        let outcome = &first["dispatch"]["outcomes"][0];
        assert_eq!(outcome["traitId"], "t-watch");
        assert_eq!(outcome["orbitalId"], "o-trace");
        assert_eq!(outcome["sourceTraitId"], "t-generic");
        assert!(outcome["sourcePath"].is_null());
        assert_eq!(outcome["sourceDeclarationFingerprint"], "original-fingerprint");
        assert!(outcome["definitionFingerprint"].as_str().is_some_and(|s| s.len() == 64));
        assert_eq!(outcome["observedState"], "idle");
        assert_eq!(outcome["transitionIndex"].as_f64(), Some(5.0));
        assert!(matches!(source(), orbital_core::runtime::os_host::completion::StreamPoll::Idle), "finishing the host does not record a dispatch twice");
    }

    #[test]
    fn trace_stream_observes_held_state_after_cascade_instead_of_parent_destination() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let mut schema = serde_json::to_value(live_program()).unwrap();
        schema["orbitals"][0]["traits"][0]["stateMachine"] = serde_json::json!({
            "states": [{ "name": "idle", "isInitial": true }, { "name": "working" }, { "name": "done" }],
            "events": [{ "key": "GO", "name": "Go" }, { "key": "ADVANCE", "name": "Advance", "input": true }],
            "transitions": [
                { "from": "idle", "to": "working", "event": "GO", "effects": [["emit", "ADVANCE", {}]] },
                { "from": "working", "to": "done", "event": "ADVANCE" }
            ]
        });
        runtime.register_schema(serde_json::from_value(schema).unwrap()).unwrap();
        let mut source = trace_source(&runtime, serde_json::json!({ "traits": ["Watch"] }));
        trace_poll(&mut source);
        let (tx, rx) = std::sync::mpsc::channel();
        let inner = Arc::clone(&runtime);
        std::thread::spawn(move || { let _ = tx.send(inner.dispatch_continuing("Watch", "default", "GO", Value::Null, None, None)); });
        rx.recv_timeout(std::time::Duration::from_secs(20)).expect("cascaded dispatch must finish without deadlocking").unwrap();
        let trace = trace_poll(&mut source);
        let outcomes = trace["dispatch"]["outcomes"].as_array().unwrap();
        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0]["to"], "working");
        assert_eq!(outcomes[1]["to"], "done");
        assert_eq!(outcomes[0]["observedState"], "done");
        assert_eq!(outcomes[0]["stateEntityId"], "default");
    }

    #[test]
    fn trace_stream_requires_explicit_selection_and_ignores_unselected_traits() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(live_program()).unwrap();
        let mut invalid = trace_source(&runtime, serde_json::json!({}));
        assert_eq!(trace_poll(&mut invalid)["type"], "trace-disconnected");
        let mut source = trace_source(&runtime, serde_json::json!({ "traits": ["Other"] }));
        trace_poll(&mut source);
        runtime.dispatch_continuing("Watch", "default", "STOP", Value::Null, None, None).unwrap();
        assert!(matches!(source(), orbital_core::runtime::os_host::completion::StreamPoll::Idle));
    }

    #[test]
    fn control_an_unkeyed_live_subscribe_is_refused() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(live_program()).unwrap();
        runtime.dispatch_continuing("Watch", "default", "OPEN_UNKEYED", Value::Null, None, None).unwrap();
        broadcast(&runtime, "E1");
        let entity = feed(&runtime, |e| e["error"].is_string());
        assert!(entity["error"].as_str().unwrap_or_default().contains("give it a key"), "{entity}");
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(feed(&runtime, |_| true)["seen"].as_str().unwrap_or_default(), "", "no stream was opened");
    }

    #[test]
    fn edge_a_runtime_that_is_not_shared_names_why_it_cannot_dispatch() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory()));
        let failure = orbital_core::effects::executor::SubstrateHandler::run(
            runtime.substrate.as_ref(),
            "program/dispatch",
            &[Value::string("Desk.Notes.ADD_NOTE"), Value::Null],
        )
        .unwrap_err();
        assert!(failure.message.contains("not shared"), "{}", failure.message);
        let other = orbital_core::effects::executor::SubstrateHandler::run(runtime.substrate.as_ref(), "program/read", &[]).unwrap_err();
        assert_eq!(other.message, "no substrate host configured for program/read", "control: other ops keep the kernel's message");
    }

    fn ticking_program() -> orbital_core::OrbitalSchema {
        ticking_program_every(1000)
    }

    fn ticking_program_every(interval: u64) -> orbital_core::OrbitalSchema {
        serde_json::from_value(serde_json::json!({
            "name": "Clock",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Clock",
                "entity": { "name": "Beat", "persistence": "persistent", "collection": "beats", "fields": [{ "name": "id", "type": "string" }, { "name": "n", "type": "number" }] },
                "traits": [{
                    "name": "Pulse", "linkedEntity": "Beat",
                    "stateMachine": { "states": [{ "name": "idle", "isInitial": true }], "events": [{ "key": "INIT", "name": "Init" }], "transitions": [{ "from": "idle", "to": "idle", "event": "INIT" }] },
                    "ticks": [
                        { "name": "beat", "interval": interval, "runsInBackground": true, "effects": [["persist", "create", "Beat", { "n": 1 }]] },
                        { "name": "screen", "interval": interval, "effects": [["persist", "create", "Beat", { "n": 2 }]] }
                    ]
                }]
            }]
        }))
        .expect("the program parses")
    }

    fn noon() -> orbital_core::Calendar {
        orbital_core::Calendar { minute: 0, hour: 12, day: 1, month: 1, weekday: 1, minute_key: 1 }
    }

    #[test]
    fn a_tick_pass_runs_the_due_background_tick_and_never_a_client_mounted_one() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(ticking_program()).unwrap();
        runtime.persistence.create("Beat", &serde_json::from_value(serde_json::json!({ "n": 0 })).unwrap()).unwrap();
        assert!(runtime.tick_pass(0, noon()).is_empty(), "the first pass only captures the clock");
        assert!(runtime.tick_pass(400, noon()).is_empty(), "control: not yet due");
        assert_eq!(runtime.tick_pass(1000, noon()), [("Pulse".to_string(), "beat".to_string())], "control: `screen` is a client's tick");
        let ns: Vec<f64> =
            runtime.persistence.list("Beat").unwrap().iter().filter_map(|r| serde_json::to_value(r).unwrap()["n"].as_f64()).collect();
        assert_eq!(ns.iter().filter(|n| **n == 1.0).count(), 1, "{ns:?}");
        assert!(!ns.contains(&2.0), "{ns:?}");
    }

    #[test]
    fn the_embedded_servers_tick_pass_selects_what_the_native_hosts_does() {
        let host = Host::new(Arc::new(DriverAdapter::memory())).shared();
        host.register_schema(ticking_program()).unwrap();
        let server = orbital_core::embed::EmbeddedServer::new(ticking_program()).unwrap();
        let due = |now: u64| -> Vec<(String, String)> { server.tick_pass(now).into_iter().flat_map(|(_, fired)| fired).map(|f| (f.trait_name, f.tick)).collect() };
        for now in [0, 400] {
            assert!(host.tick_pass(now, noon()).is_empty() && due(now).is_empty(), "now {now}");
        }
        assert_eq!(
            due(1000),
            host.tick_pass(1000, noon()),
            "the same background tick fires on both, and `screen` (a client's tick) on neither"
        );
        server.mount_client("tab-1", "Clock", vec!["Pulse".to_string()]);
        assert_eq!(
            due(2000),
            [("Pulse".to_string(), "beat".to_string()), ("Pulse".to_string(), "screen".to_string())],
            "control: a mounted trait's client tick is the server's too"
        );
        assert_eq!(host.tick_pass(2000, noon()), [("Pulse".to_string(), "beat".to_string())], "control: the native host tracks no mounts");
        server.tick_pause();
        assert!(server.tick_paused() && due(9000).is_empty());
        server.tick_resume();
        assert!(!server.tick_paused());
    }

    fn ask(runtime: &Host, op: &str, args: &[serde_json::Value]) -> Result<serde_json::Value, String> {
        let args: Vec<Value> = args.iter().map(|a| serde_json::from_value(a.clone()).unwrap()).collect();
        orbital_core::effects::executor::SubstrateHandler::run(runtime.substrate.as_ref(), op, &args)
            .map(|v| orbital_core::json_value::value_to_json(&v))
            .map_err(|f| f.message)
    }

    #[test]
    fn a_program_tick_runs_one_pass_and_answers_the_background_ticks_that_ran() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(ticking_program_every(10)).unwrap();
        runtime.persistence.create("Beat", &serde_json::from_value(serde_json::json!({ "n": 0 })).unwrap()).unwrap();
        assert_eq!(ask(&runtime, "program/tick", &[]).unwrap(), serde_json::json!({ "ran": [] }), "the first pass only captures the clock");
        std::thread::sleep(std::time::Duration::from_millis(40));
        assert_eq!(
            ask(&runtime, "program/tick", &[]).unwrap(),
            serde_json::json!({ "ran": [{ "trait": "Pulse", "tick": "beat" }] }),
            "control: `screen` is a client's tick and never runs on a pass"
        );
        let ns: Vec<f64> =
            runtime.persistence.list("Beat").unwrap().iter().filter_map(|r| serde_json::to_value(r).unwrap()["n"].as_f64()).collect();
        assert_eq!(ns.iter().filter(|n| **n == 1.0).count(), 1, "the pass ran the background tick once over its row: {ns:?}");
        assert!(!ns.contains(&2.0), "{ns:?}");
        assert!(ask(&runtime, "program/tick", &[serde_json::json!(5)]).unwrap_err().contains("takes no arguments"), "edge: a stray argument");
    }

    fn owned_notes(runtime: &Host) {
        let schema: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "ResolveApp",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Notes",
                "entity": { "name": "Note", "persistence": "persistent", "read_policy": ["=", ["object/get", "@entity", "ownerId"], "@user.id"],
                    "fields": [{ "name": "id", "type": "string", "primaryKey": true }, { "name": "ownerId", "type": "string" }] },
                "traits": [{ "name": "NoteBrowse", "linkedEntity": "Note",
                    "stateMachine": { "states": [{ "name": "idle", "isInitial": true }], "transitions": [{ "from": "idle", "to": "idle", "event": "LIST" }] } }],
                "pages": []
            }]
        }))
        .unwrap();
        runtime.register_schema(schema).unwrap();
        for (id, owner) in [("a1", "alice"), ("a2", "alice"), ("b1", "bob")] {
            runtime.persistence.create("Note", &serde_json::from_value(serde_json::json!({ "id": id, "ownerId": owner })).unwrap()).unwrap();
        }
    }

    #[test]
    fn a_program_resolve_rows_answers_only_the_rows_the_asking_sessions_identity_may_read() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        owned_notes(&runtime);
        sign_in(&runtime, "s-alice", "alice");
        sign_in(&runtime, "s-bob", "bob");
        let reference = serde_json::json!({ "entityType": "Note", "ids": ["a1", "b1", "a2"], "total": 3, "query": {} });
        let ids = |page: serde_json::Value| -> Vec<String> {
            page["rows"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap().to_string()).collect()
        };
        let alice = {
            let _session = runtime.kernel.enter_session("s-alice");
            ask(&runtime, "program/resolve-rows", &[reference.clone()]).unwrap()
        };
        assert_eq!(ids(alice.clone()), ["a1", "a2"], "the asking session's identity: {alice}");
        assert_eq!(alice["total"], 2);
        let bob = {
            let _session = runtime.kernel.enter_session("s-bob");
            ask(&runtime, "program/resolve-rows", &[reference.clone()]).unwrap()
        };
        assert_eq!(ids(bob), ["b1"], "control: another session reads its own rows");
        let anonymous = ask(&runtime, "program/resolve-rows", &[reference.clone()]).unwrap();
        assert_eq!(ids(anonymous), Vec::<String>::new(), "control: an unbound session is the anonymous viewer and reads no owned row");
        assert!(ask(&runtime, "program/resolve-rows", &[serde_json::json!({ "entityType": "Note" })]).unwrap_err().contains("reference"), "edge: a malformed reference");
        let malformed: Vec<Value> = [serde_json::json!({ "entityType": "Note" })].iter().map(|a| serde_json::from_value(a.clone()).unwrap()).collect();
        let refused = orbital_core::effects::executor::SubstrateHandler::run(runtime.substrate.as_ref(), "program/resolve-rows", &malformed).unwrap_err();
        let error = orbital_core::json_value::value_to_json(&refused.errors[0]);
        assert_eq!((error["code"].as_str(), error["status"].as_u64()), (Some("INVALID_REQUEST"), Some(400)), "a malformed reference is the caller's 400: {error}");
        let unasked: Vec<Value> = [serde_json::json!({ "entityType": "Nothing", "ids": [], "total": 0, "query": {} })].iter().map(|a| serde_json::from_value(a.clone()).unwrap()).collect();
        if let Err(failure) = orbital_core::effects::executor::SubstrateHandler::run(runtime.substrate.as_ref(), "program/resolve-rows", &unasked) {
            assert!(failure.errors.is_empty(), "control: a failure that is not the reference's shape names no status: {:?}", failure.errors);
        }
        let named = |client: serde_json::Value| ask(&runtime, "program/resolve-rows", &[reference.clone(), serde_json::json!({ "clientId": client })]);
        assert_eq!(ids(named(serde_json::json!("s-alice")).unwrap()), ["a1", "a2"], "clientId names the session whose identity resolves, from any thread");
        assert_eq!(ids(named(serde_json::json!("s-bob")).unwrap()), ["b1"], "control: another clientId reads its own rows");
        {
            let _session = runtime.kernel.enter_session("s-bob");
            assert_eq!(ids(named(serde_json::json!("s-alice")).unwrap()), ["a1", "a2"], "control: clientId wins over the asking thread's session");
        }
        assert!(named(serde_json::json!(7)).unwrap_err().contains("clientId"), "edge: clientId is text");
        assert!(ask(&runtime, "program/resolve-rows", &[reference.clone(), serde_json::json!({ "viewer": { "id": "alice" } })]).unwrap_err().contains("clientId"), "edge: a viewer option is refused");
        assert!(ask(&runtime, "program/resolve-rows", &[reference, serde_json::json!({ "id": "alice" })]).unwrap_err().contains("clientId"), "edge: a viewer argument is no longer taken");
    }

    fn hooked_program() -> orbital_core::OrbitalSchema {
        serde_json::from_value(serde_json::json!({
            "name": "Hooked",
            "inbound": ["hook"],
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Hooked",
                "entity": { "name": "Run", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "status", "type": "number" }, { "name": "body", "type": "string" }, { "name": "got", "type": "string" }] },
                "traits": [{
                    "name": "Caller", "linkedEntity": "Run",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }, { "name": "done" }, { "name": "failed" }],
                        "events": [{ "key": "GO", "name": "Go" }, { "key": "DONE", "name": "Done" }, { "key": "FAILED", "name": "Failed" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "GO", "effects": [["program/dispatch", { "inbound": "@payload.hook", "body": "{}", "headers": {} }, { "emit": { "success": "DONE", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "done", "event": "DONE", "effects": [["set", "@entity.status", "@payload.result.status"], ["set", "@entity.body", "@payload.result.body"]] },
                            { "from": "idle", "to": "failed", "event": "FAILED", "effects": [["set", "@entity.got", "@payload.error"]] }]
                    }
                }, {
                    "name": "Hook", "linkedEntity": "Run",
                    "emits": [{ "event": "PING_ANSWERED" }],
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "PING", "inbound": { "name": "hook", "answers": "PING_ANSWERED" } }],
                        "transitions": [{ "from": "idle", "to": "idle", "event": "PING", "effects": [["emit", "PING_ANSWERED", { "status": 201, "body": "pong" }]] }]
                    }
                }]
            }]
        }))
        .expect("the program parses")
    }

    /// What `Caller` recorded: the delivery's `{ status, body }` (the completion's `result`) once done, else the failure.
    fn answered(snapshot: &orbital_core::TraitSnapshot) -> serde_json::Value {
        let row = serde_json::to_value(&snapshot.entity).unwrap();
        if snapshot.state == "done" {
            serde_json::json!({ "status": row["status"], "body": row["body"] })
        } else {
            row["got"].clone()
        }
    }

    fn go_hook(hook: &str) -> (String, serde_json::Value) {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(hooked_program()).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let (inner, hook) = (Arc::clone(&runtime), hook.to_string());
        std::thread::spawn(move || {
            let payload: Value = serde_json::from_value(serde_json::json!({ "hook": hook })).unwrap();
            let _ = tx.send(inner.dispatch_continuing("Caller", "default", "GO", payload, None, None).map(|_| ()));
        });
        rx.recv_timeout(std::time::Duration::from_secs(20)).expect("the nested delivery returns").unwrap();
        let snapshot = runtime.kernel.snapshot_trait("Caller", Some("default"));
        (snapshot.state.clone(), answered(&snapshot))
    }

    #[test]
    fn an_inbound_program_dispatch_delivers_to_the_handler_and_answers_its_status_and_body() {
        let (state, answer) = go_hook("hook");
        assert_eq!(state, "done", "{answer}");
        assert_eq!(answer, serde_json::json!({ "status": 201, "body": "pong" }));
        let (state, refused) = go_hook("nope");
        assert_eq!(state, "done", "control: an undeclared hook is the kernel's refusal, answered: {refused}");
        assert_eq!(refused["status"], 404, "{refused}");
    }

    /// [`hooked_program`] asking with a `key`, plus a `silent` hook whose handler answers nothing.
    fn keyed_hooked_program() -> orbital_core::OrbitalSchema {
        let mut program = serde_json::to_value(hooked_program()).unwrap();
        program["inbound"] = serde_json::json!(["hook", "silent"]);
        program["orbitals"][0]["traits"][0]["stateMachine"]["transitions"][0]["effects"][0][2]["key"] = serde_json::json!("hook-1");
        program["orbitals"][0]["traits"].as_array_mut().unwrap().push(serde_json::json!({
            "name": "Silent", "linkedEntity": "Run",
            "emits": [{ "event": "HUSH_ANSWERED" }],
            "stateMachine": {
                "states": [{ "name": "idle", "isInitial": true }],
                "events": [{ "key": "HUSH", "inbound": { "name": "silent", "answers": "HUSH_ANSWERED" } }],
                "transitions": [{ "from": "idle", "to": "idle", "event": "HUSH", "effects": [] }]
            }
        }));
        serde_json::from_value(program).expect("the keyed program parses")
    }

    /// `GO` asks `program/dispatch { inbound: hook }` with a key: the delivery runs after `GO` commits, as a dispatch
    /// of its own, and its answer arrives as `DONE` (the std HTTP server's `POST /api/hooks/:name`).
    fn go_keyed_hook(hook: &str) -> (String, serde_json::Value) {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(keyed_hooked_program()).unwrap();
        let payload: Value = serde_json::from_value(serde_json::json!({ "hook": hook })).unwrap();
        runtime.dispatch_continuing("Caller", "default", "GO", payload, None, None).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let snapshot = runtime.kernel.snapshot_trait("Caller", Some("default"));
            if snapshot.state != "idle" {
                return (snapshot.state.clone(), answered(&snapshot));
            }
            assert!(std::time::Instant::now() < deadline, "the keyed delivery never completed");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[test]
    fn a_keyed_inbound_program_dispatch_answers_the_handlers_status_and_body() {
        let (state, answer) = go_keyed_hook("hook");
        assert_eq!((state.as_str(), answer), ("done", serde_json::json!({ "status": 201, "body": "pong" })));
    }

    #[test]
    fn control_a_keyed_inbound_dispatch_to_an_undeclared_hook_answers_404() {
        let (state, refused) = go_keyed_hook("nope");
        assert_eq!(state, "done", "the refusal is an answer, not a failure: {refused}");
        assert_eq!(refused, serde_json::json!({ "status": 404, "body": "{\"error\":\"Unknown hook provider: nope\"}" }));
    }

    #[test]
    fn control_a_keyed_inbound_dispatch_whose_handler_never_answers_is_500_inbound_unanswered() {
        let (state, unanswered) = go_keyed_hook("silent");
        assert_eq!(state, "done", "{unanswered}");
        assert_eq!(unanswered, serde_json::json!({ "status": 500, "body": "{\"error\":\"inbound-unanswered\"}" }));
    }

    #[test]
    fn a_program_inputs_trace_ask_answers_the_last_steps_this_host_traced() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(ticking_program()).unwrap();
        runtime.set_trace(true);
        runtime.dispatch_continuing("Pulse", "default", "INIT", Value::Null, None, None).unwrap();
        runtime.dispatch_continuing("Pulse", "default", "INIT", Value::Null, None, None).unwrap();
        let last = ask(&runtime, "program/inputs", &[serde_json::json!({ "trace": 1 })]).unwrap();
        assert_eq!(last.as_array().map(Vec::len), Some(1), "{last}");
        let newest = serde_json::to_value(&runtime.trace()[1]).unwrap();
        assert_eq!(last[0]["seq"].as_f64(), newest["seq"].as_f64(), "the newest step: {last}");
        assert_eq!(ask(&runtime, "program/inputs", &[serde_json::json!({ "trace": 0 })]).unwrap(), serde_json::json!([]), "control");
        assert!(ask(&runtime, "program/inputs", &[serde_json::json!({ "stats": true })]).unwrap_err().contains("read by the kernel"), "edge");
    }

    #[test]
    fn a_program_inputs_ask_answers_the_declared_inputs_the_dispatch_route_lists() {
        let runtime = desk();
        let answer = ask(&runtime, "program/inputs", &[serde_json::json!({})]).unwrap();
        let addresses: Vec<&str> = answer["inputs"].as_array().unwrap().iter().filter_map(|i| i["address"].as_str()).collect();
        assert_eq!(addresses, ["Desk.Notes.ADD_NOTE"], "{answer}");
        assert_eq!(answer["entities"], serde_json::json!(["Note"]));
        assert!(answer.get("trace").is_none(), "control: no trace unless asked");
        let bare = ask(&runtime, "program/inputs", &[]).unwrap();
        assert_eq!(bare["inputs"], answer["inputs"], "control: no options is the same reflection");
    }

    /// `desk()` plus a `[client]` trait that listens to `Notes.NOTED`, so the client view keeps the emit (EXEC-DIST-8).
    fn desk_with_a_client_listener() -> Arc<Host> {
        let mut program = serde_json::to_value(dispatching_program()).unwrap();
        program["orbitals"][0]["traits"].as_array_mut().unwrap().push(serde_json::json!({
            "name": "Wall", "linkedEntity": "Note", "dispatch": "client",
            "listens": [{ "event": "NOTED", "triggers": "SEEN_NOTE", "source": { "kind": "trait", "trait": "Notes" } }],
            "stateMachine": {
                "states": [{ "name": "idle", "isInitial": true }],
                "events": [{ "key": "SEEN_NOTE", "name": "Seen note" }],
                "transitions": [{ "from": "idle", "to": "idle", "event": "SEEN_NOTE", "effects": [] }]
            }
        }));
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(serde_json::from_value(program).unwrap()).unwrap();
        runtime
    }

    fn desk() -> Arc<Host> {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(dispatching_program()).unwrap();
        runtime
    }

    #[test]
    fn dropping_the_host_frees_its_kernel_and_everything_the_kernel_holds() {
        let runtime = desk();
        let kernel = Arc::downgrade(&runtime.kernel);
        drop(runtime);
        assert!(kernel.upgrade().is_none(), "the completion sink must not keep the kernel alive");
    }

    #[test]
    fn control_a_kernel_still_referenced_outlives_its_host() {
        let runtime = desk();
        let kernel = Arc::clone(&runtime.kernel);
        drop(runtime);
        assert_eq!(Arc::strong_count(&kernel), 1);
    }

    fn dispatch_request(runtime: &Host, options: serde_json::Value) -> Result<serde_json::Value, (String, serde_json::Value)> {
        let address = format!(
            "{}.{}.{}",
            options["orbital"].as_str().unwrap_or_default(),
            options["targetTrait"].as_str().unwrap_or_default(),
            options["event"].as_str().unwrap_or_default()
        );
        let options = as_signed_in(runtime, options);
        let args: Vec<Value> = [serde_json::json!(address), serde_json::json!({ "title": "via request" }), options]
            .iter()
            .map(|a| serde_json::from_value(a.clone()).unwrap())
            .collect();
        orbital_core::effects::executor::SubstrateHandler::run(runtime.substrate.as_ref(), "program/dispatch", &args)
            .map(|v| orbital_core::json_value::value_to_json(&v))
            .map_err(|f| (f.message, serde_json::Value::Array(f.errors.iter().map(orbital_core::json_value::value_to_json).collect())))
    }

    fn note_titles(runtime: &Host) -> Vec<String> {
        runtime.persistence.list("Note").unwrap().iter().filter_map(|r| serde_json::to_value(r).unwrap()["title"].as_str().map(str::to_string)).collect()
    }

    #[test]
    fn a_wire_request_runs_as_its_viewer_and_answers_the_event_response() {
        let runtime = desk();
        let answer = dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "targetTrait": "Notes", "event": "ADD_NOTE", "viewer": { "id": "u1" } })).unwrap();
        assert_eq!(answer["success"], true, "{answer}");
        assert!(answer.get("emittedEvents").is_some() && answer.get("states").is_some(), "the whole EventResponse: {answer}");
        assert_eq!(note_titles(&runtime), ["via request"]);
        let refused = dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "targetTrait": "Notes", "event": "ADD_NOTE", "viewer": { "id": "u2" } })).unwrap();
        assert_eq!(refused["guardFailed"], "Notes.ADD_NOTE", "control: another viewer meets the guard: {refused}");
        assert_eq!(note_titles(&runtime).len(), 1);
        let outcome = dispatch_request(&runtime, serde_json::json!({ "orbital": "Desk", "targetTrait": "Notes", "event": "ADD_NOTE", "viewer": { "id": "u1" } })).unwrap();
        assert_eq!(outcome["success"], true, "without wire the host still answers the raw EventResponse; the kernel shapes the model's outcome: {outcome}");
        assert!(outcome.get("ok").is_none(), "control: the host shapes nothing: {outcome}");
    }

    #[test]
    fn a_wire_answer_stamps_each_emit_with_the_orbital_that_emitted_it() {
        let runtime = desk_with_a_client_listener();
        let answer = dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "targetTrait": "Notes", "event": "ADD_NOTE", "viewer": { "id": "u1" } })).unwrap();
        let noted = answer["emittedEvents"].as_array().unwrap().iter().find(|e| e["event"] == "NOTED").cloned().expect("NOTED is answered");
        assert_eq!(noted["source"]["trait"], "Notes", "{noted}");
        assert_eq!(noted["source"]["orbital"], "Desk", "a client hears a name-only `Notes.NOTED` listen only from its own orbital (EXEC-QUEUE-6): {noted}");
    }

    #[test]
    fn an_input_request_admits_only_a_declared_external_input() {
        let runtime = desk();
        let refused = dispatch_request(&runtime, serde_json::json!({ "wire": true, "input": true, "orbital": "Desk", "targetTrait": "Agent", "event": "GO" })).unwrap();
        assert_eq!(refused["rejections"][0]["code"], "not-an-external-input", "{refused}");
        let admitted = dispatch_request(&runtime, serde_json::json!({ "wire": true, "input": true, "orbital": "Desk", "targetTrait": "Notes", "event": "ADD_NOTE", "viewer": { "id": "u1" } })).unwrap();
        assert_eq!(admitted["success"], true, "control: {admitted}");
    }

    #[test]
    fn a_request_fails_with_the_routes_code_for_an_unknown_orbital_or_locale() {
        let runtime = desk();
        let (message, errors) = dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Nowhere", "targetTrait": "Notes", "event": "ADD_NOTE" })).unwrap_err();
        assert_eq!((message.as_str(), errors.clone()), ("Orbital not found: Nowhere", serde_json::json!([{ "code": "ORBITAL_NOT_FOUND", "message": "Orbital not found: Nowhere", "status": 404 }])));
        let mut declaring = dispatching_program();
        declaring.name = "DeskLocales".to_string();
        declaring.orbitals.clear();
        declaring.locales = Some(vec!["en".to_string()]);
        runtime.register_schema(declaring).unwrap();
        let (_, errors) = dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "targetTrait": "Notes", "event": "ADD_NOTE", "locale": "xx" })).unwrap_err();
        assert!(dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "targetTrait": "Notes", "event": "ADD_NOTE", "locale": "en" })).is_ok(), "control: a declared locale");
        assert_eq!(errors[0]["code"], "LOCALE_NOT_DECLARED", "{errors}");
        assert!(dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "targetTrait": "Notes", "event": "ADD_NOTE", "locale": "" })).is_ok(), "control: an empty locale is absent");
    }

    #[test]
    fn a_native_host_refuses_carried_circuit_state_outside_a_delegated_leg() {
        let runtime = desk();
        let carried = serde_json::json!([{ "trait": "Notes", "from": "idle" }]);
        let refused = dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "event": "ADD_NOTE", "traits": carried })).unwrap();
        assert_eq!(refused["success"], false, "{refused}");
        assert_eq!(refused["rejections"][0]["code"], "client-circuit-state-refused", "{refused}");
        assert!(note_titles(&runtime).is_empty(), "nothing was run");
        let leg = dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "targetTrait": "Notes", "event": "ADD_NOTE", "traits": carried })).unwrap();
        assert_eq!(leg["success"], true, "control: a targeted leg is the delegated leg: {leg}");
        let plain = dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "event": "ADD_NOTE" })).unwrap();
        assert_ne!(plain["rejections"][0]["code"], "client-circuit-state-refused", "control: a request carrying nothing is admitted: {plain}");
    }

    struct OneLazy(std::sync::atomic::AtomicUsize);

    impl orbital_core::runtime::LazySource for OneLazy {
        fn read(&self, file: &str) -> Result<orbital_core::runtime::LazyProgram, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            assert_eq!(file, "lazy/Late.orb");
            let schema = serde_json::from_value(serde_json::json!({
                "name": "Late", "typeContractVersion": 1,
                "orbitals": [{ "name": "Late",
                    "entity": { "name": "Memo", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                    "traits": [{ "name": "Hear", "linkedEntity": "Memo",
                        "stateMachine": { "states": [{ "name": "idle", "isInitial": true }, { "name": "heard" }],
                            "transitions": [{ "from": "idle", "to": "heard", "event": "HEARD" }] } }] }]
            }))
            .unwrap();
            Ok(orbital_core::runtime::LazyProgram { schema })
        }
    }

    #[test]
    fn the_first_request_naming_a_lazy_orbital_loads_it_and_a_name_no_page_declares_stays_not_found() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let source = Arc::new(OneLazy(Default::default()));
        runtime.set_lazy_source(source.clone());
        let mut program = serde_json::to_value(dispatching_program()).unwrap();
        program["lazyPages"] = serde_json::json!([{ "path": "/late", "orbital": "Late", "orbRef": "lazy/Late.orb", "traits": [{ "name": "Hear", "dispatchMode": "persistedAwaited" }] }]);
        runtime.register_schema(serde_json::from_value(program).unwrap()).unwrap();
        let (message, _) = dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Nowhere", "targetTrait": "Hear", "event": "HEARD" })).unwrap_err();
        assert_eq!(message, "Orbital not found: Nowhere");
        assert_eq!(source.0.load(Ordering::SeqCst), 0, "control: an unknown name reads no lazy file");
        let answer = dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Late", "targetTrait": "Hear", "event": "HEARD" })).unwrap();
        assert_eq!(answer["success"], true, "{answer}");
        assert_eq!(answer["states"]["Hear"], "heard", "{answer}");
        assert_eq!(source.0.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_request_without_a_trait_discovers_its_handlers_and_never_takes_carried_traits() {
        let runtime = desk();
        let discovered = dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "event": "ADD_NOTE", "viewer": { "id": "u1" } })).unwrap();
        assert_eq!(discovered["success"], true, "{discovered}");
        assert_eq!(note_titles(&runtime), ["via request"], "discovery found Notes");
        for traits in [serde_json::json!([]), serde_json::json!([{ "trait": "Notes", "from": "idle" }])] {
            let refused = dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "event": "ADD_NOTE", "traits": traits, "viewer": { "id": "u1" } })).unwrap();
            assert_eq!(refused["rejections"][0]["code"], "client-circuit-state-refused", "{refused}");
        }
        assert_eq!(note_titles(&runtime).len(), 1, "nothing carried was run");
    }

    /// `dispatching_program` with its traits on the route page `/desk` and a `Seeder` that hears the route mount's `$MOUNT`.
    fn desk_with_a_mount_listener() -> Arc<Host> {
        let mut program = serde_json::to_value(dispatching_program()).unwrap();
        program["orbitals"][0]["traits"].as_array_mut().unwrap().push(serde_json::json!({
            "name": "Seeder", "linkedEntity": "Note",
            "listens": [{ "event": "$MOUNT", "triggers": "SEED_PAGE" }],
            "stateMachine": {
                "states": [{ "name": "idle", "isInitial": true }],
                "events": [{ "key": "SEED_PAGE", "name": "Seed page", "payloadSchema": [{ "name": "route", "type": "string", "required": true }] }],
                "transitions": [{ "from": "idle", "to": "idle", "event": "SEED_PAGE", "effects": [["set", "@entity.got", "@payload.route"]] }]
            }
        }));
        program["orbitals"][0]["pages"] = serde_json::json!([{ "name": "DeskPage", "path": "/desk", "traits": [{ "ref": "Agent" }, { "ref": "Notes" }] }]);
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(serde_json::from_value(program).unwrap()).unwrap();
        runtime
    }

    #[test]
    fn a_mount_request_naming_its_route_announces_mount_before_its_seeds() {
        let runtime = desk_with_a_mount_listener();
        dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "event": "INIT", "clientId": "tab-m", "mount": [{ "trait": "Agent", "event": "INIT" }], "route": "/desk" })).unwrap();
        let _tab = runtime.kernel.enter_session("tab-m");
        let got = serde_json::to_value(&runtime.kernel.snapshot_trait("Seeder", Some(orbital_core::RUNTIME_SINGLETON)).entity).unwrap()["got"].clone();
        assert_eq!(got, "/desk", "the $MOUNT listener heard the route (EXEC-SEED-7, twin of the embedded server host)");
    }

    /// A `Seeder` whose `$MOUNT` listener writes a `Card` through a keyed self `program/dispatch` (a deferred ask whose
    /// work is a dispatch of its own), and a `/shelf` page whose `Shelf` lists the cards on `INIT`.
    fn shelf_seeded_by_a_deferred_write() -> Arc<Host> {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(serde_json::from_value(shelf_program(true)).unwrap()).unwrap();
        runtime
    }

    /// The `Shelf` program; `listening` decides whether its `Seeder` hears `$MOUNT`. `Shelf` emits the cards it listed as `SEEN`.
    fn shelf_program(listening: bool) -> serde_json::Value {
        let machine = |events: serde_json::Value, transitions: serde_json::Value| {
            serde_json::json!({ "states": [{ "name": "idle", "isInitial": true }], "events": events, "transitions": transitions })
        };
        serde_json::json!({
            "name": "Shelf",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Shelf",
                "entity": { "name": "Card", "persistence": "persistent", "fields": [{ "name": "id", "type": "string" }, { "name": "label", "type": "string" }] },
                "traits": [{
                    "name": "Seeder", "linkedEntity": "Card",
                    "listens": if listening { serde_json::json!([{ "event": "$MOUNT", "triggers": "SEED_PAGE" }]) } else { serde_json::json!([]) },
                    "stateMachine": machine(
                        serde_json::json!([{ "key": "SEED_PAGE", "name": "Seed page" }, { "key": "SEEDED", "name": "Seeded" }]),
                        serde_json::json!([
                            { "from": "idle", "to": "idle", "event": "SEED_PAGE", "effects": [["program/dispatch", "Shelf.Writer.WRITE", {},
                                { "orbital": "Shelf", "targetTrait": "Writer", "event": "WRITE", "wire": true },
                                { "key": "seed-write", "emit": { "success": "SEEDED", "failure": "SEEDED" } }]] },
                            { "from": "idle", "to": "idle", "event": "SEEDED", "effects": [] }]))
                }, {
                    "name": "Writer", "linkedEntity": "Card",
                    "stateMachine": machine(
                        serde_json::json!([{ "key": "WRITE", "name": "Write" }]),
                        serde_json::json!([{ "from": "idle", "to": "idle", "event": "WRITE", "effects": [["persist", "create", "Card", { "id": "c1", "label": "seeded" }]] }]))
                }, {
                    "name": "Shelf", "linkedEntity": "Card",
                    "emits": [{ "event": "SEEN" }],
                    "stateMachine": machine(
                        serde_json::json!([{ "key": "INIT", "name": "Init" }, { "key": "L", "name": "Listed" }]),
                        serde_json::json!([
                            { "from": "idle", "to": "idle", "event": "INIT", "effects": [["fetch", "Card", { "emit": { "success": "L" } }]] },
                            { "from": "idle", "to": "idle", "event": "L", "effects": [["emit", "SEEN", { "listed": "@payload" }]] }]))
                }],
                "pages": [{ "name": "ShelfPage", "path": "/shelf", "traits": [{ "ref": "Shelf" }] }]
            }]
        })
    }

    /// `shelf_program(true)` whose `Seeder` first dispatches, keyed, into a `Signer` in a `seed-x` session (`input` as given)
    /// and only on its answer dispatches the write, as std-mock-seed signs a writer in before its chunk.
    fn chained_shelf_program(input: bool) -> serde_json::Value {
        let mut program = shelf_program(true);
        let traits = program["orbitals"][0]["traits"].as_array_mut().unwrap();
        traits[0]["stateMachine"] = serde_json::json!({
            "states": [{ "name": "idle", "isInitial": true }],
            "events": [{ "key": "SEED_PAGE", "name": "Seed page" }, { "key": "SIGNED", "name": "Signed" }, { "key": "SEEDED", "name": "Seeded" }],
            "transitions": [
                { "from": "idle", "to": "idle", "event": "SEED_PAGE", "effects": [["program/dispatch", "Shelf.Signer.ACT", {},
                    { "orbital": "Shelf", "targetTrait": "Signer", "event": "ACT", "clientId": "seed-x", "input": input, "wire": true },
                    { "key": "seed-signin", "emit": { "success": "SIGNED", "failure": "SEEDED" } }]] },
                { "from": "idle", "to": "idle", "event": "SIGNED", "effects": [["program/dispatch", "Shelf.Writer.WRITE", {},
                    { "orbital": "Shelf", "targetTrait": "Writer", "event": "WRITE", "clientId": "seed-x", "wire": true },
                    { "key": "seed-write", "emit": { "success": "SEEDED", "failure": "SEEDED" } }]] },
                { "from": "idle", "to": "idle", "event": "SEEDED", "effects": [] }]
        });
        traits.push(serde_json::json!({
            "name": "Signer", "linkedEntity": "Card",
            "stateMachine": { "states": [{ "name": "idle", "isInitial": true }],
                "events": [{ "key": "ACT", "name": "Act", "input": input }], "transitions": [{ "from": "idle", "to": "idle", "event": "ACT", "effects": [] }] }
        }));
        program
    }

    /// `chained_shelf_program(true)` whose `Signer` answers only after a keyed `os/getenv` of its own, as std-dev-personas
    /// reads the emulator host before it signs a seed session in; the guest is granted that variable.
    fn env_chained_shelf_program() -> serde_json::Value {
        let mut program = chained_shelf_program(true);
        let signer = &mut program["orbitals"][0]["traits"].as_array_mut().unwrap()[3];
        signer["stateMachine"] = serde_json::json!({ "states": [{ "name": "idle", "isInitial": true }],
            "events": [{ "key": "ACT", "name": "Act", "input": true }, { "key": "GOT", "name": "Got" }, { "key": "SIGNED_IN", "name": "Signed in" }],
            "transitions": [
                { "from": "idle", "to": "idle", "event": "ACT", "effects": [["os/getenv", "HOME", { "key": "who", "emit": { "success": "GOT", "failure": "GOT" } }]] },
                { "from": "idle", "to": "idle", "event": "GOT", "effects": [["emit", "SIGNED_IN", {}]] },
                { "from": "idle", "to": "idle", "event": "SIGNED_IN", "effects": [] }] });
        signer["emits"] = serde_json::json!([{ "event": "SIGNED_IN" }]);
        program["orbitals"][0]["osGrants"] = serde_json::json!({ "env": ["HOME"] });
        program
    }

    /// `chained_shelf_program(false)` whose `Seeder` is a page trait with an `INIT` of its own, as the catalog composes
    /// std-mock-page-seed: it is a seed of the mount and its `$MOUNT` listener at once.
    fn page_seeder_shelf_program() -> serde_json::Value {
        let mut program = chained_shelf_program(false);
        let seeder = &mut program["orbitals"][0]["traits"][0]["stateMachine"];
        seeder["events"].as_array_mut().unwrap().push(serde_json::json!({ "key": "INIT", "name": "Init" }));
        seeder["transitions"].as_array_mut().unwrap().push(serde_json::json!({ "from": "idle", "to": "idle", "event": "INIT", "effects": [] }));
        program["orbitals"][0]["pages"][0]["traits"] = serde_json::json!([{ "ref": "Seeder" }, { "ref": "Shelf" }]);
        program
    }

    #[test]
    fn a_hosted_guest_runs_the_mount_listener_it_also_seeds_before_any_seed_lifecycle() {
        let answer = guest_open_answer(page_seeder_shelf_program());
        assert!(answer.contains("SEEN") && answer.contains("c1"), "the Seeder's $MOUNT work is not held for its own INIT (EXEC-SEED-7 precedes EXEC-SEED-2): {answer}");
    }

    #[test]
    fn a_hosted_guest_mounts_its_page_after_a_chain_whose_step_waits_on_a_keyed_os_ask() {
        let answer = guest_open_answer_with(
            env_chained_shelf_program(),
            serde_json::json!({ "env": ["HOME"], "kernel": [{ "target": "guest", "kinds": ["spawn"] }] }),
            serde_json::json!({ "route": "/shelf", "declared": true, "grants": { "env": ["HOME"] } }),
        );
        assert!(answer.contains("SEEN") && answer.contains("c1"), "the sign-in step's keyed os/getenv is part of the listener's lineage (EXEC-SEED-7): {answer}");
    }

    /// The `Shelf` program opened as a guest at `/shelf` by an `Opener` program served here; the open's answer as text.
    fn guest_shelf_open_answer(listening: bool) -> String {
        guest_open_answer(shelf_program(listening))
    }

    fn guest_open_answer(program: serde_json::Value) -> String {
        guest_open_answer_with(program, serde_json::json!({}), serde_json::json!({ "route": "/shelf" }))
    }

    #[test]
    fn a_hosted_guest_mounts_its_page_after_a_chain_of_deferred_dispatches_its_mount_listener_made() {
        let answer = guest_open_answer(chained_shelf_program(false));
        assert!(answer.contains("SEEN") && answer.contains("c1"), "the page listed the row written after the listener's keyed sign-in step settled (EXEC-SEED-7): {answer}");
    }

    #[test]
    fn a_hosted_guest_mounts_its_page_after_a_chain_through_an_input_dispatch() {
        let answer = guest_open_answer(chained_shelf_program(true));
        assert!(answer.contains("SEEN") && answer.contains("c1"), "an input: true step is part of the listener's lineage too (EXEC-SEED-7): {answer}");
    }

    fn guest_open_answer_with(program: serde_json::Value, opener_grants: serde_json::Value, open: serde_json::Value) -> String {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let opener: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Opener",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Opener",
                "entity": { "name": "Box", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "answer", "type": "string" }] },
                "osGrants": opener_grants,
                "traits": [{
                    "name": "Opener", "linkedEntity": "Box",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "OPEN", "name": "Open", "payloadSchema": [{ "name": "program", "type": "object", "required": true }] },
                            { "key": "OPENED", "name": "Opened" }, { "key": "FAILED", "name": "Failed" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "OPEN", "effects": [["program/open", "@payload.program", open, { "emit": { "success": "OPENED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "OPENED", "effects": [["set", "@entity.answer", ["json/stringify", "@payload"]]] },
                            { "from": "idle", "to": "idle", "event": "FAILED", "effects": [["set", "@entity.answer", ["str/concat", "failed: ", ["json/stringify", "@payload"]]]] }]
                    }
                }]
            }]
        }))
        .unwrap();
        runtime.register_schema(opener).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let host = Arc::clone(&runtime);
        std::thread::spawn(move || {
            let args: Vec<Value> = [
                serde_json::json!("Opener.Opener.OPEN"),
                serde_json::json!({ "program": program }),
                serde_json::json!({ "wire": true, "orbital": "Opener", "targetTrait": "Opener", "event": "OPEN", "clientId": "tab-g" }),
            ]
            .iter()
            .map(|a| serde_json::from_value(a.clone()).unwrap())
            .collect();
            let answered = orbital_core::effects::executor::SubstrateHandler::run(host.substrate.as_ref(), "program/dispatch", &args)
                .map_err(|f| f.message)
                .map(|_| {
                    let _tab = host.kernel.enter_session("tab-g");
                    serde_json::to_value(&host.kernel.snapshot_trait("Opener", Some(orbital_core::RUNTIME_SINGLETON)).entity).unwrap()["answer"].to_string()
                });
            let _ = tx.send(answered);
        });
        rx.recv_timeout(std::time::Duration::from_secs(20)).expect("the open settles").unwrap()
    }

    /// A guest whose page shows a `[client]` trait ticking every 20 ms, and a `[client]` trait on another page ticking too.
    fn ticking_guest_program() -> serde_json::Value {
        let ticker = |name: &str, label: &str| serde_json::json!({
            "name": name, "linkedEntity": format!("{name}Count"), "dispatch": "client",
            "stateMachine": {
                "states": [{ "name": "idle", "isInitial": true }],
                "events": [{ "key": "INIT", "name": "Init" }],
                "transitions": [{ "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": ["str/concat", label, " ", "@entity.n"] }]] }]
            },
            "ticks": [{ "name": "beat", "interval": 20, "effects": [["set", "@entity.n", ["+", "@entity.n", 1]], ["render-ui", "main", { "type": "typography", "content": ["str/concat", label, " ", "@entity.n"] }]] }]
        });
        let silent = |name: &str| serde_json::json!({
            "name": format!("{name}Silent"), "linkedEntity": format!("{name}Count"), "dispatch": "client",
            "stateMachine": { "states": [{ "name": "idle", "isInitial": true }], "events": [{ "key": "INIT", "name": "Init" }], "transitions": [{ "from": "idle", "to": "idle", "event": "INIT", "effects": [] }] },
            "ticks": [{ "name": "count", "interval": 5, "effects": [["set", "@entity.n", "@entity.n"]] }]
        });
        let orbital = |name: &str, label: &str, route: &str| serde_json::json!({
            "name": format!("{name}Orbital"),
            "entity": { "name": format!("{name}Count"), "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "n", "type": "number", "default": 0 }] },
            "traits": [ticker(name, label), silent(name)],
            "pages": [{ "name": format!("{name}Page"), "path": route, "traits": [{ "ref": name }, { "ref": format!("{name}Silent") }] }]
        });
        serde_json::json!({ "name": "TickingGuest", "typeContractVersion": 1, "orbitals": [orbital("Shown", "shown", "/"), orbital("Hidden", "hidden", "/other")] })
    }

    /// Opens `program` as a hosted guest at `/`, subscribes to its ticks with `program/live { kind: "guest" }` and answers
    /// the `onMessage` payloads (each stringified) that arrived within `wait`.
    fn guest_tick_items(program: serde_json::Value, wait: std::time::Duration) -> Vec<String> {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let opener: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Opener",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Opener",
                "entity": { "name": "Box", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "items", "type": "array", "items": { "type": "string" }, "default": [] }, { "name": "failed", "type": "string" }] },
                "osGrants": { "kernel": [{ "target": "guest", "kinds": ["spawn"] }] },
                "traits": [{
                    "name": "Opener", "linkedEntity": "Box",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "OPEN", "name": "Open", "payloadSchema": [{ "name": "program", "type": "object", "required": true }] },
                            { "key": "OPENED", "name": "Opened" }, { "key": "FAILED", "name": "Failed" }, { "key": "TICKED", "name": "Ticked" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "OPEN", "effects": [["program/open", "@payload.program", { "route": "/", "declared": true }, { "emit": { "success": "OPENED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "OPENED", "effects": [["program/live", { "kind": "guest", "guest": "@payload.result.guest" }, { "key": "guest-ticks", "emit": { "onMessage": "TICKED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "TICKED", "effects": [["set", "@entity.items", ["array/concat", "@entity.items", [["json/stringify", "@payload.rendered"]]]]] },
                            { "from": "idle", "to": "idle", "event": "FAILED", "effects": [["set", "@entity.failed", ["json/stringify", "@payload"]]] }]
                    }
                }]
            }]
        }))
        .unwrap();
        runtime.register_schema(opener).unwrap();
        // A browser tab's session subscribes, as the catalog's does: every item must come back into that session.
        {
            let _tab = runtime.kernel.enter_session("tab-t");
            runtime.dispatch("Opener", "default", "OPEN", serde_json::from_value(serde_json::json!({ "program": program })).unwrap()).unwrap();
        }
        std::thread::sleep(wait);
        let _tab = runtime.kernel.enter_session("tab-t");
        let entity = serde_json::to_value(&runtime.kernel.snapshot_trait("Opener", Some("default")).entity).unwrap();
        assert!(entity["failed"].is_null() || entity["failed"] == "", "the open and the subscription succeed: {}", entity["failed"]);
        entity["items"].as_array().into_iter().flatten().filter_map(|i| i.as_str().map(str::to_string)).collect()
    }

    /// Opens a guest that reads `data/x.txt` under its working directory as it mounts, and answers what it painted (or why the open was refused).
    fn open_file_reading_guest(kernel_kinds: serde_json::Value, opener_fs: serde_json::Value, options: serde_json::Value) -> String {
        let guest = serde_json::json!({
            "name": "FileGuest", "typeContractVersion": 1,
            "osGrants": { "anchored": { "fs": [{ "anchor": "cwd", "glob": "data/**", "modes": ["read"] }] } },
            "orbitals": [{
                "name": "Files",
                "entity": { "name": "Doc", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{
                    "name": "Reader", "linkedEntity": "Doc",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "INIT", "name": "Init" }, { "key": "READ_OK", "name": "Ok" }, { "key": "READ_NO", "name": "No" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "INIT", "effects": [["os/read-file", { "anchor": "cwd", "path": "data/x.txt" }, { "emit": { "success": "READ_OK", "failure": "READ_NO" } }]] },
                            { "from": "idle", "to": "idle", "event": "READ_OK", "effects": [["render-ui", "main", { "type": "typography", "content": "read ok" }]] },
                            { "from": "idle", "to": "idle", "event": "READ_NO", "effects": [["render-ui", "main", { "type": "typography", "content": ["str/concat", "read failed: ", "@payload.error"] }]] }
                        ]
                    }
                }],
                "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Reader" }] }]
            }]
        });
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let opener: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Opener", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Opener",
                "entity": { "name": "Box", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "answer", "type": "string" }] },
                "osGrants": { "kernel": [{ "target": "guest", "kinds": kernel_kinds }], "fs": opener_fs },
                "traits": [{
                    "name": "Opener", "linkedEntity": "Box",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "OPEN", "name": "Open", "payloadSchema": [{ "name": "program", "type": "object", "required": true }] }, { "key": "OPENED", "name": "Opened" }, { "key": "FAILED", "name": "Failed" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "OPEN", "effects": [["program/open", "@payload.program", options, { "emit": { "success": "OPENED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "OPENED", "effects": [["set", "@entity.answer", ["json/stringify", "@payload.result.rendered"]]] },
                            { "from": "idle", "to": "idle", "event": "FAILED", "effects": [["set", "@entity.answer", ["str/concat", "refused: ", "@payload.error"]]] }
                        ]
                    }
                }]
            }]
        }))
        .unwrap();
        runtime.register_schema(opener).unwrap();
        runtime.dispatch("Opener", "default", "OPEN", serde_json::from_value(serde_json::json!({ "program": guest })).unwrap()).unwrap();
        let entity = serde_json::to_value(&runtime.kernel.snapshot_trait("Opener", Some("default")).entity).unwrap();
        entity["answer"].as_str().unwrap_or_default().to_string()
    }

    fn guest_reading_env(opener_env: serde_json::Value, env: serde_json::Value) -> String {
        let guest = serde_json::json!({
            "name": "EnvGuest", "typeContractVersion": 1,
            "osGrants": { "env": ["ORB_GUEST_ENV_PROBE"] },
            "orbitals": [{
                "name": "Envs",
                "entity": { "name": "Doc", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{
                    "name": "Reader", "linkedEntity": "Doc",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "INIT", "name": "Init" }, { "key": "READ_OK", "name": "Ok", "payloadSchema": [{ "name": "value", "type": "string" }] }, { "key": "READ_NO", "name": "No", "payloadSchema": [{ "name": "error", "type": "string" }] }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "INIT", "effects": [["os/getenv", "ORB_GUEST_ENV_PROBE", { "emit": { "success": "READ_OK", "failure": "READ_NO" } }]] },
                            { "from": "idle", "to": "idle", "event": "READ_OK", "effects": [["render-ui", "main", { "type": "typography", "content": ["str/concat", "env=", ["default", "@payload.value", "unset"]] }]] },
                            { "from": "idle", "to": "idle", "event": "READ_NO", "effects": [["render-ui", "main", { "type": "typography", "content": ["str/concat", "no ", "@payload.error"] }]] }
                        ]
                    }
                }],
                "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Reader" }] }]
            }]
        });
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let opener: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Opener", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Opener",
                "entity": { "name": "Box", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "answer", "type": "string" }] },
                "osGrants": { "kernel": [{ "target": "guest", "kinds": ["spawn", "declared"] }], "env": opener_env },
                "traits": [{
                    "name": "Opener", "linkedEntity": "Box",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "OPEN", "name": "Open", "payloadSchema": [{ "name": "program", "type": "object", "required": true }] }, { "key": "OPENED", "name": "Opened", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "FAILED", "name": "Failed", "payloadSchema": [{ "name": "error", "type": "string" }] }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "OPEN", "effects": [["program/open", "@payload.program", { "route": "/", "declared": true, "integrations": "live", "env": env }, { "emit": { "success": "OPENED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "OPENED", "effects": [["set", "@entity.answer", ["json/stringify", "@payload.result.rendered"]]] },
                            { "from": "idle", "to": "idle", "event": "FAILED", "effects": [["set", "@entity.answer", ["str/concat", "refused: ", "@payload.error"]]] }
                        ]
                    }
                }]
            }]
        }))
        .unwrap();
        runtime.register_schema(opener).unwrap();
        runtime.dispatch("Opener", "default", "OPEN", serde_json::from_value(serde_json::json!({ "program": guest })).unwrap()).unwrap();
        let entity = serde_json::to_value(&runtime.kernel.snapshot_trait("Opener", Some("default")).entity).unwrap();
        entity["answer"].as_str().unwrap_or_default().to_string()
    }

    #[test]
    fn a_guest_reads_the_value_program_open_set_for_it() {
        let answer = guest_reading_env(serde_json::json!(["ORB_GUEST_ENV_PROBE"]), serde_json::json!({ "ORB_GUEST_ENV_PROBE": "demo" }));
        assert!(answer.contains("env=demo"), "{answer}");
    }

    #[test]
    fn control_a_guest_opened_with_a_null_entry_reads_the_variable_as_unset() {
        let answer = guest_reading_env(serde_json::json!(["ORB_GUEST_ENV_PROBE"]), serde_json::json!({ "ORB_GUEST_ENV_PROBE": null }));
        assert!(answer.contains("env=unset"), "{answer}");
    }

    #[test]
    fn edge_an_opener_without_the_grant_cannot_set_a_guests_variable() {
        let answer = guest_reading_env(serde_json::json!([]), serde_json::json!({ "ORB_GUEST_ENV_PROBE": "demo" }));
        assert!(answer.starts_with("refused:") && answer.contains("ORB_GUEST_ENV_PROBE"), "{answer}");
    }

    fn guest_dispatch_emits(keyed: bool) -> String {
        guest_dispatch_emits_held(keyed, false)
    }

    /// `dispatch_keyed`: the opener's `program/dispatch` carries a `key` of its own, and the run is held as `orb run`
    /// holds it before the answer is read (G-ORB-638).
    fn guest_dispatch_emits_held(keyed: bool, dispatch_keyed: bool) -> String {
        let ask_options = if keyed {
            serde_json::json!({ "key": "probe", "emit": { "success": "GOT", "failure": "GOT" } })
        } else {
            serde_json::json!({ "emit": { "success": "GOT", "failure": "GOT" } })
        };
        let guest = serde_json::json!({
            "name": "BindGuest", "typeContractVersion": 1,
            "osGrants": { "env": ["ORB_GUEST_ENV_PROBE"] },
            "orbitals": [{
                "name": "Binds",
                "entity": { "name": "Doc", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{
                    "name": "Binder", "linkedEntity": "Doc",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "INIT", "name": "Init" }, { "key": "PRESENT", "name": "Present" }, { "key": "GOT", "name": "Got", "payloadSchema": [{ "name": "value", "type": "string" }] }, { "key": "BOUND", "name": "Bound" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": "ready" }]] },
                            { "from": "idle", "to": "idle", "event": "PRESENT", "effects": [["os/getenv", "ORB_GUEST_ENV_PROBE", ask_options]] },
                            { "from": "idle", "to": "idle", "event": "GOT", "effects": [["emit", "BOUND", {}]] }
                        ]
                    }
                }],
                "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Binder" }] }]
            }]
        });
        let dispatch_options = if dispatch_keyed {
            serde_json::json!({ "guest": "@payload.result.guest", "key": "ask-guest", "emit": { "success": "ANSWERED", "failure": "FAILED" } })
        } else {
            serde_json::json!({ "guest": "@payload.result.guest", "emit": { "success": "ANSWERED", "failure": "FAILED" } })
        };
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let opener: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Opener", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Opener",
                "entity": { "name": "Box", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "answer", "type": "string" }] },
                "osGrants": { "kernel": [{ "target": "guest", "kinds": ["spawn", "declared"] }], "env": ["ORB_GUEST_ENV_PROBE"] },
                "traits": [{
                    "name": "Opener", "linkedEntity": "Box",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "OPEN", "name": "Open", "payloadSchema": [{ "name": "program", "type": "object", "required": true }] }, { "key": "OPENED", "name": "Opened", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "ANSWERED", "name": "Answered", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "FAILED", "name": "Failed", "payloadSchema": [{ "name": "error", "type": "string" }] }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "OPEN", "effects": [["program/open", "@payload.program", { "route": "/", "declared": true, "integrations": "live", "env": { "ORB_GUEST_ENV_PROBE": "demo" } }, { "emit": { "success": "OPENED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "OPENED", "effects": [["program/dispatch", "PRESENT", {}, dispatch_options]] },
                            { "from": "idle", "to": "idle", "event": "ANSWERED", "effects": [["set", "@entity.answer", ["json/stringify", "@payload.result.emitted"]]] },
                            { "from": "idle", "to": "idle", "event": "FAILED", "effects": [["set", "@entity.answer", ["str/concat", "refused: ", "@payload.error"]]] }
                        ]
                    }
                }]
            }]
        }))
        .unwrap();
        runtime.register_schema(opener).unwrap();
        runtime.dispatch("Opener", "default", "OPEN", serde_json::from_value(serde_json::json!({ "program": guest })).unwrap()).unwrap();
        if dispatch_keyed {
            let (_keep, stop) = std::sync::mpsc::channel::<()>();
            assert_eq!(super::super::run::hold(&runtime, &stop), super::super::run::RunEnd::Idle);
        }
        let entity = serde_json::to_value(&runtime.kernel.snapshot_trait("Opener", Some("default")).entity).unwrap();
        entity["answer"].as_str().unwrap_or_default().to_string()
    }

    /// A `persist` on a store-bound entity whose driver answers after a deferred file read. `reply` is the payload the
    /// driver's `ROWS_WRITTEN` carries; its declared contract takes `outcomes` as a list. What the asker heard, or
    /// `None` when its dispatch had not ended after ten seconds.
    fn deferred_driver_write(label: &str, reply: serde_json::Value) -> Option<String> {
        deferred_driver_write_heard_as(label, reply, serde_json::json!([]))
    }

    /// [`deferred_driver_write`] where the asker declares `saved` as the payload contract of its success event.
    fn deferred_driver_write_heard_as(label: &str, reply: serde_json::Value, saved: serde_json::Value) -> Option<String> {
        let dir = std::env::temp_dir().join(format!("orb-driver-reply-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("k.txt"), "k").unwrap();
        let dir = std::fs::canonicalize(&dir).unwrap();
        let file = dir.join("k.txt");
        let program: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "DriverReply", "typeContractVersion": 1,
            "osGrants": { "fs": [{ "glob": format!("{}/**", dir.to_str().unwrap()), "modes": ["read"] }] },
            "orbitals": [{
                "name": "Desk",
                "entity": { "name": "Box", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "answer", "type": "string" }] },
                "auxiliaryEntities": [{ "name": "Ticket", "persistence": "persistent", "collection": "tickets", "store": "Rows", "fields": [{ "name": "id", "type": "string" }] }],
                "traits": [{ "name": "Desk", "linkedEntity": "Box", "stateMachine": {
                    "states": [{ "name": "idle", "isInitial": true }],
                    "events": [{ "key": "SAVE", "name": "Save" }, { "key": "SAVED", "name": "Saved", "payloadSchema": saved }, { "key": "SAVE_FAILED", "name": "SaveFailed", "payloadSchema": [{ "name": "error", "type": "string" }] }],
                    "transitions": [
                        { "from": "idle", "to": "idle", "event": "SAVE", "effects": [["persist", "create", "Ticket", { "id": "t1" }, { "emit": { "success": "SAVED", "failure": "SAVE_FAILED" } }]] },
                        { "from": "idle", "to": "idle", "event": "SAVED", "effects": [["set", "@entity.answer", "saved"]] },
                        { "from": "idle", "to": "idle", "event": "SAVE_FAILED", "effects": [["set", "@entity.answer", "failed"]] }
                    ] } }]
            }, {
                "name": "Rows", "pages": [],
                "entity": { "name": "Mirror", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{ "name": "RowsDriver", "linkedEntity": "Mirror",
                    "emits": [{ "event": "ROWS_WRITTEN", "payloadSchema": [{ "name": "key", "type": "string", "required": true }, { "name": "outcomes", "type": "array", "required": true, "items": { "type": "object" } }] }],
                    "stateMachine": {
                        "states": [{ "name": "ready", "isInitial": true }],
                        "events": [{ "key": "ROWS_WRITE", "name": "ROWS_WRITE" }, { "key": "FILE_READ", "name": "FILE_READ" }],
                        "transitions": [
                            { "from": "ready", "to": "ready", "event": "ROWS_WRITE", "effects": [["os/read-file", file.to_str().unwrap(), { "key": "@payload.key", "emit": { "success": "FILE_READ", "failure": "FILE_READ" } }]] },
                            { "from": "ready", "to": "ready", "event": "FILE_READ", "effects": [["emit", "ROWS_WRITTEN", reply]] }
                        ] } }]
            }]
        }))
        .unwrap();
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(program).unwrap();
        let (done, ended) = std::sync::mpsc::channel();
        let asker = runtime.clone();
        std::thread::spawn(move || {
            let _ = asker.dispatch("Desk", "default", "SAVE", serde_json::from_value(serde_json::json!({})).unwrap());
            let entity = serde_json::to_value(&asker.kernel.snapshot_trait("Desk", Some("default")).entity).unwrap();
            let _ = done.send(entity["answer"].as_str().unwrap_or_default().to_string());
        });
        ended.recv_timeout(std::time::Duration::from_secs(10)).ok()
    }

    // G-CROSS-581
    #[test]
    fn a_store_drivers_deferred_reply_that_breaks_its_contract_fails_the_asking_persist() {
        let broken = deferred_driver_write("broken", serde_json::json!({ "key": "@payload.key", "outcomes": "not a list" }));
        assert_eq!(broken.as_deref(), Some("failed"), "the persist ends on its failure route, it does not wait forever");
    }

    #[test]
    fn a_store_drivers_deferred_outcome_that_is_no_write_outcome_or_misfits_its_entity_fails_the_persist() {
        let bogus = deferred_driver_write("bogus", serde_json::json!({ "key": "@payload.key", "outcomes": [{ "kind": "bogus" }] }));
        assert_eq!(bogus.as_deref(), Some("failed"), "an outcome that is no RowWriteOutcome");
        let misfit = deferred_driver_write("misfit", serde_json::json!({ "key": "@payload.key", "outcomes": [{ "kind": "created", "row": { "id": 7 } }] }));
        assert_eq!(misfit.as_deref(), Some("failed"), "a written row that does not fit its entity");
        let none = deferred_driver_write("none", serde_json::json!({ "key": "another-ask", "outcomes": [] }));
        assert_eq!(none.as_deref(), Some("failed"), "edge: the driver's work settles with no reply to this ask");
    }

    #[test]
    fn control_a_store_drivers_deferred_reply_that_fits_its_contract_completes_the_persist() {
        let fits = deferred_driver_write("fits", serde_json::json!({ "key": "@payload.key", "outcomes": [{ "kind": "created", "row": { "id": "t1" } }] }));
        assert_eq!(fits.as_deref(), Some("saved"));
    }

    fn guest_signed_in_access(verify: &str) -> String {
        let dir = std::env::temp_dir().join(format!("orb-guest-sign-{verify}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("keys.txt"), "k").unwrap();
        let dir = std::fs::canonicalize(&dir).unwrap();
        let keys = dir.join("keys.txt");
        let fs_grant = serde_json::json!([{ "glob": format!("{}/**", dir.to_str().unwrap()), "modes": ["read"] }]);
        let header = orbital_core::codec::base64url_encode(br#"{"alg":"none","typ":"JWT"}"#);
        let claims = serde_json::json!({ "iss": "firebase-auth-emulator@example.com", "aud": "test", "sub": "u1", "exp": 4_000_000_000u64 });
        let token = format!("{header}.{}.", orbital_core::codec::base64url_encode(claims.to_string().as_bytes()));
        let sign_in = serde_json::json!(["let", [["v", ["jws/verify", "@entity.token", [], { "issuer": "firebase-auth-emulator@example.com", "audience": "test", "allowUnsigned": true }]]], ["identity/sign-in", "@v"]]);
        let presented = match verify {
            "keyed" => serde_json::json!([["set", "@entity.token", "@payload.token"], ["os/getenv", "ORB_GUEST_ENV_PROBE", { "key": "verify", "emit": { "success": "VERIFIED", "failure": "VERIFIED" } }]]),
            "late" | "store-late" | "deferred-store-late" => serde_json::json!([["set", "@entity.token", "@payload.token"], ["os/read-file", keys.to_str().unwrap(), { "key": "verify", "emit": { "success": "VERIFIED", "failure": "VERIFIED" } }]]),
            _ => serde_json::json!([["set", "@entity.token", "@payload.token"], sign_in.clone()]),
        };
        let stored = verify.contains("store");
        let driver = if verify.starts_with("deferred") {
            serde_json::json!([
                { "from": "ready", "to": "ready", "event": "ROW_READ", "effects": [["os/read-file", keys.to_str().unwrap(), { "key": "@payload.key", "emit": { "success": "FILE_READ", "failure": "FILE_READ" } }]] },
                { "from": "ready", "to": "ready", "event": "FILE_READ", "effects": [["emit", "ROW_FOUND", { "key": "@payload.key", "row": { "id": "u1" } }]] }
            ])
        } else {
            serde_json::json!([
                { "from": "ready", "to": "ready", "event": "ROW_READ", "effects": [["emit", "ROW_FOUND", { "key": "@payload.key", "row": { "id": "u1" } }]] }
            ])
        };
        let principal = if stored {
            serde_json::json!({ "name": "Principal", "identity": true, "persistence": "persistent", "collection": "principals", "store": "Rows", "fields": [{ "name": "id", "type": "string" }] })
        } else {
            serde_json::json!({ "name": "Principal", "identity": true, "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] })
        };
        let mut guest = serde_json::json!({
            "name": "GatedGuest", "typeContractVersion": 1,
            "site": { "signIn": "/" },
            "osGrants": { "env": ["ORB_GUEST_ENV_PROBE"], "fs": fs_grant.clone() },
            "orbitals": [{
                "name": "Gated",
                "entity": principal,
                "traits": [
                    { "name": "Home", "linkedEntity": "Principal", "stateMachine": { "states": [{ "name": "idle", "isInitial": true }], "events": [{ "key": "INIT", "name": "Init" }],
                        "transitions": [{ "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": "home" }]] }] } },
                    { "name": "Me", "linkedEntity": "Principal", "stateMachine": { "states": [{ "name": "idle", "isInitial": true }], "events": [{ "key": "INIT", "name": "Init" }],
                        "transitions": [{ "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": "me" }]] }] } }
                ],
                "pages": [
                    { "name": "Home", "path": "/", "traits": [{ "ref": "Home" }] },
                    { "name": "Me", "path": "/me", "access": "authenticated", "traits": [{ "ref": "Me" }] }
                ]
            }, {
                "name": "Signing",
                "entity": { "name": "SignBox", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "token", "type": "string" }] },
                "traits": [{
                    "name": "Signer", "linkedEntity": "SignBox",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "PRESENTED", "name": "Presented", "payloadSchema": [{ "name": "token", "type": "string", "required": true }] }, { "key": "VERIFIED", "name": "Verified", "payloadSchema": [{ "name": "value", "type": "string" }] }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "PRESENTED", "effects": presented },
                            { "from": "idle", "to": "idle", "event": "VERIFIED", "effects": [sign_in] }
                        ]
                    }
                }],
                "pages": [{ "name": "SignerBg", "path": "", "protocol": "boot", "traits": [{ "ref": "Signer" }] }]
            }]
        });
        if stored {
            guest["orbitals"].as_array_mut().unwrap().push(serde_json::json!({ "name": "Rows", "pages": [],
                "entity": { "name": "Mirror", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{ "name": "RowsDriver", "linkedEntity": "Mirror", "stateMachine": {
                    "states": [{ "name": "ready", "isInitial": true }],
                    "events": [{ "key": "ROW_READ", "name": "ROW_READ" }, { "key": "FILE_READ", "name": "FILE_READ" }],
                    "emits": [{ "event": "ROW_FOUND" }],
                    "transitions": driver } }] }));
        }
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let opener: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Opener", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Opener",
                "entity": { "name": "Box", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "guest", "type": "string" }, { "name": "answer", "type": "string" }] },
                "osGrants": { "kernel": [{ "target": "guest", "kinds": ["spawn", "declared"] }], "env": ["ORB_GUEST_ENV_PROBE"], "fs": fs_grant },
                "traits": [{
                    "name": "Opener", "linkedEntity": "Box",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "NAV", "name": "Nav" }, { "key": "OPEN", "name": "Open", "payloadSchema": [{ "name": "program", "type": "object", "required": true }, { "name": "token", "type": "string", "required": true }] }, { "key": "OPENED", "name": "Opened", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "ANSWERED", "name": "Answered", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "NAVIGATED", "name": "Navigated", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "FAILED", "name": "Failed", "payloadSchema": [{ "name": "error", "type": "string" }] }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "OPEN", "effects": [["set", "@entity.answer", "@payload.token"], ["program/open", "@payload.program", { "route": "/", "declared": true, "integrations": "live", "env": { "ORB_GUEST_ENV_PROBE": "demo" } }, { "emit": { "success": "OPENED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "OPENED", "effects": [["set", "@entity.guest", "@payload.result.guest"], ["program/dispatch", "Signer.PRESENTED", { "token": "@entity.answer" }, { "guest": "@payload.result.guest", "emit": { "success": "ANSWERED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "ANSWERED", "effects": [] },
                            { "from": "idle", "to": "idle", "event": "NAV", "effects": [["program/navigate", { "guest": "@entity.guest", "route": "/me", "remount": true }, { "emit": { "success": "NAVIGATED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "NAVIGATED", "effects": [["set", "@entity.answer", ["json/stringify", "@payload.result.access"]]] },
                            { "from": "idle", "to": "idle", "event": "FAILED", "effects": [["set", "@entity.answer", ["str/concat", "refused: ", "@payload.error"]]] }
                        ]
                    }
                }]
            }]
        }))
        .unwrap();
        runtime.register_schema(opener).unwrap();
        runtime.dispatch("Opener", "default", "OPEN", serde_json::from_value(serde_json::json!({ "program": guest, "token": token })).unwrap()).unwrap();
        let handle = serde_json::to_value(&runtime.kernel.snapshot_trait("Opener", Some("default")).entity).unwrap()["guest"].as_str().unwrap_or_default().to_string();
        for _ in 0..25 {
            let _ = runtime.kernel.guest_tick_pass(&handle);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        runtime.dispatch("Opener", "default", "NAV", serde_json::from_value(serde_json::json!({})).unwrap()).unwrap();
        let entity = serde_json::to_value(&runtime.kernel.snapshot_trait("Opener", Some("default")).entity).unwrap();
        entity["answer"].as_str().unwrap_or_default().to_string()
    }

    #[test]
    fn a_guest_signed_in_from_a_completion_is_admitted_to_its_authenticated_page() {
        let answer = guest_signed_in_access("keyed");
        assert!(answer.contains("admitted") && answer.contains("u1"), "a sign-in run after a keyed ask completes binds the guest's viewer: {answer}");
    }

    #[test]
    fn a_guest_signed_in_from_a_completion_relayed_on_a_live_pass_is_admitted_to_its_authenticated_page() {
        let answer = guest_signed_in_access("late");
        assert!(answer.contains("admitted") && answer.contains("u1"), "a sign-in run on a later live pass binds the guest's viewer, not a viewerless session: {answer}");
    }

    #[test]
    fn a_guest_signed_in_against_a_store_driven_identity_row_is_admitted_to_its_authenticated_page() {
        let answer = guest_signed_in_access("store");
        assert!(answer.contains("admitted") && answer.contains("u1"), "a sign-in that reads its identity row through the store driver binds the guest's viewer: {answer}");
    }

    #[test]
    fn a_guest_signed_in_on_a_live_pass_against_a_store_driven_identity_row_is_admitted() {
        let answer = guest_signed_in_access("store-late");
        assert!(answer.contains("admitted") && answer.contains("u1"), "{answer}");
    }

    #[test]
    fn a_guest_signed_in_against_an_identity_row_its_driver_answers_later_is_admitted() {
        let answer = guest_signed_in_access("deferred-store");
        assert!(answer.contains("admitted") && answer.contains("u1"), "a sign-in parked on the driver's deferred reply binds the guest's viewer when it resumes: {answer}");
    }

    #[test]
    fn a_guest_signed_in_on_a_live_pass_against_an_identity_row_its_driver_answers_later_is_admitted() {
        let answer = guest_signed_in_access("deferred-store-late");
        assert!(answer.contains("admitted") && answer.contains("u1"), "{answer}");
    }

    #[test]
    fn control_a_guest_signed_in_directly_is_admitted_to_its_authenticated_page() {
        let answer = guest_signed_in_access("direct");
        assert!(answer.contains("admitted") && answer.contains("u1"), "{answer}");
    }

    fn guest_late_emits(deferred: bool) -> Vec<String> {
        let dir = std::env::temp_dir().join(format!("orb-guest-late-{deferred}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("token.txt");
        std::fs::write(&file, "t").unwrap();
        let dir = std::fs::canonicalize(&dir).unwrap();
        let file = dir.join("token.txt");
        let read = if deferred {
            serde_json::json!(["os/read-file", file.to_str().unwrap(), { "key": "late", "emit": { "success": "DONE", "failure": "DONE" } }])
        } else {
            serde_json::json!(["emit", "DONE", {}])
        };
        let guest = serde_json::json!({
            "name": "LateGuest", "typeContractVersion": 1,
            "osGrants": { "fs": [{ "glob": format!("{}/**", dir.to_str().unwrap()), "modes": ["read"] }] },
            "orbitals": [{
                "name": "Lates",
                "entity": { "name": "Doc", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{
                    "name": "Late", "linkedEntity": "Doc",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "INIT", "name": "Init" }, { "key": "PRESENT", "name": "Present" }, { "key": "DONE", "name": "Done" }, { "key": "BOUND", "name": "Bound" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": "ready" }]] },
                            { "from": "idle", "to": "idle", "event": "PRESENT", "effects": [read] },
                            { "from": "idle", "to": "idle", "event": "DONE", "effects": [["emit", "BOUND", {}]] }
                        ]
                    }
                }],
                "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Late" }] }]
            }]
        });
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let opener: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Opener", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Opener",
                "entity": { "name": "Box", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "guest", "type": "string" }] },
                "osGrants": { "kernel": [{ "target": "guest", "kinds": ["spawn", "declared"] }], "fs": [{ "glob": format!("{}/**", dir.to_str().unwrap()), "modes": ["read"] }] },
                "traits": [{
                    "name": "Opener", "linkedEntity": "Box",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "OPEN", "name": "Open", "payloadSchema": [{ "name": "program", "type": "object", "required": true }] }, { "key": "OPENED", "name": "Opened", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "ANSWERED", "name": "Answered", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "FAILED", "name": "Failed", "payloadSchema": [{ "name": "error", "type": "string" }] }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "OPEN", "effects": [["program/open", "@payload.program", { "route": "/", "declared": true, "integrations": "live" }, { "emit": { "success": "OPENED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "OPENED", "effects": [["set", "@entity.guest", "@payload.result.guest"], ["program/dispatch", "PRESENT", {}, { "guest": "@payload.result.guest", "emit": { "success": "ANSWERED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "ANSWERED", "effects": [] },
                            { "from": "idle", "to": "idle", "event": "FAILED", "effects": [] }
                        ]
                    }
                }]
            }]
        }))
        .unwrap();
        runtime.register_schema(opener).unwrap();
        runtime.dispatch("Opener", "default", "OPEN", serde_json::from_value(serde_json::json!({ "program": guest })).unwrap()).unwrap();
        let entity = serde_json::to_value(&runtime.kernel.snapshot_trait("Opener", Some("default")).entity).unwrap();
        let handle = entity["guest"].as_str().unwrap_or_default().to_string();
        let mut seen = Vec::new();
        for _ in 0..100 {
            if let Ok(orbital_core::runtime::kernel::GuestTicks::Painted(answer)) = runtime.kernel.guest_tick_pass(&handle) {
                let answer = serde_json::to_value(&answer).unwrap();
                for emitted in answer["emitted"].as_array().cloned().unwrap_or_default() {
                    seen.push(emitted["event"].as_str().unwrap_or_default().to_string());
                }
            }
            if seen.iter().any(|e| e == "BOUND") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        seen
    }

    struct PortEcho;

    impl orbital_core::effects::executor::SubstrateHandler for PortEcho {
        fn run(&self, op: &str, _args: &[Value]) -> Result<Value, orbital_core::effects::executor::SubstrateFailure> {
            Ok(Value::string(format!("ran {op}")))
        }
    }

    /// What a guest's `program/read` did, as the opener's dispatch into it reports: the events the guest emitted, or
    /// `OPEN_REFUSED` when `program/open` failed. `held` is whether the opener holds `guest: [program]`, `asked`
    /// whether its `program/open` asks for the port.
    fn guest_program_port(held: bool, asked: bool) -> Vec<String> {
        let guest = serde_json::json!({
            "name": "PortGuest", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Ports",
                "entity": { "name": "Doc", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{
                    "name": "Port", "linkedEntity": "Doc",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "INIT", "name": "Init" }, { "key": "ASK", "name": "Ask" }, { "key": "GOT", "name": "Got" }, { "key": "REFUSED", "name": "Refused" }, { "key": "HAD_PORT", "name": "Had" }, { "key": "NO_PORT", "name": "No" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": "ready" }]] },
                            { "from": "idle", "to": "idle", "event": "ASK", "effects": [["program/read", "app x [ version: \"1.0.0\" ]", {}, { "emit": { "success": "GOT", "failure": "REFUSED" } }]] },
                            { "from": "idle", "to": "idle", "event": "GOT", "effects": [["emit", "HAD_PORT", {}]] },
                            { "from": "idle", "to": "idle", "event": "REFUSED", "effects": [["emit", "NO_PORT", {}]] }
                        ]
                    }
                }],
                "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Port" }] }]
            }]
        });
        let config = HostConfig { substrate: Some(SubstratePort(Arc::new(PortEcho))), ..HostConfig::default() };
        let runtime = Host::with_config(Arc::new(DriverAdapter::memory()), config).shared();
        let kinds = if held { serde_json::json!(["spawn", "program"]) } else { serde_json::json!(["spawn"]) };
        let opener: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Opener", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Opener",
                "entity": { "name": "Box", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "seen", "type": "object" }, { "name": "refused", "type": "string" }] },
                "osGrants": { "kernel": [{ "target": "guest", "kinds": kinds }] },
                "traits": [{
                    "name": "Opener", "linkedEntity": "Box",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "OPEN", "name": "Open", "payloadSchema": [{ "name": "program", "type": "object", "required": true }] }, { "key": "OPENED", "name": "Opened", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "ANSWERED", "name": "Answered", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "FAILED", "name": "Failed", "payloadSchema": [{ "name": "error", "type": "string" }] }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "OPEN", "effects": [["program/open", "@payload.program", { "route": "/", "grants": {}, "program": asked }, { "emit": { "success": "OPENED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "OPENED", "effects": [["program/dispatch", "ASK", {}, { "guest": "@payload.result.guest", "emit": { "success": "ANSWERED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "ANSWERED", "effects": [["set", "@entity.seen", "@payload.result"]] },
                            { "from": "idle", "to": "idle", "event": "FAILED", "effects": [["set", "@entity.refused", "@payload.error"]] }
                        ]
                    }
                }]
            }]
        }))
        .unwrap();
        runtime.register_schema(opener).unwrap();
        runtime.dispatch("Opener", "default", "OPEN", serde_json::from_value(serde_json::json!({ "program": guest })).unwrap()).unwrap();
        let entity = serde_json::to_value(&runtime.kernel.snapshot_trait("Opener", Some("default")).entity).unwrap();
        if entity["refused"].as_str().is_some_and(|text| !text.is_empty()) {
            return vec!["OPEN_REFUSED".to_string(), entity["refused"].as_str().unwrap_or_default().to_string()];
        }
        entity["seen"]["emitted"].as_array().cloned().unwrap_or_default().iter().map(|e| e["event"].as_str().unwrap_or_default().to_string()).collect()
    }

    #[test]
    fn g_orbital_shell_rust_037_a_guest_reads_a_program_only_with_the_program_grant_asked_and_held() {
        let granted = guest_program_port(true, true);
        assert!(granted.iter().any(|e| e == "HAD_PORT"), "held and asked: the guest's program/read is answered by the opener's port: {granted:?}");
        let unasked = guest_program_port(true, false);
        assert!(unasked.iter().any(|e| e == "NO_PORT"), "control: held but not asked, the guest has no program port: {unasked:?}");
        let unheld = guest_program_port(false, true);
        assert_eq!(unheld.first().map(String::as_str), Some("OPEN_REFUSED"), "asked without the grant: program/open refuses: {unheld:?}");
        assert!(unheld.get(1).is_some_and(|reason| reason.contains("program")), "the refusal names the grant: {unheld:?}");
    }

    #[test]
    fn a_guests_late_completion_emits_reach_the_opener_on_its_live_pass() {
        let seen = guest_late_emits(true);
        assert!(seen.iter().any(|e| e == "BOUND"), "the BOUND a deferred completion emitted arrives on the guest's live pass: {seen:?}");
    }

    #[test]
    fn control_what_a_guest_dispatch_already_answered_is_not_relayed_again() {
        let seen = guest_late_emits(false);
        assert!(!seen.iter().any(|e| e == "BOUND"), "a BOUND emitted inside the dispatch was answered there, not on the live pass: {seen:?}");
    }

    fn guest_said(arguments: serde_json::Value) -> String {
        let guest = serde_json::json!({
            "name": "SayGuest", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Says",
                "entity": { "name": "Doc", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{
                    "name": "Sayer", "linkedEntity": "Doc",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "INIT", "name": "Init" }, { "key": "SAY", "name": "Say", "payloadSchema": [{ "name": "word", "type": "string", "required": true }] }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": "ready" }]] },
                            { "from": "idle", "to": "idle", "event": "SAY", "effects": [["render-ui", "main", { "type": "typography", "content": ["str/concat", "said ", "@payload.word"] }]] }
                        ]
                    }
                }],
                "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Sayer" }] }]
            }]
        });
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let opener: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Opener", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Opener",
                "entity": { "name": "Box", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "answer", "type": "string" }] },
                "osGrants": { "kernel": [{ "target": "guest", "kinds": ["spawn", "declared"] }] },
                "traits": [{
                    "name": "Opener", "linkedEntity": "Box",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "OPEN", "name": "Open", "payloadSchema": [{ "name": "program", "type": "object", "required": true }] }, { "key": "OPENED", "name": "Opened", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "ANSWERED", "name": "Answered", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "FAILED", "name": "Failed", "payloadSchema": [{ "name": "error", "type": "string" }] }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "OPEN", "effects": [["program/open", "@payload.program", { "route": "/", "declared": true, "integrations": "live" }, { "emit": { "success": "OPENED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "OPENED", "effects": [["program/dispatch", "Sayer.SAY", arguments, { "guest": "@payload.result.guest", "emit": { "success": "ANSWERED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "ANSWERED", "effects": [["set", "@entity.answer", ["json/stringify", "@payload.result.rendered"]]] },
                            { "from": "idle", "to": "idle", "event": "FAILED", "effects": [["set", "@entity.answer", ["str/concat", "refused: ", "@payload.error"]]] }
                        ]
                    }
                }]
            }]
        }))
        .unwrap();
        runtime.register_schema(opener).unwrap();
        runtime.dispatch("Opener", "default", "OPEN", serde_json::from_value(serde_json::json!({ "program": guest })).unwrap()).unwrap();
        let entity = serde_json::to_value(&runtime.kernel.snapshot_trait("Opener", Some("default")).entity).unwrap();
        entity["answer"].as_str().unwrap_or_default().to_string()
    }

    #[test]
    fn a_guest_dispatch_takes_its_payload_as_json_text() {
        let answer = guest_said(serde_json::json!("{\"word\":\"hi\"}"));
        assert!(answer.contains("said hi"), "{answer}");
    }

    #[test]
    fn control_a_guest_dispatch_takes_its_payload_as_an_object() {
        let answer = guest_said(serde_json::json!({ "word": "hi" }));
        assert!(answer.contains("said hi"), "{answer}");
    }

    #[test]
    fn edge_a_guest_dispatch_refuses_text_that_is_not_a_json_object() {
        let answer = guest_said(serde_json::json!("[1, 2]"));
        assert!(answer.starts_with("refused:") && answer.contains("JSON object"), "{answer}");
    }

    #[test]
    fn a_guest_dispatch_answers_what_its_keyed_completions_emit() {
        let answer = guest_dispatch_emits(true);
        assert!(answer.contains("BOUND"), "the guest's BOUND, emitted once its keyed ask completes, is in the dispatch answer: {answer}");
    }

    #[test]
    fn a_held_run_waits_for_a_keyed_guest_dispatch_to_complete() {
        let answer = guest_dispatch_emits_held(true, true);
        assert!(answer.contains("BOUND"), "the keyed dispatch's completion lands before the run ends: {answer:?}");
        let unkeyed_ask = guest_dispatch_emits_held(false, true);
        assert!(unkeyed_ask.contains("BOUND"), "edge: a keyed dispatch of an event whose ask is unkeyed: {unkeyed_ask:?}");
    }

    #[test]
    fn control_a_guest_dispatch_answers_what_an_unkeyed_ask_emits() {
        let answer = guest_dispatch_emits(false);
        assert!(answer.contains("BOUND"), "{answer}");
    }

    fn guest_root_with_file(label: &str) -> String {
        let root = std::env::temp_dir().join(format!("orb-guest-root-{label}-{}", std::process::id()));
        std::fs::create_dir_all(root.join("data")).unwrap();
        std::fs::write(root.join("data/x.txt"), "x").unwrap();
        std::fs::canonicalize(root).unwrap().to_string_lossy().into_owned()
    }

    #[test]
    fn a_live_guest_of_an_opener_holding_guest_declared_keeps_its_declared_grants_in_its_own_working_directory() {
        let root = guest_root_with_file("live");
        let answer = open_file_reading_guest(
            serde_json::json!(["spawn", "declared"]),
            serde_json::json!([{ "glob": format!("{root}/**"), "modes": ["read", "write", "create", "remove"] }]),
            serde_json::json!({ "route": "/", "declared": true, "integrations": "live", "cwd": root }),
        );
        assert!(answer.contains("read ok"), "the guest reads data/x.txt under its own working directory: {answer}");
    }

    #[test]
    fn control_a_mocked_guest_stays_narrowed_even_when_the_opener_holds_guest_declared() {
        let answer = open_file_reading_guest(serde_json::json!(["spawn", "declared"]), serde_json::json!([]), serde_json::json!({ "route": "/", "declared": true, "integrations": "mock" }));
        assert!(answer.contains("read failed"), "mock mode narrows the declared fs grant away: {answer}");
    }

    #[test]
    fn control_a_live_guest_without_the_guest_declared_grant_stays_narrowed() {
        let answer = open_file_reading_guest(serde_json::json!(["spawn"]), serde_json::json!([]), serde_json::json!({ "route": "/", "declared": true, "integrations": "live" }));
        assert!(answer.contains("read failed"), "without the explicit grant the 11-07 narrowing still applies: {answer}");
    }

    #[test]
    fn edge_a_guest_working_directory_the_opener_cannot_reach_refuses_the_open() {
        let root = guest_root_with_file("unreachable");
        let answer = open_file_reading_guest(serde_json::json!(["spawn", "declared"]), serde_json::json!([]), serde_json::json!({ "route": "/", "declared": true, "integrations": "live", "cwd": root }));
        assert!(answer.starts_with("refused:") && answer.contains("does not hold"), "{answer}");
    }

    #[test]
    fn edge_a_mocked_guest_takes_no_working_directory() {
        let root = guest_root_with_file("mock");
        let answer = open_file_reading_guest(
            serde_json::json!(["spawn", "declared"]),
            serde_json::json!([{ "glob": format!("{root}/**"), "modes": ["read", "write", "create", "remove"] }]),
            serde_json::json!({ "route": "/", "declared": true, "integrations": "mock", "cwd": root }),
        );
        assert!(answer.starts_with("refused:") && answer.contains("`cwd`"), "{answer}");
    }

    /// Opens a guest declaring env `PATH` and `ORB_SURELY_UNSET`, then answers `program/inputs { guest, environment }`.
    fn guest_environment(kernel_kinds: serde_json::Value, environment: serde_json::Value) -> serde_json::Value {
        let guest = serde_json::json!({
            "name": "EnvGuest", "typeContractVersion": 1,
            "osGrants": { "env": ["PATH", "ORB_SURELY_UNSET"], "net": [{ "target": "api.example.com:443", "kinds": ["tcp"] }] },
            "orbitals": [{
                "name": "Envs",
                "entity": { "name": "Doc", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{ "name": "Shown", "linkedEntity": "Doc", "stateMachine": { "states": [{ "name": "idle", "isInitial": true }], "events": [{ "key": "INIT", "name": "Init" }], "transitions": [{ "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": "up" }]] }] } }],
                "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Shown" }] }]
            }]
        });
        guest_read(guest, kernel_kinds, serde_json::json!({ "environment": environment }))
    }

    /// A guest whose traits name the credentials `PATH` (held in the process environment) and `ORB_SURELY_UNSET`, and whose
    /// `INIT` asks for a file its grants do not cover.
    fn guest_with_secrets_and_a_refused_read() -> serde_json::Value {
        serde_json::json!({
            "name": "SecretGuest", "typeContractVersion": 1,
            "osGrants": { "fs": [{ "glob": "/orb-granted-elsewhere/**", "modes": ["read"] }] },
            "orbitals": [{
                "name": "Secrets",
                "entity": { "name": "Doc", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{ "name": "Shown", "linkedEntity": "Doc", "stateMachine": {
                    "states": [{ "name": "idle", "isInitial": true }],
                    "events": [{ "key": "INIT", "name": "Init" }, { "key": "A", "name": "A" }, { "key": "B", "name": "B" }, { "key": "READ_DONE", "name": "Done" }, { "key": "READ_FAILED", "name": "Failed", "payloadSchema": [{ "name": "error", "type": "string" }, { "name": "kind", "type": "string" }] }],
                    "transitions": [
                        { "from": "idle", "to": "idle", "event": "INIT", "effects": [["os/read-file", "/orb-never-granted/a.txt", { "emit": { "success": "READ_DONE", "failure": "READ_FAILED" } }]] },
                        { "from": "idle", "to": "idle", "event": "A", "effects": [["credential-status", { "credential": "PATH", "scope": "app" }, { "emit": { "success": "READ_DONE", "failure": "READ_FAILED" } }]] },
                        { "from": "idle", "to": "idle", "event": "B", "effects": [["credential-status", { "credential": "ORB_SURELY_UNSET", "scope": "app" }, { "emit": { "success": "READ_DONE", "failure": "READ_FAILED" } }]] },
                        { "from": "idle", "to": "idle", "event": "READ_DONE" },
                        { "from": "idle", "to": "idle", "event": "READ_FAILED" }
                    ]
                } }],
                "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Shown" }] }]
            }]
        })
    }

    /// Opens `guest` live under an opener holding `kernel: { guest: kernel_kinds }`, then answers `program/inputs { guest, ..read }`.
    fn guest_read(guest: serde_json::Value, kernel_kinds: serde_json::Value, read: serde_json::Value) -> serde_json::Value {
        let mut ask = read;
        ask["guest"] = serde_json::json!("@entity.guest");
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let opener: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Opener", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Opener",
                "entity": { "name": "Box", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "guest", "type": "string" }, { "name": "answer", "type": "string" }] },
                "osGrants": { "kernel": [{ "target": "guest", "kinds": kernel_kinds }] },
                "traits": [{
                    "name": "Opener", "linkedEntity": "Box",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "OPEN", "name": "Open", "payloadSchema": [{ "name": "program", "type": "object", "required": true }] }, { "key": "OPENED", "name": "Opened" }, { "key": "FAILED", "name": "Failed" }, { "key": "INSPECT", "name": "Inspect" }, { "key": "SEEN", "name": "Seen" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "OPEN", "effects": [["program/open", "@payload.program", { "route": "/", "declared": true, "integrations": "live" }, { "emit": { "success": "OPENED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "OPENED", "effects": [["set", "@entity.guest", "@payload.result.guest"]] },
                            { "from": "idle", "to": "idle", "event": "INSPECT", "effects": [["program/inputs", ask, { "emit": { "success": "SEEN", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "SEEN", "effects": [["set", "@entity.answer", ["json/stringify", "@payload.result"]]] },
                            { "from": "idle", "to": "idle", "event": "FAILED", "effects": [["set", "@entity.answer", ["str/concat", "refused: ", "@payload.error"]]] }
                        ]
                    }
                }]
            }]
        }))
        .unwrap();
        runtime.register_schema(opener).unwrap();
        runtime.dispatch("Opener", "default", "OPEN", serde_json::from_value(serde_json::json!({ "program": guest })).unwrap()).unwrap();
        runtime.dispatch("Opener", "default", "INSPECT", serde_json::from_value(serde_json::json!({})).unwrap()).unwrap();
        let entity = serde_json::to_value(&runtime.kernel.snapshot_trait("Opener", Some("default")).entity).unwrap();
        let answer = entity["answer"].as_str().unwrap_or_default();
        serde_json::from_str(answer).unwrap_or(serde_json::Value::String(answer.to_string()))
    }

    /// A program whose `Ticket` rows a driver answers after a keyed `os/read-file` settles in a later dispatch: with `replies`
    /// the deferred success replies, without it the driver stays silent.
    fn deferred_store_driver(replies: bool) -> orbital_core::OrbitalSchema {
        let settled: serde_json::Value = if replies {
            serde_json::json!([["emit", "ROW_FOUND", { "key": "@payload.key", "row": { "id": "t1", "title": "from the driver" } }]])
        } else {
            serde_json::json!([])
        };
        serde_json::from_value(serde_json::json!({
            "name": "deferred-store", "typeContractVersion": 1,
            "osGrants": { "fs": [{ "glob": "/etc/hosts", "modes": ["read"] }] },
            "orbitals": [
                { "name": "Tickets", "pages": [],
                  "entity": { "name": "Ticket", "persistence": "persistent", "collection": "tickets", "store": "Rows",
                      "fields": [{ "name": "id", "type": "string" }, { "name": "title", "type": "string" }] },
                  "traits": [{ "name": "Desk", "linkedEntity": "Ticket", "stateMachine": {
                      "states": [{ "name": "idle", "isInitial": true }],
                      "events": [{ "key": "LOOK", "name": "LOOK" }, { "key": "SEEN", "name": "SEEN" }, { "key": "UNSEEN", "name": "UNSEEN" }],
                      "transitions": [
                          { "from": "idle", "to": "idle", "event": "LOOK", "effects": [["fetch", "Ticket", { "id": "t1", "emit": { "success": "SEEN", "failure": "UNSEEN" } }]] },
                          { "from": "idle", "to": "idle", "event": "SEEN", "effects": [["set", "@entity.title", "seen"]] },
                          { "from": "idle", "to": "idle", "event": "UNSEEN", "effects": [["set", "@entity.title", ["str/concat", "unseen: ", "@payload.error"]]] }
                      ] } }] },
                { "name": "Rows", "pages": [],
                  "entity": { "name": "Mirror", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                  "traits": [{ "name": "RowsDriver", "linkedEntity": "Mirror", "stateMachine": {
                      "states": [{ "name": "ready", "isInitial": true }],
                      "events": [{ "key": "ROW_READ", "name": "ROW_READ" }, { "key": "FILE_READ", "name": "FILE_READ" }, { "key": "FILE_FAILED", "name": "FILE_FAILED" }],
                      "emits": [{ "event": "ROW_FOUND" }],
                      "transitions": [
                          { "from": "ready", "to": "ready", "event": "ROW_READ", "effects": [["os/read-file", "/etc/hosts", { "key": "@payload.key", "emit": { "success": "FILE_READ", "failure": "FILE_FAILED" } }]] },
                          { "from": "ready", "to": "ready", "event": "FILE_READ", "effects": settled },
                          { "from": "ready", "to": "ready", "event": "FILE_FAILED", "effects": [["emit", "STORE_FAILED", { "key": "@payload.key", "message": "@payload.error" }]] }
                      ] } }] }
            ]
        }))
        .unwrap()
    }

    /// A program whose `Ticket` row a driver answers at once, read by id only after a keyed `os/read-file` completes:
    /// the read runs in the completion's dispatch, as identity/sign-in does after a bearer's key fetch.
    fn read_after_a_completion() -> serde_json::Value {
        serde_json::json!({
            "name": "late-read", "typeContractVersion": 1,
            "osGrants": { "fs": [{ "glob": "/etc/hosts", "modes": ["read"] }] },
            "orbitals": [
                { "name": "Tickets",
                  "entity": { "name": "Ticket", "persistence": "persistent", "collection": "tickets", "store": "Rows",
                      "fields": [{ "name": "id", "type": "string" }, { "name": "title", "type": "string" }] },
                  "traits": [{ "name": "Desk", "linkedEntity": "Ticket", "stateMachine": {
                      "states": [{ "name": "idle", "isInitial": true }],
                      "events": [{ "key": "INIT", "name": "INIT" }, { "key": "LOOK", "name": "LOOK" }, { "key": "READY", "name": "READY" }, { "key": "SEEN", "name": "SEEN" }, { "key": "UNSEEN", "name": "UNSEEN" }, { "key": "FOUND", "name": "FOUND" }, { "key": "MISSING", "name": "MISSING" }],
                      "transitions": [
                          { "from": "idle", "to": "idle", "event": "INIT", "effects": [["render-ui", "main", { "type": "typography", "content": "desk" }]] },
                          { "from": "idle", "to": "idle", "event": "LOOK", "effects": [["os/read-file", "/etc/hosts", { "key": "late", "emit": { "success": "READY", "failure": "READY" } }]] },
                          { "from": "idle", "to": "idle", "event": "READY", "effects": [["fetch", "Ticket", { "id": "t1", "emit": { "success": "SEEN", "failure": "UNSEEN" } }]] },
                          { "from": "idle", "to": "idle", "event": "SEEN", "effects": [["emit", "FOUND", { "title": ["default", "@payload.data.title", "none"] }]] },
                          { "from": "idle", "to": "idle", "event": "UNSEEN", "effects": [["emit", "MISSING", { "error": "@payload.error" }]] }
                      ] } }],
                  "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Desk" }] }] },
                { "name": "Rows", "pages": [],
                  "entity": { "name": "Mirror", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                  "traits": [{ "name": "RowsDriver", "linkedEntity": "Mirror", "stateMachine": {
                      "states": [{ "name": "ready", "isInitial": true }],
                      "events": [{ "key": "ROW_READ", "name": "ROW_READ" }],
                      "emits": [{ "event": "ROW_FOUND" }],
                      "transitions": [
                          { "from": "ready", "to": "ready", "event": "ROW_READ", "effects": [["emit", "ROW_FOUND", { "key": "@payload.key", "row": { "id": "t1", "title": "from the driver" } }]] }
                      ] } }] }
            ]
        })
    }

    #[test]
    fn a_read_in_a_completions_dispatch_reaches_the_store_driver() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(serde_json::from_value(read_after_a_completion()).unwrap()).unwrap();
        runtime.tap_completions();
        runtime.dispatch("Desk", "default", "LOOK", serde_json::from_value(serde_json::json!({})).unwrap()).unwrap();
        let mut events = Vec::new();
        for _ in 0..100 {
            for completion in runtime.drain_completions() {
                let response = orbital_core::EventResponse::from_process_results(&completion.outcomes.unwrap_or_default(), "Desk", &completion.event);
                events.extend(response.emitted_events.iter().map(|e| serde_json::json!({ "event": e.event, "payload": serde_json::to_value(&e.payload).unwrap_or_default() })));
            }
            if !events.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let text = serde_json::to_string(&events).unwrap();
        assert!(text.contains("from the driver"), "a served host's completion read is answered by the driver: {text}");
    }

    #[test]
    fn a_guests_read_in_a_completions_dispatch_reaches_its_store_driver() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let opener: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Opener", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Opener",
                "entity": { "name": "Box", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "guest", "type": "string" }] },
                "osGrants": { "kernel": [{ "target": "guest", "kinds": ["spawn", "declared"] }], "fs": [{ "glob": "/etc/hosts", "modes": ["read"] }] },
                "traits": [{
                    "name": "Opener", "linkedEntity": "Box",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "OPEN", "name": "Open", "payloadSchema": [{ "name": "program", "type": "object", "required": true }] }, { "key": "OPENED", "name": "Opened", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "ANSWERED", "name": "Answered", "payloadSchema": [{ "name": "result", "type": "object" }] }, { "key": "FAILED", "name": "Failed", "payloadSchema": [{ "name": "error", "type": "string" }] }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "OPEN", "effects": [["program/open", "@payload.program", { "route": "/", "declared": true, "integrations": "live" }, { "emit": { "success": "OPENED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "OPENED", "effects": [["set", "@entity.guest", "@payload.result.guest"], ["program/dispatch", "LOOK", {}, { "guest": "@payload.result.guest", "emit": { "success": "ANSWERED", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "ANSWERED", "effects": [] },
                            { "from": "idle", "to": "idle", "event": "FAILED", "effects": [] }
                        ]
                    }
                }]
            }]
        }))
        .unwrap();
        runtime.register_schema(opener).unwrap();
        runtime.dispatch("Opener", "default", "OPEN", serde_json::from_value(serde_json::json!({ "program": read_after_a_completion() })).unwrap()).unwrap();
        let entity = serde_json::to_value(&runtime.kernel.snapshot_trait("Opener", Some("default")).entity).unwrap();
        let handle = entity["guest"].as_str().unwrap_or_default().to_string();
        let mut seen = Vec::new();
        for _ in 0..100 {
            if let Ok(orbital_core::runtime::kernel::GuestTicks::Painted(answer)) = runtime.kernel.guest_tick_pass(&handle) {
                seen.extend(serde_json::to_value(&answer).unwrap()["emitted"].as_array().cloned().unwrap_or_default());
            }
            if !seen.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let text = serde_json::to_string(&seen).unwrap();
        assert!(text.contains("from the driver"), "a live guest's completion read is answered by its driver: {text}");
    }

    /// `LOOK`'s result as the asking trait recorded it, failing rather than hanging when the dispatch never ends.
    fn look_through_a_deferred_driver(replies: bool) -> String {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
            runtime.register_schema(deferred_store_driver(replies)).unwrap();
            let outcome = runtime.dispatch("Desk", "default", "LOOK", serde_json::from_value(serde_json::json!({})).unwrap());
            let entity = serde_json::to_value(&runtime.kernel.snapshot_trait("Desk", Some("default")).entity).unwrap();
            tx.send(format!("{:?} {}", outcome.err().map(|e| e.to_string()), entity["title"])).unwrap();
        });
        rx.recv_timeout(std::time::Duration::from_secs(20)).expect("the dispatch parked on the driver's reply must end")
    }

    #[test]
    fn a_store_ask_parks_until_a_driver_replies_from_deferred_work() {
        let seen = look_through_a_deferred_driver(true);
        assert!(seen.contains("seen") && !seen.contains("unseen"), "{seen}");
    }

    #[test]
    fn control_a_driver_whose_deferred_work_ends_without_a_reply_fails_the_ask() {
        let seen = look_through_a_deferred_driver(false);
        assert!(seen.contains("sent no reply"), "{seen}");
    }

    #[test]
    fn a_guests_credentials_list_each_name_its_program_declares_masked_and_held_or_not() {
        let seen = guest_read(guest_with_secrets_and_a_refused_read(), serde_json::json!(["spawn", "declared"]), serde_json::json!({ "credentials": true }));
        let entries = seen["credentials"].as_array().unwrap_or_else(|| panic!("{seen}"));
        let of = |name: &str| entries.iter().find(|e| e["name"] == name).unwrap_or_else(|| panic!("{name} in {seen}"));
        assert_eq!(of("PATH")["set"], true, "{seen}");
        assert_eq!(of("ORB_SURELY_UNSET")["set"], false, "{seen}");
        assert_eq!(of("ORB_SURELY_UNSET")["tail"], "", "{seen}");
        assert!(of("PATH").get("value").is_none(), "a listing carries no value: {seen}");
    }

    #[test]
    fn a_guests_credential_value_is_revealed_only_by_name_and_only_when_declared() {
        let guest = guest_with_secrets_and_a_refused_read;
        let kinds = || serde_json::json!(["spawn", "declared"]);
        let seen = guest_read(guest(), kinds(), serde_json::json!({ "credentials": "PATH" }));
        assert_eq!(seen["credentials"]["value"], std::env::var("PATH").unwrap().as_str(), "{seen}");
        let refused = guest_read(guest(), kinds(), serde_json::json!({ "credentials": "HOME" }));
        assert!(refused.as_str().is_some_and(|text| text.starts_with("refused: ")), "{refused}");
    }

    #[test]
    fn control_an_opener_without_guest_declared_reads_no_credentials_or_denials() {
        for read in [serde_json::json!({ "credentials": true }), serde_json::json!({ "denials": true })] {
            let seen = guest_read(guest_with_secrets_and_a_refused_read(), serde_json::json!(["spawn"]), read);
            assert!(seen.as_str().is_some_and(|text| text.starts_with("refused: ")), "{seen}");
        }
    }

    #[test]
    fn a_guests_denied_operation_is_listed_even_when_its_program_handled_the_failure() {
        let seen = guest_read(guest_with_secrets_and_a_refused_read(), serde_json::json!(["spawn", "declared"]), serde_json::json!({ "denials": true }));
        let entries = seen["denials"].as_array().unwrap_or_else(|| panic!("{seen}"));
        assert_eq!(entries.len(), 1, "{seen}");
        assert!(entries[0]["message"].as_str().is_some_and(|m| m.contains("/orb-never-granted/a.txt")), "{seen}");
        assert!(entries[0]["count"].as_f64().is_some_and(|n| n >= 1.0), "{seen}");
    }

    #[test]
    fn control_a_guest_nothing_was_refused_lists_no_denials() {
        let guest = serde_json::json!({
            "name": "QuietGuest", "typeContractVersion": 1,
            "orbitals": [{
                "name": "Quiet",
                "entity": { "name": "Doc", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{ "name": "Shown", "linkedEntity": "Doc", "stateMachine": { "states": [{ "name": "idle", "isInitial": true }], "events": [{ "key": "INIT", "name": "Init" }], "transitions": [{ "from": "idle", "to": "idle", "event": "INIT" }] } }],
                "pages": [{ "name": "Home", "path": "/", "traits": [{ "ref": "Shown" }] }]
            }]
        });
        let seen = guest_read(guest, serde_json::json!(["spawn", "declared"]), serde_json::json!({ "denials": true }));
        assert_eq!(seen["denials"], serde_json::json!([]), "{seen}");
    }

    #[test]
    fn a_guests_environment_lists_each_declared_name_masked_with_its_effective_grants() {
        let path = std::env::var("PATH").unwrap();
        let tail: String = path.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
        let seen = guest_environment(serde_json::json!(["spawn", "declared"]), serde_json::json!(true));
        let entries = seen["environment"].as_array().unwrap_or_else(|| panic!("{seen}"));
        let of = |name: &str| entries.iter().find(|e| e["name"] == name).cloned().unwrap_or_else(|| panic!("{name}: {seen}"));
        assert_eq!(of("PATH")["set"], true);
        assert_eq!(of("PATH")["tail"], tail.as_str());
        assert!(of("PATH").get("value").is_none(), "the list never carries a value: {seen}");
        assert_eq!(of("ORB_SURELY_UNSET")["set"], false, "edge: a declared variable the host does not have is listed unset");
        assert_eq!(seen["grants"]["net"][0]["target"], "api.example.com:443", "the grants the guest really holds: {seen}");
    }

    #[test]
    fn asking_for_one_declared_name_reveals_its_value() {
        let seen = guest_environment(serde_json::json!(["spawn", "declared"]), serde_json::json!("PATH"));
        assert_eq!(seen["environment"]["value"], std::env::var("PATH").unwrap().as_str(), "{seen}");
    }

    #[test]
    fn edge_a_name_the_guest_does_not_declare_is_refused() {
        let seen = guest_environment(serde_json::json!(["spawn", "declared"]), serde_json::json!("HOME"));
        assert!(seen.as_str().is_some_and(|e| e.starts_with("refused:") && e.contains("does not declare")), "{seen}");
    }

    #[test]
    fn control_an_opener_without_guest_declared_cannot_read_a_guests_environment() {
        let seen = guest_environment(serde_json::json!(["spawn"]), serde_json::json!(true));
        assert!(seen.as_str().is_some_and(|e| e.starts_with("refused:") && e.contains("guest") && e.contains("declared")), "{seen}");
    }

    #[test]
    fn a_guest_host_with_mocked_integrations_holds_no_credential_the_process_environment_names() {
        use orbital_core::runtime::secret_source::SecretSource;
        let mocked = Host::new(Arc::new(DriverAdapter::memory())).holding_no_credentials();
        assert_eq!(mocked.credentials().secret_of("PATH", None).ok().flatten(), None);
        assert!(
            Host::new(Arc::new(DriverAdapter::memory())).credentials().secret_of("PATH", None).ok().flatten().is_some(),
            "control: a live guest host reads the opener's environment"
        );
    }

    #[test]
    fn a_hosted_guest_streams_the_paints_its_page_traits_client_ticks_make() {
        let items = guest_tick_items(ticking_guest_program(), std::time::Duration::from_millis(400));
        assert!(items.len() >= 2, "the guest's [client] tick fires on the guest's own clock and each pass that moved arrives (EXEC-ENDPOINT-3a, EXEC-DIST-14): {items:?}");
        assert!(items.iter().any(|i| i.contains("shown 2")), "each paint carries the ticked count: {items:?}");
        assert!(items.iter().all(|i| !i.contains("hidden")), "control: a [client] trait on a page the guest does not show never ticks (EXEC-TICK-6): {items:?}");
        assert!(items.iter().all(|i| i != "[]"), "edge: a pass whose ticks painted nothing (ShownSilent, every 5 ms) is no item: {items:?}");
        assert!(items.len() < 40, "edge: items come at the painting tick's pace (20 ms), not the silent one's: {}", items.len());
    }

    #[test]
    fn a_hosted_guest_mounts_its_page_after_the_deferred_write_its_mount_listener_made() {
        let answer = guest_shelf_open_answer(true);
        assert!(answer.contains("SEEN") && answer.contains("c1"), "the guest page listed the row its $MOUNT listener's self program/dispatch wrote (EXEC-CONC-7, EXEC-SEED-7): {answer}");
    }

    #[test]
    fn control_a_hosted_guest_with_no_mount_listener_lists_nothing() {
        let answer = guest_shelf_open_answer(false);
        assert!(answer.contains("SEEN") && !answer.contains("c1"), "the guest page mounted and listed nothing: {answer}");
    }

    /// The mount's answer, on a thread the test abandons after 20 s so a barrier that never settles fails instead of hanging.
    fn mounted_cards(runtime: &Arc<Host>, request: serde_json::Value) -> Vec<String> {
        let (tx, rx) = std::sync::mpsc::channel();
        let host = Arc::clone(runtime);
        std::thread::spawn(move || {
            let _ = tx.send(dispatch_request(&host, request));
        });
        let answer = rx.recv_timeout(std::time::Duration::from_secs(20)).expect("the mount settles").unwrap();
        let rows = answer["data"]["Card"].as_array().cloned().unwrap_or_default();
        rows.iter().filter_map(|r| r["id"].as_str().map(str::to_string)).collect()
    }

    #[test]
    fn a_route_mount_runs_its_seeds_after_the_deferred_asks_its_mount_listener_made() {
        let runtime = shelf_seeded_by_a_deferred_write();
        let cards = mounted_cards(&runtime, serde_json::json!({ "wire": true, "orbital": "Shelf", "event": "INIT", "clientId": "tab-s", "mount": [{ "trait": "Shelf", "event": "INIT" }], "route": "/shelf" }));
        assert_eq!(cards, ["c1"], "the page's INIT fetch sees the row the $MOUNT listener's deferred dispatch wrote (EXEC-SEED-7)");
    }

    #[test]
    fn control_a_route_less_mount_lists_before_any_seed() {
        let runtime = shelf_seeded_by_a_deferred_write();
        let cards = mounted_cards(&runtime, serde_json::json!({ "wire": true, "orbital": "Shelf", "event": "INIT", "clientId": "tab-t", "mount": [{ "trait": "Shelf", "event": "INIT" }] }));
        assert!(cards.is_empty(), "no route, no $MOUNT, nothing seeded: {cards:?}");
    }

    #[test]
    fn control_a_mount_request_naming_no_route_announces_nothing() {
        let runtime = desk_with_a_mount_listener();
        dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "event": "INIT", "clientId": "tab-n", "mount": [{ "trait": "Agent", "event": "INIT" }] })).unwrap();
        let _tab = runtime.kernel.enter_session("tab-n");
        let got = serde_json::to_value(&runtime.kernel.snapshot_trait("Seeder", Some(orbital_core::RUNTIME_SINGLETON)).entity).unwrap()["got"].clone();
        assert!(got.is_null() || got == "", "{got}");
    }

    #[test]
    fn a_request_addresses_its_entity_and_runs_a_mount_batch() {
        let runtime = desk();
        dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "targetTrait": "Notes", "event": "ADD_NOTE", "entityId": "n9", "viewer": { "id": "u1" }, "clientId": "tab-a" })).unwrap();
        {
            let _tab = runtime.kernel.enter_session("tab-a");
            assert_eq!(runtime.kernel.snapshot_trait("Notes", Some("n9")).state, "idle");
            assert_eq!(runtime.kernel.snapshot_trait("Notes", Some("n8")).state, "", "control: another entity was never addressed");
        }
        let mounted = dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "event": "INIT", "mount": [{ "trait": "Agent", "event": "INIT" }] })).unwrap();
        assert!(mounted.get("success").is_some(), "a mount batch answers one EventResponse: {mounted}");
    }

    #[test]
    fn a_nested_request_in_its_own_session_and_origin_leaves_the_askers_intact() {
        let runtime = desk();
        let seen = |runtime: &Host| runtime.kernel.thread_session().map(|s| s.to_string());
        let origin = |runtime: &Host| runtime.origin_by_thread.read().get(&std::thread::current().id()).cloned();
        {
            let _asker = runtime.kernel.enter_session("s-u1");
            let _tab = runtime.enter_origin(Some("tab-a"));
            dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "targetTrait": "Notes", "event": "ADD_NOTE", "viewer": { "id": "u2" }, "clientId": "tab-b" })).unwrap();
            assert_eq!(seen(&runtime).as_deref(), Some("s-u1"), "the asker's session is restored");
            assert_eq!(origin(&runtime).as_deref(), Some("tab-a"), "the asker's origin is restored");
            dispatch_request(&runtime, serde_json::json!({ "wire": true, "orbital": "Desk", "targetTrait": "Notes", "event": "ADD_NOTE" })).unwrap();
            assert_eq!(origin(&runtime).as_deref(), Some("tab-a"), "a request naming no clientId keeps the asker's origin");
        }
        assert_eq!(seen(&runtime), None, "control: with no asker the slot is empty again");
        assert_eq!(origin(&runtime), None, "control");
    }

    #[test]
    fn a_wire_stream_request_answers_its_items_in_execution_order() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        let schema: orbital_core::OrbitalSchema = serde_json::from_value(serde_json::json!({
            "name": "Board",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Board",
                "entity": { "name": "Tile", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{
                    "name": "Tiles", "linkedEntity": "Tile",
                    "emits": [{ "event": "PONG" }],
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "PING", "name": "Ping" }, { "key": "PONG", "name": "Pong" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "PING", "effects": [
                                ["render-ui", "main", { "type": "stack", "children": [] }],
                                ["emit", "PONG", { "n": 1 }]] },
                            { "from": "idle", "to": "idle", "event": "PONG" }]
                    }
                }]
            }]
        }))
        .unwrap();
        runtime.register_schema(schema).unwrap();
        let base = serde_json::json!({ "wire": true, "orbital": "Board", "targetTrait": "Tiles", "event": "PING" });
        let mut streamed = base.clone();
        streamed["stream"] = serde_json::json!(true);
        let answer = dispatch_request(&runtime, streamed).unwrap();
        let kinds: Vec<&str> = answer["items"].as_array().unwrap().iter().filter_map(|i| i["type"].as_str()).collect();
        assert_eq!(kinds, ["effect", "event", "complete"], "declaration order, then the response: {answer}");
        assert_eq!(answer["items"][1]["data"]["event"], "PONG");
        assert_eq!(answer["items"][2]["data"]["success"], answer["success"], "complete carries the response");
        assert!(dispatch_request(&runtime, base).unwrap().get("items").is_none(), "control: no items without stream");
    }

    fn scratch_dir() -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("orbital-deferred-reader-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    fn deferred_reader(dir: &std::path::Path) -> orbital_core::OrbitalSchema {
        let path = dir.join("note.txt");
        std::fs::write(&path, "hello").unwrap();
        serde_json::from_value(serde_json::json!({
            "name": "Reader",
            "osGrants": { "fs": [{ "glob": format!("{}/**", dir.display()), "modes": ["read"] }] },
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Reader",
                "entity": { "name": "Doc", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{
                    "name": "Files", "linkedEntity": "Doc",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "READ", "name": "Read" }, { "key": "READ_DONE", "name": "Read done" },
                            { "key": "READ_FAILED", "name": "Read failed" }, { "key": "SEEN", "name": "Seen" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "READ", "effects": [["os/read-file", path.to_str().unwrap(), { "key": "r1", "emit": { "success": "READ_DONE", "failure": "READ_FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "READ_DONE", "effects": [["emit", "SEEN", { "text": "@payload.text" }]] },
                            { "from": "idle", "to": "idle", "event": "READ_FAILED", "effects": [["emit", "SEEN", { "text": "@payload.error" }]] }
                        ]
                    }
                }]
            }]
        }))
        .expect("the program parses")
    }

    fn items_of(live: &mut broadcast::Receiver<LiveBroadcastItem>, event: &str) -> Vec<LiveBroadcastItem> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut seen = Vec::new();
        while std::time::Instant::now() < deadline {
            match live.try_recv() {
                Ok(item) if item.event == event => seen.push(item),
                Ok(_) => {}
                Err(_) if !seen.is_empty() => break,
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
            }
        }
        seen
    }

    #[test]
    fn a_deferred_read_completion_goes_to_the_tab_that_deferred_it_as_a_dispatch_it_folds() {
        let dir = scratch_dir();
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(deferred_reader(&dir)).unwrap();
        let mut live = runtime.subscribe_live();
        let answer = dispatch_request(&runtime, serde_json::json!({ "orbital": "Reader", "targetTrait": "Files", "event": "READ", "clientId": "tab-1" })).unwrap();
        assert!(answer["emittedEvents"].as_array().is_none_or(|e| e.iter().all(|e| e["event"] != "SEEN")), "the read is deferred: {answer}");
        let items = items_of(&mut live, "READ_DONE");
        assert_eq!(items.len(), 1, "{items:?}");
        assert!(items[0].delivered_to(Some("tab-1"), |_| false));
        assert!(!items[0].delivered_to(Some("tab-2"), |_| true), "control: another tab does not get it");
        let frame = serde_json::to_value(live_item_value(&items[0])).unwrap();
        assert_eq!(frame["type"], "dispatch");
        assert_eq!(frame["orbitalName"], "Reader");
        assert_eq!(frame["request"]["targetTrait"], "Files");
        assert_eq!(frame["request"]["event"], "READ_DONE");
        assert_eq!(frame["response"]["states"]["Files"], "idle", "the completion's states fold on the tab: {frame}");
    }

    #[test]
    fn control_a_deferred_completions_emits_are_not_pushed_to_be_run_again() {
        let dir = scratch_dir();
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(deferred_reader(&dir)).unwrap();
        let mut live = runtime.subscribe_live();
        dispatch_request(&runtime, serde_json::json!({ "orbital": "Reader", "targetTrait": "Files", "event": "READ", "clientId": "tab-1" })).unwrap();
        assert!(items_of(&mut live, "SEEN").is_empty(), "SEEN already ran on the server; it is inside the dispatch item");
    }

    #[test]
    fn control_a_deferred_read_with_no_requesting_tab_pushes_to_no_one() {
        let dir = scratch_dir();
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(deferred_reader(&dir)).unwrap();
        let mut live = runtime.subscribe_live();
        dispatch_request(&runtime, serde_json::json!({ "orbital": "Reader", "targetTrait": "Files", "event": "READ" })).unwrap();
        assert!(items_of(&mut live, "READ_DONE").is_empty());
    }
    fn deferred_counter_reader(dir: &std::path::Path, done_rewrites_list: bool) -> orbital_core::OrbitalSchema {
        let mut program = serde_json::to_value(deferred_reader(dir)).unwrap();
        let items: Vec<serde_json::Value> = (0..50).map(|i| serde_json::json!({ "k": format!("item-{i}") })).collect();
        let mut done = vec![serde_json::json!(["set", "@entity.count", ["+", "@entity.count", 1]]), serde_json::json!(["emit", "SEEN", { "text": "@payload.text" }])];
        if done_rewrites_list {
            done.insert(0, serde_json::json!(["set", "@entity.items", "@entity.items"]));
        }
        let orbital = &mut program["orbitals"][0];
        orbital["entity"]["fields"] = serde_json::json!([
            { "name": "id", "type": "string" },
            { "name": "count", "type": "number", "default": 0 },
            { "name": "items", "type": "array", "default": items }
        ]);
        orbital["traits"][0]["stateMachine"]["transitions"][1]["effects"] = serde_json::Value::Array(done);
        // The page shows both fields, so a frame carries them (EXEC-DIST-7: a frame ships only what the browser reads).
        orbital["traits"][0]["stateMachine"]["transitions"][0]["effects"].as_array_mut().unwrap().insert(0, serde_json::json!(["render-ui", "main", { "type": "stack", "children": [
            { "type": "typography", "content": "@entity.count" }, { "type": "data-list", "items": "@entity.items" }] }]));
        serde_json::from_value(program).expect("the program parses")
    }

    fn tool_painter() -> orbital_core::OrbitalSchema {
        let machine = |states: serde_json::Value, events: serde_json::Value, transitions: serde_json::Value| serde_json::json!({ "states": states, "events": events, "transitions": transitions });
        serde_json::from_value(serde_json::json!({
            "name": "Desk",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Desk",
                "entity": { "name": "Card", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "text", "type": "string" }] },
                "traits": [{
                    "name": "Agent", "linkedEntity": "Card",
                    "stateMachine": machine(
                        serde_json::json!([{ "name": "idle", "isInitial": true }]),
                        serde_json::json!([{ "key": "GO", "name": "Go" }, { "key": "DONE", "name": "Done" }, { "key": "FAILED", "name": "Failed" }]),
                        serde_json::json!([
                            { "from": "idle", "to": "idle", "event": "GO", "effects": [
                                ["program/dispatch", "Panel.SHOW", { "text": "asked by the tool" }, { "key": "t1", "emit": { "success": "DONE", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "DONE", "effects": [] },
                            { "from": "idle", "to": "idle", "event": "FAILED", "effects": [] }]))
                }, {
                    "name": "Panel", "linkedEntity": "Card",
                    "stateMachine": machine(
                        serde_json::json!([{ "name": "hidden", "isInitial": true }, { "name": "shown" }]),
                        serde_json::json!([{ "key": "SHOW", "name": "Show", "input": true, "payloadSchema": [{ "name": "text", "type": "string", "required": true }] }]),
                        serde_json::json!([{ "from": "hidden", "to": "shown", "event": "SHOW",
                            "effects": [["set", "@entity.text", "@payload.text"], ["render-ui", "main", { "type": "typography", "content": "@payload.text" }]] }]))
                }]
            }]
        }))
        .expect("the program parses")
    }

    fn tool_painter_asked_by(client: Option<&str>) -> (Arc<Host>, broadcast::Receiver<LiveBroadcastItem>) {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(tool_painter()).unwrap();
        let live = runtime.subscribe_live();
        let mut request = serde_json::json!({ "orbital": "Desk", "targetTrait": "Agent", "event": "GO" });
        if let Some(client) = client {
            request["clientId"] = serde_json::json!(client);
        }
        dispatch_request(&runtime, request).unwrap();
        (runtime, live)
    }

    #[test]
    fn a_tool_dispatch_goes_to_the_asking_tab_as_a_dispatch_it_folds() {
        let (_runtime, mut live) = tool_painter_asked_by(Some("tab-1"));
        let items = items_of(&mut live, "SHOW");
        assert_eq!(items.len(), 1, "{items:?}");
        assert!(items[0].delivered_to(Some("tab-1"), |_| false));
        assert!(!items[0].delivered_to(Some("tab-2"), |_| true), "control: another tab does not get it");
        let frame = serde_json::to_value(live_item_value(&items[0])).unwrap();
        assert_eq!(frame["type"], "dispatch");
        assert_eq!(frame["orbitalName"], "Desk");
        assert_eq!(frame["request"]["targetTrait"], "Panel");
        assert_eq!(frame["request"]["event"], "SHOW");
        assert_eq!(frame["response"]["states"]["Panel"], "shown", "{frame}");
        assert!(frame["response"]["clientEffectsByTrait"].to_string().contains("asked by the tool"), "the tab is sent what the tool painted: {frame}");
    }

    #[test]
    fn control_a_tool_dispatch_with_no_asking_tab_pushes_to_no_one() {
        let (_runtime, mut live) = tool_painter_asked_by(None);
        assert!(items_of(&mut live, "SHOW").is_empty());
    }

    #[test]
    fn control_the_askers_own_completion_is_still_pushed_once_beside_the_tool_dispatch() {
        let (_runtime, mut live) = tool_painter_asked_by(Some("tab-1"));
        assert_eq!(items_of(&mut live, "DONE").len(), 1);
    }

    fn tool_that_reads_later(dir: &std::path::Path) -> orbital_core::OrbitalSchema {
        let path = dir.join("note.txt");
        std::fs::write(&path, "hello").unwrap();
        let machine = |states: serde_json::Value, events: serde_json::Value, transitions: serde_json::Value| serde_json::json!({ "states": states, "events": events, "transitions": transitions });
        serde_json::from_value(serde_json::json!({
            "name": "Desk",
            "osGrants": { "fs": [{ "glob": format!("{}/**", dir.display()), "modes": ["read"] }] },
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Desk",
                "entity": { "name": "Card", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "text", "type": "string" }] },
                "traits": [{
                    "name": "Agent", "linkedEntity": "Card",
                    "stateMachine": machine(
                        serde_json::json!([{ "name": "idle", "isInitial": true }]),
                        serde_json::json!([{ "key": "GO", "name": "Go" }, { "key": "DONE", "name": "Done" }, { "key": "FAILED", "name": "Failed" }]),
                        serde_json::json!([
                            { "from": "idle", "to": "idle", "event": "GO", "effects": [
                                ["program/dispatch", "Panel.SHOW", {}, { "key": "t1", "emit": { "success": "DONE", "failure": "FAILED" } }]] },
                            { "from": "idle", "to": "idle", "event": "DONE", "effects": [] },
                            { "from": "idle", "to": "idle", "event": "FAILED", "effects": [] }]))
                }, {
                    "name": "Panel", "linkedEntity": "Card",
                    "stateMachine": machine(
                        serde_json::json!([{ "name": "hidden", "isInitial": true }, { "name": "reading" }, { "name": "shown" }]),
                        serde_json::json!([{ "key": "SHOW", "name": "Show", "input": true }, { "key": "READ_DONE", "name": "Read done" }, { "key": "READ_FAILED", "name": "Read failed" },
                            { "key": "PAINT_ASKED", "name": "Paint asked" }, { "key": "PAINT_REFUSED", "name": "Paint refused" }]),
                        serde_json::json!([
                            { "from": "hidden", "to": "reading", "event": "SHOW", "effects": [["os/read-file", path.to_str().unwrap(), { "key": "r1", "emit": { "success": "READ_DONE", "failure": "READ_FAILED" } }]] },
                            { "from": "reading", "to": "shown", "event": "READ_DONE", "effects": [["render-ui", "main", { "type": "typography", "content": "read after the tool" }],
                                ["program/dispatch", "Late.PAINT", {}, { "key": "t2", "emit": { "success": "PAINT_ASKED", "failure": "PAINT_REFUSED" } }]] },
                            { "from": "shown", "to": "shown", "event": "PAINT_ASKED", "effects": [] },
                            { "from": "shown", "to": "shown", "event": "PAINT_REFUSED", "effects": [] },
                            { "from": "reading", "to": "hidden", "event": "READ_FAILED", "effects": [] }]))
                }, {
                    "name": "Late", "linkedEntity": "Card",
                    "stateMachine": machine(
                        serde_json::json!([{ "name": "idle", "isInitial": true }, { "name": "reading" }, { "name": "painted" }]),
                        serde_json::json!([{ "key": "PAINT", "name": "Paint", "input": true }, { "key": "LATE_DONE", "name": "Late done" }, { "key": "LATE_FAILED", "name": "Late failed" }]),
                        serde_json::json!([
                            { "from": "idle", "to": "reading", "event": "PAINT", "effects": [["os/read-file", path.to_str().unwrap(), { "key": "r2", "emit": { "success": "LATE_DONE", "failure": "LATE_FAILED" } }]] },
                            { "from": "reading", "to": "painted", "event": "LATE_DONE", "effects": [["render-ui", "main", { "type": "typography", "content": "painted two hops later" }]] },
                            { "from": "reading", "to": "idle", "event": "LATE_FAILED", "effects": [] }]))
                }]
            }]
        }))
        .expect("the program parses")
    }

    fn later_read_asked_by(client: Option<&str>) -> Vec<LiveBroadcastItem> {
        later_event_asked_by(client, "READ_DONE")
    }

    fn later_event_asked_by(client: Option<&str>, event: &str) -> Vec<LiveBroadcastItem> {
        let dir = scratch_dir();
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(tool_that_reads_later(&dir)).unwrap();
        let mut live = runtime.subscribe_live();
        let mut request = serde_json::json!({ "orbital": "Desk", "targetTrait": "Agent", "event": "GO" });
        if let Some(client) = client {
            request["clientId"] = serde_json::json!(client);
        }
        dispatch_request(&runtime, request).unwrap();
        items_of(&mut live, event)
    }

    #[test]
    fn a_tool_dispatched_from_a_completion_still_completes_on_the_asking_tab() {
        let items = later_event_asked_by(Some("tab-1"), "LATE_DONE");
        assert_eq!(items.len(), 1, "{items:?}");
        assert!(items[0].delivered_to(Some("tab-1"), |_| false));
        assert!(!items[0].delivered_to(Some("tab-2"), |_| true), "control: another tab does not get it");
        let frame = serde_json::to_value(live_item_value(&items[0])).unwrap();
        assert!(frame["response"]["clientEffectsByTrait"].to_string().contains("painted two hops later"), "{frame}");
    }

    #[test]
    fn control_a_tool_dispatched_from_a_completion_with_no_asking_tab_pushes_to_no_one() {
        assert!(later_event_asked_by(None, "LATE_DONE").is_empty());
    }

    #[test]
    fn what_a_tool_dispatch_defers_completes_on_the_asking_tab() {
        let items = later_read_asked_by(Some("tab-1"));
        assert_eq!(items.len(), 1, "{items:?}");
        assert!(items[0].delivered_to(Some("tab-1"), |_| false));
        assert!(!items[0].delivered_to(Some("tab-2"), |_| true), "control: another tab does not get it");
        let frame = serde_json::to_value(live_item_value(&items[0])).unwrap();
        assert_eq!(frame["response"]["states"]["Panel"], "shown", "{frame}");
        assert!(frame["response"]["clientEffectsByTrait"].to_string().contains("read after the tool"), "the tab is sent what the completion painted: {frame}");
    }

    #[test]
    fn control_what_a_tool_dispatch_defers_with_no_asking_tab_pushes_to_no_one() {
        assert!(later_read_asked_by(None).is_empty());
    }

    fn completion_row(done_rewrites_list: bool) -> serde_json::Value {
        let dir = scratch_dir();
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(deferred_counter_reader(&dir, done_rewrites_list)).unwrap();
        let mut live = runtime.subscribe_live();
        dispatch_request(&runtime, serde_json::json!({ "orbital": "Reader", "targetTrait": "Files", "event": "READ", "clientId": "tab-1" })).unwrap();
        let items = items_of(&mut live, "READ_DONE");
        assert_eq!(items.len(), 1, "{items:?}");
        let frame = serde_json::to_value(live_item_value(&items[0])).unwrap();
        frames_in(&frame["response"])["Files"].clone()
    }

    /// The frames a response's delta leaves a client that held none: its ops folded.
    fn frames_in(response: &serde_json::Value) -> serde_json::Value {
        let mut frames = serde_json::Map::new();
        for op in response["frames"]["ops"].as_array().into_iter().flatten() {
            let (name, frame) = (op[0].as_str().unwrap(), op[1].as_str().unwrap_or_default());
            match name {
                "clear" => frames.clear(),
                "del" => {
                    frames.remove(frame);
                }
                "put" => {
                    frames.insert(frame.to_string(), op[2].clone());
                }
                "set" => {
                    for (name, fields) in op[1].as_object().unwrap() {
                        let held = frames.entry(name.clone()).or_insert_with(|| serde_json::json!({}));
                        for (field, value) in fields.as_object().unwrap() {
                            held[field] = value.clone();
                        }
                    }
                }
                other => panic!("unknown frame op {other}"),
            }
        }
        serde_json::Value::Object(frames)
    }

    /// The snapshot the host answers a client that asks for its session's frames from nothing.
    fn view_of(runtime: &Host, orbital: &str, trait_name: &str, tab: &str) -> serde_json::Value {
        let options = serde_json::json!({ "orbital": orbital, "targetTrait": trait_name, "event": "VIEW", "clientId": tab, "framesSince": 0 });
        let args: Vec<Value> = [serde_json::json!(format!("{orbital}.{trait_name}.VIEW")), serde_json::json!({}), options]
            .iter()
            .map(|a| serde_json::from_value(a.clone()).unwrap())
            .collect();
        let answer = orbital_core::effects::executor::SubstrateHandler::run(runtime.substrate.as_ref(), "program/dispatch", &args).expect("the host answers a frames request");
        frames_in(&orbital_core::json_value::value_to_json(&answer))
    }

    #[test]
    fn a_deferred_completion_that_only_writes_a_counter_pushes_a_row_without_the_held_list() {
        let row = completion_row(false);
        assert!(row.get("count").is_some(), "{row}");
        assert!(row.get("items").is_none(), "the unchanged list rode the pushed dispatch: {row}");
    }

    #[test]
    fn control_a_deferred_completion_that_rewrites_the_list_pushes_it_to_a_client_never_sent_it() {
        let row = completion_row(true);
        assert_eq!(row["items"].as_array().map(Vec::len), Some(50), "{row}");
    }
    fn topic_program() -> orbital_core::OrbitalSchema {
        serde_json::from_value(serde_json::json!({
            "name": "Topics",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Topics",
                "entity": { "name": "Topic", "persistence": "runtime", "fields": [
                    { "name": "id", "type": "string" }, { "name": "topic", "type": "string" }, { "name": "seen", "type": "string" }] },
                "traits": [{
                    "name": "Viewer", "linkedEntity": "Topic",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "SET_TOPIC", "name": "Set topic", "payloadSchema": [{ "name": "topic", "type": "string", "required": true }] },
                            { "key": "READ_BACK", "name": "Read back" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "SET_TOPIC", "effects": [["set", "@entity.topic", "@payload.topic"]] },
                            { "from": "idle", "to": "idle", "event": "READ_BACK", "effects": [["set", "@entity.seen", "@entity.topic"]] }
                        ]
                    }
                }]
            }]
        }))
        .expect("the program parses")
    }

    fn topic_dispatch(runtime: &Host, event: &str, topic: Option<&str>, tab: &str) -> serde_json::Value {
        let options = as_signed_in(runtime, serde_json::json!({ "orbital": "Topics", "targetTrait": "Viewer", "event": event, "clientId": tab, "viewer": { "id": tab } }));
        let args: Vec<Value> = [serde_json::json!(format!("Topics.Viewer.{event}")), serde_json::json!(topic.map_or(serde_json::json!({}), |t| serde_json::json!({ "topic": t }))), options]
            .iter()
            .map(|a| serde_json::from_value(a.clone()).unwrap())
            .collect();
        let answer = orbital_core::effects::executor::SubstrateHandler::run(runtime.substrate.as_ref(), "program/dispatch", &args).expect("the dispatch runs");
        orbital_core::json_value::value_to_json(&answer)
    }

    fn store_bound_program() -> orbital_core::OrbitalSchema {
        serde_json::from_value(serde_json::json!({ "name": "store-seam", "typeContractVersion": 1, "orbitals": [
            { "name": "Tickets", "pages": [],
              "entity": { "name": "Ticket", "persistence": "persistent", "store": "Rows", "fields": [{ "name": "id", "type": "string" }, { "name": "used", "type": "boolean" }] },
              "traits": [{ "name": "Desk", "linkedEntity": "Ticket",
                "stateMachine": { "states": [{ "name": "idle", "isInitial": true }],
                    "events": [{ "key": "LOOK", "name": "LOOK" }, { "key": "ADD", "name": "ADD" }],
                    "transitions": [{ "from": "idle", "to": "idle", "event": "LOOK", "effects": [["fetch", "Ticket", { "id": "@payload.id" }]] },
                        { "from": "idle", "to": "idle", "event": "ADD", "effects": [["persist", "create", "Ticket", { "id": "@payload.id", "used": false }]] }] } }] },
            { "name": "Rows", "pages": [],
              "entity": { "name": "Mirror", "persistence": "persistent", "fields": [{ "name": "id", "type": "string" }] },
              "traits": [{ "name": "RowsDriver", "linkedEntity": "Mirror",
                "stateMachine": { "states": [{ "name": "ready", "isInitial": true }],
                    "events": [{ "key": "ROW_READ", "name": "ROW_READ" }, { "key": "ROWS_WRITE", "name": "ROWS_WRITE" }],
                    "transitions": [{ "from": "ready", "to": "ready", "event": "ROWS_WRITE",
                        "effects": [["persist", "create", "Mirror", { "id": "@payload.key" }], ["emit", "ROWS_WRITTEN", { "key": "@payload.key", "outcomes": [{ "kind": "created", "row": { "id": "t2", "used": false } }] }]] },
                        { "from": "ready", "to": "ready", "event": "ROW_READ",
                        "effects": [["persist", "create", "Mirror", { "id": "@payload.key" }], ["emit", "ROW_FOUND", { "key": "@payload.key", "row": { "id": "t1", "used": false } }]] }] } }] }
        ] })).expect("the program parses")
    }

    fn store_event(runtime: &Host, event: &str, id: &str) -> serde_json::Value {
        let options = serde_json::json!({ "orbital": "Tickets", "targetTrait": "Desk", "event": event, "clientId": "tab-1" });
        let args: Vec<Value> = [serde_json::json!(format!("Tickets.Desk.{event}")), serde_json::json!({ "id": id }), options]
            .iter()
            .map(|a| serde_json::from_value(a.clone()).unwrap())
            .collect();
        let answer = orbital_core::effects::executor::SubstrateHandler::run(runtime.substrate.as_ref(), "program/dispatch", &args).expect("the dispatch runs");
        orbital_core::json_value::value_to_json(&answer)
    }

    #[test]
    fn a_served_dispatch_reads_a_store_bound_row_through_its_driver() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(store_bound_program()).unwrap();
        let read = store_event(&runtime, "LOOK", "t1");
        let results = read["effectResults"].as_array().cloned().unwrap_or_default();
        let fetch = results.iter().find(|r| r["effect"] == "fetch").unwrap_or_else(|| panic!("no fetch result: {read}"));
        assert_eq!(fetch["success"], true, "{read}");
        assert_eq!(fetch["data"][0]["id"], "t1", "{read}");
    }

    #[test]
    fn a_served_dispatch_creates_a_store_bound_row_through_its_driver() {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(store_bound_program()).unwrap();
        let wrote = store_event(&runtime, "ADD", "t2");
        let persist = wrote["effectResults"].as_array().and_then(|r| r.iter().find(|e| e["effect"] == "persist")).unwrap_or_else(|| panic!("no persist result: {wrote}"));
        assert_eq!(persist["success"], true, "{wrote}");
    }

    fn keyed_store_look() -> (String, serde_json::Value) {
        let mut program = serde_json::to_value(store_bound_program()).unwrap();
        program["orbitals"].as_array_mut().unwrap().push(serde_json::json!({
            "name": "Caller", "pages": [],
            "entity": { "name": "Run", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }, { "name": "got", "type": "string" }] },
            "traits": [{ "name": "Route", "linkedEntity": "Run",
                "stateMachine": { "states": [{ "name": "idle", "isInitial": true }, { "name": "done" }, { "name": "failed" }],
                    "events": [{ "key": "GO", "name": "GO" }, { "key": "DONE", "name": "DONE" }, { "key": "FAILED", "name": "FAILED" }],
                    "transitions": [
                        { "from": "idle", "to": "idle", "event": "GO", "effects": [["program/dispatch", "Tickets.Desk.LOOK", { "id": "t1" },
                            { "orbital": "Tickets", "targetTrait": "Desk", "event": "LOOK", "clientId": "tab-1", "wire": true },
                            { "key": "look-1", "emit": { "success": "DONE", "failure": "FAILED" } }]] },
                        { "from": "idle", "to": "done", "event": "DONE", "effects": [["set", "@entity.got", ["json/stringify", "@payload.result"]]] },
                        { "from": "idle", "to": "failed", "event": "FAILED", "effects": [["set", "@entity.got", "@payload.error"]] }] } }]
        }));
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(serde_json::from_value(program).unwrap()).unwrap();
        runtime.dispatch_continuing("Route", "default", "GO", Value::Null, None, None).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let snapshot = runtime.kernel.snapshot_trait("Route", Some("default"));
            if snapshot.state != "idle" {
                let got = serde_json::to_value(&snapshot.entity).unwrap()["got"].as_str().unwrap_or_default().to_string();
                return (snapshot.state, serde_json::from_str(&got).unwrap_or(serde_json::Value::String(got)));
            }
            assert!(std::time::Instant::now() < deadline, "the keyed dispatch never completed");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[test]
    fn a_keyed_served_dispatch_reads_a_store_bound_row_through_its_driver() {
        let (state, answer) = keyed_store_look();
        assert_eq!(state, "done", "{answer}");
        let fetch = answer["effectResults"].as_array().and_then(|r| r.iter().find(|e| e["effect"] == "fetch")).unwrap_or_else(|| panic!("no fetch result: {answer}"));
        assert_eq!(fetch["success"], true, "{answer}");
        let put = serde_json::json!(["put", "Desk", { "id": "t1" }]);
        assert!(answer["frames"]["ops"].as_array().is_some_and(|ops| ops.contains(&put)), "a wire answer carries the row in its frames: {answer}");
    }

    fn topic_host() -> Arc<Host> {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(topic_program()).unwrap();
        runtime
    }

    #[test]
    fn a_tabs_server_trait_frame_is_not_overwritten_by_another_tabs_dispatch() {
        let runtime = topic_host();
        topic_dispatch(&runtime, "SET_TOPIC", Some("app"), "tab-a");
        topic_dispatch(&runtime, "SET_TOPIC", Some("cli"), "tab-b");
        topic_dispatch(&runtime, "READ_BACK", None, "tab-a");
        let read = view_of(&runtime, "Topics", "Viewer", "tab-a");
        assert_eq!(read["Viewer"]["seen"], "app", "tab-b's write reached tab-a's frame: {read}");
    }

    #[test]
    fn control_one_tab_reads_back_its_own_last_write() {
        let runtime = topic_host();
        topic_dispatch(&runtime, "SET_TOPIC", Some("app"), "tab-a");
        topic_dispatch(&runtime, "READ_BACK", None, "tab-a");
        let read = view_of(&runtime, "Topics", "Viewer", "tab-a");
        assert_eq!(read["Viewer"]["seen"], "app", "{read}");
    }
    fn handoff_program() -> orbital_core::OrbitalSchema {
        let machine = |events: serde_json::Value, arms: serde_json::Value| serde_json::json!({ "states": [{ "name": "idle", "isInitial": true }], "events": events, "transitions": arms });
        serde_json::from_value(serde_json::json!({
            "name": "Handoff",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Handoff",
                "entity": { "name": "Job", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [
                    { "name": "Source", "linkedEntity": "Job", "stateMachine": machine(
                        serde_json::json!([{ "key": "GO", "name": "Go" }]),
                        serde_json::json!([{ "from": "idle", "to": "idle", "event": "GO", "effects": [
                            ["emit", "HANDOFF", { "blob": "server-only-payload" }], ["emit", "PAINT", { "label": "for-the-client" }]] }])) },
                    { "name": "Sink", "linkedEntity": "Job",
                      "listens": [{ "event": "HANDOFF", "triggers": "TAKE", "source": { "kind": "trait", "trait": "Source" } }],
                      "stateMachine": machine(serde_json::json!([{ "key": "TAKE", "name": "Take" }]), serde_json::json!([{ "from": "idle", "to": "idle", "event": "TAKE", "effects": [] }])) },
                    { "name": "Painter", "linkedEntity": "Job", "dispatch": "client",
                      "listens": [{ "event": "PAINT", "triggers": "SHOW", "source": { "kind": "trait", "trait": "Source" } }],
                      "stateMachine": machine(serde_json::json!([{ "key": "SHOW", "name": "Show" }]), serde_json::json!([{ "from": "idle", "to": "idle", "event": "SHOW", "effects": [] }])) }
                ]
            }]
        }))
        .expect("the program parses")
    }

    fn handoff_answer(wire: bool) -> Vec<String> {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(handoff_program()).unwrap();
        let answer = dispatch_request(&runtime, serde_json::json!({ "wire": wire, "orbital": "Handoff", "targetTrait": "Source", "event": "GO" })).unwrap();
        answer["emittedEvents"].as_array().unwrap().iter().map(|e| e["event"].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn a_wire_answer_drops_an_emit_only_server_traits_listen_to() {
        let emitted = handoff_answer(true);
        assert!(!emitted.contains(&"HANDOFF".to_string()), "{emitted:?}");
    }

    #[test]
    fn control_a_wire_answer_keeps_an_emit_a_client_trait_listens_to() {
        assert!(handoff_answer(true).contains(&"PAINT".to_string()));
    }

    #[test]
    fn control_a_request_that_is_not_for_a_browser_keeps_every_emit() {
        let emitted = handoff_answer(false);
        assert!(emitted.contains(&"HANDOFF".to_string()) && emitted.contains(&"PAINT".to_string()), "{emitted:?}");
    }

    #[test]
    fn a_pushed_server_trait_completion_drops_its_server_only_emits_and_its_payload() {
        let dir = scratch_dir();
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(deferred_reader(&dir)).unwrap();
        let mut live = runtime.subscribe_live();
        dispatch_request(&runtime, serde_json::json!({ "orbital": "Reader", "targetTrait": "Files", "event": "READ", "clientId": "tab-1" })).unwrap();
        let items = items_of(&mut live, "READ_DONE");
        assert_eq!(items.len(), 1, "{items:?}");
        let frame = serde_json::to_value(live_item_value(&items[0])).unwrap();
        assert!(frame["request"].get("payload").is_none(), "the completion payload rode the push: {frame}");
        assert!(frame["response"]["emittedEvents"].as_array().is_none_or(|e| e.iter().all(|e| e["event"] != "SEEN")), "SEEN has no client listener: {frame}");
    }
    fn card_program(persistence: &str) -> orbital_core::OrbitalSchema {
        serde_json::from_value(serde_json::json!({
            "name": "Cards",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Cards",
                "entity": { "name": "Card", "persistence": persistence, "fields": [{ "name": "id", "type": "string" }, { "name": "label", "type": "string" }] },
                "traits": [{
                    "name": "Deck", "linkedEntity": "Card",
                    "stateMachine": {
                        "states": [{ "name": "idle", "isInitial": true }],
                        "events": [{ "key": "SAVE", "name": "Save" }, { "key": "LIST", "name": "List" }, { "key": "L", "name": "Listed" }],
                        "transitions": [
                            { "from": "idle", "to": "idle", "event": "SAVE", "effects": [["persist", "create", "Card", { "id": "c1", "label": "x" }]] },
                            { "from": "idle", "to": "idle", "event": "LIST", "effects": [["fetch", "Card", { "emit": { "success": "L" } }]] }
                        ]
                    }
                }]
            }]
        }))
        .expect("the program parses")
    }

    fn listed_card_ids(persistence: &str) -> Vec<String> {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(card_program(persistence)).unwrap();
        dispatch_request(&runtime, serde_json::json!({ "orbital": "Cards", "targetTrait": "Deck", "event": "SAVE" })).unwrap();
        let listed = dispatch_request(&runtime, serde_json::json!({ "orbital": "Cards", "targetTrait": "Deck", "event": "LIST" })).unwrap();
        let rows = listed["data"]["Card"].as_array().cloned().unwrap_or_default();
        rows.iter().filter_map(|r| r["id"].as_str().map(str::to_string)).collect()
    }

    #[test]
    fn a_runtime_entity_created_by_one_dispatch_is_listed_by_the_next() {
        assert_eq!(listed_card_ids("runtime"), ["c1"]);
    }

    #[test]
    fn control_a_persistent_entity_created_by_one_dispatch_is_listed_by_the_next() {
        assert_eq!(listed_card_ids("persistent"), ["c1"]);
    }

    fn session_program(shared: bool) -> orbital_core::OrbitalSchema {
        let machine = |events: serde_json::Value, arms: serde_json::Value| serde_json::json!({ "states": [{ "name": "idle", "isInitial": true }], "events": events, "transitions": arms });
        let arm = |event: &str, effects: serde_json::Value| serde_json::json!({ "from": "idle", "to": "idle", "event": event, "effects": effects });
        serde_json::from_value(serde_json::json!({
            "name": "Sessions",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Sessions",
                "entity": { "name": "Topic", "persistence": "runtime", "shared": shared, "fields": [
                    { "name": "id", "type": "string" }, { "name": "topic", "type": "string", "default": "" }, { "name": "seen", "type": "string", "default": "" }] },
                "traits": [
                    { "name": "Viewer", "linkedEntity": "Topic", "stateMachine": machine(
                        serde_json::json!([{ "key": "SET_TOPIC", "name": "Set", "payloadSchema": [{ "name": "topic", "type": "string", "required": true }] },
                            { "key": "READ_BACK", "name": "Read" }, { "key": "OPEN", "name": "Open" }, { "key": "CLOSE_FEED", "name": "Close feed" }, { "key": "ITEM", "name": "Item" }, { "key": "LIVE_FAILED", "name": "Failed" }]),
                        serde_json::json!([
                            arm("SET_TOPIC", serde_json::json!([["set", "@entity.topic", "@payload.topic"]])),
                            arm("READ_BACK", serde_json::json!([["set", "@entity.seen", "@entity.topic"]])),
                            arm("OPEN", serde_json::json!([["program/live", {}, { "key": "feed", "emit": { "onMessage": "ITEM", "failure": "LIVE_FAILED" } }]])),
                            arm("CLOSE_FEED", serde_json::json!([["cancel-call", "feed"]])),
                            arm("ITEM", serde_json::json!([])),
                            arm("LIVE_FAILED", serde_json::json!([]))])) },
                    { "name": "Sibling", "linkedEntity": "Topic", "stateMachine": machine(
                        serde_json::json!([{ "key": "PEEK", "name": "Peek" }]),
                        serde_json::json!([arm("PEEK", serde_json::json!([["set", "@entity.seen", "@entity.topic"]]))])) }
                ]
            }]
        }))
        .expect("the program parses")
    }

    fn session_host(shared: bool) -> Arc<Host> {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(session_program(shared)).unwrap();
        runtime
    }

    fn session_dispatch(runtime: &Host, trait_name: &str, event: &str, topic: Option<&str>, extra: serde_json::Value) -> Result<serde_json::Value, (String, serde_json::Value)> {
        let mut options = serde_json::json!({ "orbital": "Sessions", "targetTrait": trait_name, "event": event });
        if let (serde_json::Value::Object(options), serde_json::Value::Object(extra)) = (&mut options, extra) {
            options.extend(extra);
        }
        let payload = topic.map_or(serde_json::json!({}), |t| serde_json::json!({ "topic": t }));
        let args: Vec<Value> = [serde_json::json!(format!("Sessions.{trait_name}.{event}")), payload, options]
            .iter()
            .map(|a| serde_json::from_value(a.clone()).unwrap())
            .collect();
        orbital_core::effects::executor::SubstrateHandler::run(runtime.substrate.as_ref(), "program/dispatch", &args)
            .map(|v| orbital_core::json_value::value_to_json(&v))
            .map_err(|f| (f.message, serde_json::Value::Array(f.errors.iter().map(orbital_core::json_value::value_to_json).collect())))
    }

    fn as_tab(tab: &str) -> serde_json::Value {
        serde_json::json!({ "clientId": tab })
    }

    fn seen_by(runtime: &Host, trait_name: &str, event: &str, tab: &str) -> serde_json::Value {
        let answer = session_dispatch(runtime, trait_name, event, None, as_tab(tab)).expect("the dispatch runs");
        let _ = answer;
        let view = view_of(runtime, "Sessions", "Viewer", tab);
        view.get(trait_name).or_else(|| view.get("$shared::Topic")).map_or(serde_json::Value::Null, |frame| frame["seen"].clone())
    }

    fn open_client_stream(runtime: &Host, tab: &str) -> orbital_core::runtime::native_host::SubstrateSource {
        let Value::Object(options) = serde_json::from_value(serde_json::json!({ "clientId": tab })).unwrap() else { panic!("options") };
        runtime.substrate.live_source("program/live", &[Value::Object(options)]).expect("the client's live stream")
    }

    #[test]
    fn control_a_sessions_frame_is_kept_while_its_live_stream_is_open() {
        let runtime = session_host(false);
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), as_tab("tab-a")).unwrap();
        let _stream = open_client_stream(&runtime, "tab-a");
        assert_eq!(seen_by(&runtime, "Viewer", "READ_BACK", "tab-a"), "app");
    }

    #[test]
    fn control_a_client_whose_stream_never_opened_keeps_its_frame() {
        let runtime = session_host(false);
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), as_tab("tab-a")).unwrap();
        assert_eq!(seen_by(&runtime, "Viewer", "READ_BACK", "tab-a"), "app");
    }

    #[test]
    fn a_request_naming_no_client_is_its_own_scratch_session() {
        let runtime = session_host(false);
        let wire = serde_json::json!({ "wire": true });
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), wire.clone()).unwrap();
        let read = session_dispatch(&runtime, "Viewer", "READ_BACK", None, wire).unwrap();
        assert_ne!(frames_in(&read)["Viewer"]["seen"], "app", "a clientId-less request left a frame behind");
        assert!(runtime.kernel.frame_sessions().is_empty(), "a scratch session outlived its dispatch: {:?}", runtime.kernel.frame_sessions());
    }

    #[test]
    fn control_an_ask_naming_no_client_and_no_wire_runs_in_the_program_session() {
        let runtime = session_host(false);
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), serde_json::json!({})).unwrap();
        let read = session_dispatch(&runtime, "Viewer", "READ_BACK", None, serde_json::json!({})).unwrap();
        assert_eq!(frames_in(&read)["Viewer"]["seen"], "app");
    }

    #[test]
    fn control_a_request_naming_a_client_keeps_its_frame_across_requests() {
        let runtime = session_host(false);
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), as_tab("tab-a")).unwrap();
        assert_eq!(seen_by(&runtime, "Viewer", "READ_BACK", "tab-a"), "app");
    }

    #[test]
    fn a_shared_entitys_frame_is_one_per_session_across_its_bound_traits() {
        let runtime = session_host(true);
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), as_tab("tab-a")).unwrap();
        assert_eq!(seen_by(&runtime, "Sibling", "PEEK", "tab-a"), "app", "a sibling in the same session reads the shared frame");
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("cli"), as_tab("tab-b")).unwrap();
        assert_eq!(seen_by(&runtime, "Sibling", "PEEK", "tab-a"), "app", "another session's write reached this session's shared frame");
        assert_eq!(seen_by(&runtime, "Sibling", "PEEK", "tab-b"), "cli");
    }

    #[test]
    fn a_mount_seeds_the_frames_of_the_session_that_mounted() {
        let runtime = session_host(false);
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), as_tab("tab-a")).unwrap();
        let mounted = session_dispatch(&runtime, "Viewer", "INIT", None, serde_json::json!({ "clientId": "tab-b", "mount": [{ "trait": "Viewer", "event": "INIT" }] })).unwrap();
        assert_eq!(frames_in(&mounted)["Viewer"]["topic"], "", "tab-b's mount was seeded from tab-a's frame: {mounted}");
    }

    fn stepper_host() -> Arc<Host> {
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(serde_json::from_value(serde_json::json!({
            "name": "Steps",
            "typeContractVersion": 1,
            "orbitals": [{
                "name": "Steps",
                "entity": { "name": "Step", "persistence": "runtime", "fields": [{ "name": "id", "type": "string" }] },
                "traits": [{
                    "name": "Stepper", "linkedEntity": "Step",
                    "stateMachine": {
                        "states": [{ "name": "a", "isInitial": true }, { "name": "b" }],
                        "events": [{ "key": "ADVANCE", "name": "Advance" }, { "key": "INIT", "name": "Init" }, { "key": "PING", "name": "Ping" }],
                        "transitions": [
                            { "from": "a", "to": "b", "event": "ADVANCE", "effects": [] },
                            { "from": "a", "to": "a", "event": "INIT", "effects": [] }
                        ]
                    }
                }]
            }]
        })).expect("the program parses")).unwrap();
        runtime
    }

    fn step(runtime: &Host, event: &str, tab: &str, extra: serde_json::Value) -> serde_json::Value {
        let mut options = serde_json::json!({ "orbital": "Steps", "targetTrait": "Stepper", "event": event, "clientId": tab });
        if let (serde_json::Value::Object(options), serde_json::Value::Object(extra)) = (&mut options, extra) {
            options.extend(extra);
        }
        let args: Vec<Value> = [serde_json::json!(format!("Steps.Stepper.{event}")), serde_json::json!({}), options]
            .iter()
            .map(|a| serde_json::from_value(a.clone()).unwrap())
            .collect();
        let answer = orbital_core::effects::executor::SubstrateHandler::run(runtime.substrate.as_ref(), "program/dispatch", &args).expect("the dispatch runs");
        orbital_core::json_value::value_to_json(&answer)
    }

    #[test]
    fn a_tabs_trait_state_is_not_moved_by_another_tabs_transition() {
        let runtime = stepper_host();
        step(&runtime, "ADVANCE", "tab-b", serde_json::json!({}));
        let own = step(&runtime, "ADVANCE", "tab-a", serde_json::json!({}));
        assert_eq!(own["transitioned"], true, "tab-a was moved by tab-b's transition: {own}");
        assert_eq!(own["states"]["Stepper"], "b");
    }

    #[test]
    fn control_the_same_tab_sees_its_own_transition() {
        let runtime = stepper_host();
        step(&runtime, "ADVANCE", "tab-a", serde_json::json!({}));
        let again = step(&runtime, "ADVANCE", "tab-a", serde_json::json!({}));
        assert_eq!(again["transitioned"], false, "{again}");
        assert_eq!(again["states"]["Stepper"], "b");
    }

    #[test]
    fn a_held_state_echo_names_the_requesting_tabs_state() {
        let runtime = stepper_host();
        step(&runtime, "ADVANCE", "tab-b", serde_json::json!({}));
        let quiet = step(&runtime, "PING", "tab-a", serde_json::json!({}));
        assert_eq!(quiet["states"]["Stepper"], "a", "{quiet}");
        let own = step(&runtime, "PING", "tab-b", serde_json::json!({}));
        assert_eq!(own["states"]["Stepper"], "b", "control: tab-b holds its own state: {own}");
    }

    #[test]
    fn a_mount_runs_from_the_state_of_the_session_that_mounts() {
        let runtime = stepper_host();
        step(&runtime, "ADVANCE", "tab-a", serde_json::json!({}));
        let mounted = step(&runtime, "INIT", "tab-b", serde_json::json!({ "mount": [{ "trait": "Stepper", "event": "INIT" }] }));
        assert_eq!(mounted["transitioned"], true, "tab-b's mount saw tab-a's state: {mounted}");
    }

    #[test]
    fn a_scratch_request_leaves_no_trait_state_behind() {
        let runtime = stepper_host();
        let before = runtime.kernel.stats().active_instances;
        let wire = serde_json::json!({ "wire": true });
        let args: Vec<Value> = [serde_json::json!("Steps.Stepper.ADVANCE"), serde_json::json!({}), serde_json::json!({ "orbital": "Steps", "targetTrait": "Stepper", "event": "ADVANCE", "wire": wire["wire"] })]
            .iter()
            .map(|a| serde_json::from_value(a.clone()).unwrap())
            .collect();
        orbital_core::effects::executor::SubstrateHandler::run(runtime.substrate.as_ref(), "program/dispatch", &args).expect("the dispatch runs");
        assert_eq!(runtime.kernel.stats().active_instances, before, "a scratch session's trait instance outlived its dispatch");
    }

    fn revive_host() -> Arc<Host> {
        let mut program = serde_json::to_value(session_program(false)).unwrap();
        let viewer = &mut program["orbitals"][0]["traits"][0]["stateMachine"];
        viewer["events"].as_array_mut().unwrap().push(serde_json::json!({ "key": "INIT", "name": "Init" }));
        viewer["transitions"].as_array_mut().unwrap().push(serde_json::json!(
            { "from": "idle", "to": "idle", "event": "INIT", "effects": [["set", "@entity.topic", "from-init"]] }));
        let runtime = Host::new(Arc::new(DriverAdapter::memory())).shared();
        runtime.register_schema(serde_json::from_value(program).unwrap()).unwrap();
        runtime
    }

    fn reconnect_mount(runtime: &Host, tab: &str, carried: Option<serde_json::Value>) -> Result<serde_json::Value, (String, serde_json::Value)> {
        let mut extra = serde_json::json!({ "clientId": tab, "mount": [{ "trait": "Viewer", "event": "INIT" }] });
        if let (Some(carried), serde_json::Value::Object(extra)) = (carried, &mut extra) {
            extra.insert("traits".to_string(), serde_json::json!([{ "trait": "Viewer", "from": "idle" }]));
            extra.insert("entityByTrait".to_string(), serde_json::json!({ "Viewer": carried }));
        }
        session_dispatch(runtime, "Viewer", "INIT", None, extra)
    }

    #[test]
    fn a_sessions_frame_is_freed_once_its_live_stream_drops_and_it_is_quiet() {
        let runtime = session_host(false);
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), as_tab("tab-a")).unwrap();
        let stream = open_client_stream(&runtime, "tab-a");
        assert!(runtime.kernel.frame_sessions().contains(&"tab-a".to_string()), "control: the frame exists while the stream is open");
        drop(stream);
        assert!(!runtime.kernel.frame_sessions().contains(&"tab-a".to_string()), "the ended quiet session's frame was kept");
    }

    #[test]
    fn a_sessions_trait_state_is_freed_with_its_frames() {
        let runtime = stepper_host();
        step(&runtime, "ADVANCE", "tab-a", serde_json::json!({}));
        let held = runtime.kernel.stats().active_instances;
        let Value::Object(options) = serde_json::from_value(serde_json::json!({ "clientId": "tab-a" })).unwrap() else { panic!("options") };
        let stream = runtime.substrate.live_source("program/live", &[Value::Object(options)]).expect("the client's live stream");
        drop(stream);
        assert!(runtime.kernel.stats().active_instances < held, "the ended session's trait instance was kept");
    }

    #[test]
    fn a_session_is_kept_while_a_keyed_subscription_it_opened_is_pending() {
        let runtime = session_host(false);
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), as_tab("tab-a")).unwrap();
        session_dispatch(&runtime, "Viewer", "OPEN", None, as_tab("tab-a")).unwrap();
        drop(open_client_stream(&runtime, "tab-a"));
        assert!(runtime.kernel.frame_sessions().contains(&"tab-a".to_string()), "a session with a pending subscription was freed");
    }

    #[test]
    fn a_session_is_freed_once_the_subscription_it_held_has_been_cancelled() {
        let runtime = session_host(false);
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), as_tab("tab-a")).unwrap();
        session_dispatch(&runtime, "Viewer", "OPEN", None, as_tab("tab-a")).unwrap();
        session_dispatch(&runtime, "Viewer", "CLOSE_FEED", None, as_tab("tab-a")).unwrap();
        drop(open_client_stream(&runtime, "tab-a"));
        assert!(!runtime.kernel.frame_sessions().contains(&"tab-a".to_string()), "a quiet session stayed");
    }

    #[test]
    fn a_request_to_a_freed_session_is_refused_with_a_named_error_and_never_rebuilt_from_defaults() {
        let runtime = session_host(false);
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), as_tab("tab-a")).unwrap();
        drop(open_client_stream(&runtime, "tab-a"));
        let (_, errors) = session_dispatch(&runtime, "Viewer", "READ_BACK", None, as_tab("tab-a")).unwrap_err();
        assert_eq!(errors[0]["code"], "SESSION_ENDED", "{errors}");
        assert_eq!(errors[0]["status"], 409, "{errors}");
    }

    #[test]
    fn control_a_never_seen_client_starts_a_fresh_session_not_a_refusal() {
        let runtime = session_host(false);
        assert!(session_dispatch(&runtime, "Viewer", "READ_BACK", None, as_tab("tab-new")).is_ok());
    }

    #[test]
    fn a_reconnect_mount_carrying_the_frame_rebuilds_it_exactly() {
        let runtime = revive_host();
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), as_tab("tab-a")).unwrap();
        drop(open_client_stream(&runtime, "tab-a"));
        let mounted = reconnect_mount(&runtime, "tab-a", Some(serde_json::json!({ "topic": "app" }))).unwrap();
        assert_eq!(frames_in(&mounted)["Viewer"]["topic"], "app", "the carried frame was not rebuilt: {mounted}");
        assert_eq!(seen_by(&runtime, "Viewer", "READ_BACK", "tab-a"), "app", "the rebuilt session was reset by INIT");
    }

    #[test]
    fn a_reconnect_mount_without_a_carried_frame_starts_from_init_as_a_fresh_tab_does() {
        let runtime = revive_host();
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), as_tab("tab-a")).unwrap();
        drop(open_client_stream(&runtime, "tab-a"));
        reconnect_mount(&runtime, "tab-a", None).unwrap();
        assert_eq!(seen_by(&runtime, "Viewer", "READ_BACK", "tab-a"), "from-init");
    }

    /// G-CROSS-357 twin of the embedded server's: a session whose only stream dropped before any request held no state, so
    /// a carried frame is the client's own step and INIT runs here.
    #[test]
    fn a_reconnect_mount_into_a_session_that_held_no_state_runs_init_over_the_carried_frame() {
        let runtime = revive_host();
        drop(open_client_stream(&runtime, "tab-a"));
        reconnect_mount(&runtime, "tab-a", Some(serde_json::json!({ "topic": "client-made" }))).unwrap();
        assert_eq!(seen_by(&runtime, "Viewer", "READ_BACK", "tab-a"), "from-init");
    }

    #[test]
    fn control_a_mount_into_a_live_session_ignores_a_carried_frame_and_runs_init() {
        let runtime = revive_host();
        session_dispatch(&runtime, "Viewer", "SET_TOPIC", Some("app"), as_tab("tab-a")).unwrap();
        let _stream = open_client_stream(&runtime, "tab-a");
        reconnect_mount(&runtime, "tab-a", Some(serde_json::json!({ "topic": "stale" }))).unwrap();
        assert_eq!(seen_by(&runtime, "Viewer", "READ_BACK", "tab-a"), "from-init");
    }

    #[test]
    fn a_row_literally_named_singleton_keeps_its_own_state_beside_the_unaddressed_instance() {
        let runtime = stepper_host();
        step(&runtime, "ADVANCE", "tab-a", serde_json::json!({}));
        let row = step(&runtime, "ADVANCE", "tab-a", serde_json::json!({ "entityId": "singleton" }));
        assert_eq!(row["transitioned"], true, "the row named singleton shared the unaddressed instance's state: {row}");
    }

    #[test]
    fn control_the_unaddressed_instance_keeps_its_state_across_requests() {
        let runtime = stepper_host();
        step(&runtime, "ADVANCE", "tab-a", serde_json::json!({}));
        let again = step(&runtime, "ADVANCE", "tab-a", serde_json::json!({}));
        assert_eq!(again["transitioned"], false, "{again}");
    }

}
