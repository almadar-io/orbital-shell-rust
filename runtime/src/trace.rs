use super::{TraceAsk, TraceEntry, TRACE_CAPACITY};
use orbital_core::runtime::dispatch_trace::DispatchTraceRecord;
use orbital_core::runtime::native_host::SubstrateSource;
use orbital_core::runtime::os_host::completion::StreamPoll;
use orbital_core::runtime::resumable::{IoAnswer, IoRequest};
use orbital_core::{Value, ValueMap};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};

static INSTANCE_SEQ: AtomicU64 = AtomicU64::new(0);

struct Subscription {
    id: u64,
    traits: Vec<String>,
    entity: Option<String>,
    sender: mpsc::Sender<Value>,
}

pub(super) struct TraceJournal {
    pub log: Mutex<VecDeque<TraceEntry>>,
    pub on: AtomicBool,
    pub file: Mutex<Option<(std::path::PathBuf, std::fs::File)>>,
    pub version: AtomicU64,
    pending_asks: Mutex<std::collections::HashMap<u64, Vec<TraceAsk>>>,
    alive: std::sync::Weak<()>,
    instance: String,
    seq: AtomicU64,
    next_subscriber: AtomicU64,
    subscribers: Arc<Mutex<Vec<Subscription>>>,
}

struct TraceConnection {
    id: u64,
    subscribers: Arc<Mutex<Vec<Subscription>>>,
}

impl Drop for TraceConnection {
    fn drop(&mut self) {
        self.subscribers.lock().retain(|s| s.id != self.id);
    }
}

impl TraceJournal {
    pub fn new(alive: std::sync::Weak<()>) -> Self {
        Self {
            alive,
            log: Mutex::new(VecDeque::new()),
            on: AtomicBool::new(false),
            file: Mutex::new(None),
            version: AtomicU64::new(0),
            pending_asks: Mutex::default(),
            instance: format!(
                "{}-{}-{}",
                std::process::id(),
                orbital_core::time_util::now_millis(),
                INSTANCE_SEQ.fetch_add(1, Ordering::SeqCst)
            ),
            seq: AtomicU64::new(0),
            next_subscriber: AtomicU64::new(0),
            subscribers: Arc::default(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.on.load(Ordering::SeqCst) || !self.subscribers.lock().is_empty()
    }

    pub fn needs_completion(&self) -> bool {
        self.enabled() || !self.pending_asks.lock().is_empty()
    }

    pub fn record_ask(&self, request: &IoRequest, answer: &IoAnswer) {
        let Some(dispatch_id) = orbital_core::runtime::dispatch_trace::answer_dispatch_id() else {
            return;
        };
        if self.enabled() {
            self.pending_asks
                .lock()
                .entry(dispatch_id)
                .or_default()
                .push(TraceAsk {
                    request: format!("{request:?}"),
                    answer: format!("{answer:?}"),
                });
        }
    }

    pub fn take_asks(&self, dispatch_id: u64) -> Vec<TraceAsk> {
        self.pending_asks
            .lock()
            .remove(&dispatch_id)
            .unwrap_or_default()
    }

    pub fn source(self: &Arc<Self>, options: &ValueMap) -> SubstrateSource {
        let traits: Vec<String> = match options.get("traits") {
            Some(Value::Array(items))
                if items
                    .iter()
                    .all(|item| item.as_str().is_some_and(|s| !s.is_empty())) =>
            {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            }
            _ => Vec::new(),
        };
        let instance = options.get("instance").and_then(Value::as_str);
        let reason = if options
            .get("instance")
            .is_some_and(|v| v.as_str().is_none())
            || options
                .get("seedEntityId")
                .is_some_and(|v| v.as_str().is_none())
        {
            Some("trace instance and seedEntityId must be strings when supplied")
        } else if traits.is_empty() {
            Some("trace subscription requires explicit traits")
        } else if instance.is_some_and(|i| i != self.instance) {
            Some("selected instance is no longer running")
        } else {
            None
        };
        let (sender, receiver) = mpsc::channel();
        if let Some(reason) = reason {
            let _ = sender.send(wire(serde_json::json!({ "type": "trace-disconnected", "instance": self.instance, "reason": reason })));
            return Box::new(move || match receiver.try_recv() {
                Ok(item) => StreamPoll::Item(item),
                Err(_) => StreamPoll::Ended,
            });
        }
        let id = self.next_subscriber.fetch_add(1, Ordering::SeqCst);
        let mut subscribers = self.subscribers.lock();
        let _ = sender.send(wire(serde_json::json!({ "type": "trace-attached", "instance": self.instance, "programVersion": self.version.load(Ordering::SeqCst), "nextSeq": self.seq.load(Ordering::SeqCst) })));
        subscribers.push(Subscription {
            id,
            traits,
            entity: options
                .get("seedEntityId")
                .and_then(Value::as_str)
                .map(str::to_string),
            sender,
        });
        drop(subscribers);
        let connection = TraceConnection {
            id,
            subscribers: self.subscribers.clone(),
        };
        let host = self.alive.clone();
        let instance = self.instance.clone();
        let mut disconnected = false;
        Box::new(move || {
            let _connection = &connection;
            match receiver.try_recv() {
                Ok(item) => StreamPoll::Item(item),
                Err(mpsc::TryRecvError::Empty) if host.upgrade().is_some() => StreamPoll::Idle,
                Err(_) if !disconnected => {
                    disconnected = true;
                    StreamPoll::Item(wire(
                        serde_json::json!({ "type": "trace-disconnected", "instance": instance, "reason": "host stopped" }),
                    ))
                }
                Err(_) => StreamPoll::Ended,
            }
        })
    }

    pub fn record(&self, observed: DispatchTraceRecord) {
        let asks = self.take_asks(observed.dispatch_id);
        let mut subscribers = self.subscribers.lock();
        if !self.on.load(Ordering::SeqCst) && subscribers.is_empty() {
            return;
        }
        let seed = observed
            .seeds
            .iter()
            .map(|s| format!("{}.{}", s.trait_name, s.event))
            .collect::<Vec<_>>()
            .join("+");
        let seed_entity_id = observed
            .seeds
            .first()
            .and_then(|s| s.entity_id.clone())
            .unwrap_or_else(|| "singleton".into());
        let seed_traits = observed
            .seeds
            .iter()
            .map(|s| s.trait_name.clone())
            .collect();
        let entry = TraceEntry {
            seq: self.seq.fetch_add(1, Ordering::SeqCst),
            dispatch_id: observed.dispatch_id,
            at_ms: orbital_core::time_util::now_millis(),
            instance: self.instance.clone(),
            program_version: self.version.load(Ordering::SeqCst),
            seed,
            seed_entity_id,
            seed_traits,
            outcomes: observed.outcomes,
            asks,
            error: observed.error,
        };
        let envelope = wire(
            serde_json::json!({ "type": "trace", "instance": self.instance, "programVersion": entry.program_version, "dispatch": entry }),
        );
        subscribers.retain(|subscriber| {
            let selected = subscriber
                .entity
                .as_ref()
                .is_none_or(|id| id == &entry.seed_entity_id)
                && (entry
                    .outcomes
                    .iter()
                    .any(|outcome| subscriber.traits.contains(&outcome.trait_name))
                    || (entry.error.is_some()
                        && entry
                            .seed_traits
                            .iter()
                            .any(|t| subscriber.traits.contains(t))));
            !selected || subscriber.sender.send(envelope.clone()).is_ok()
        });
        if let Some((path, file)) = self.file.lock().as_mut() {
            use std::io::Write;
            let written = serde_json::to_string(&entry)
                .map_err(std::io::Error::other)
                .and_then(|line| writeln!(file, "{line}"));
            if let Err(error) = written {
                tracing::error!(path = %path.display(), %error, "trace file write failed");
            }
        }
        let mut log = self.log.lock();
        if log.len() == TRACE_CAPACITY {
            log.pop_front();
        }
        log.push_back(entry);
    }
}

fn wire(json: serde_json::Value) -> Value {
    serde_json::from_value(json).expect("trace wire contains JSON values")
}
