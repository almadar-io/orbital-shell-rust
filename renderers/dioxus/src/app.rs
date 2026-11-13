//! Root Dioxus component + shared-substrate link probe.
//!
//! The root [`App`] component reads the Orbital circuit (`ClientCore`) and the
//! pattern registry from a thread-local [`APP`] cell (wasm is single-threaded)
//! and paints the current `ui_slots` through the Dioxus renderers. User events
//! flow back through an [`EventSink`] (`Callback`) that drives `ClientCore`
//! synchronously and bumps a version signal so Dioxus re-renders — the same
//! closed circuit as the egui web shell, different paint layer.

use crate::client_core::ClientCore;
use crate::recurse;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusView, EventSink, NavigationSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::{resolve_row_set_markers, SlotContent};
use orbital_core::runtime::theme::resolve_preset_key;
use orbital_core::{slot_role, SlotRole, ThemeMode, ThemePresets, Value, ValueMap};
use std::cell::RefCell;
#[cfg(all(feature = "native-http", not(target_arch = "wasm32")))]
fn spawn_server_leg<T: Send + 'static>(
    job: impl FnOnce() -> T + Send + 'static,
    finish: impl FnOnce(T) + 'static,
) {
    let (tx, rx) = futures_channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(job());
    });
    spawn(async move {
        match rx.await {
            Ok(result) => finish(result),
            Err(_) => tracing::error!(target: "almadar::client", "server worker stopped"),
        }
    });
}

#[cfg(all(
    any(feature = "native-http", feature = "native-audio"),
    not(target_arch = "wasm32")
))]
fn bump(mut version: Signal<u32>) {
    let next = version.peek().wrapping_add(1);
    version.set(next);
}

#[cfg(all(feature = "native-http", not(target_arch = "wasm32")))]
fn remote_mount(version: Signal<u32>) {
    let begun = APP.with(|cell| {
        let mut borrow = cell.borrow_mut();
        let state = borrow.as_mut()?;
        let kernel = state.core.client_kernel.clone()?;
        kernel.open_feed();
        match state.core.begin_mount_page() {
            Ok(begun) => Some((kernel, begun)),
            Err(error) => {
                tracing::error!(target: "almadar::client", %error, "remote mount refused");
                state.core.fail_mount(error.to_string());
                state.core.status_message = Some((format!("Error: {error}"), true));
                None
            }
        }
    });
    let Some((kernel, begun)) = begun else { return };
    let awaiting = if begun.has_legs() {
        APP.with(|cell| {
            cell.borrow_mut().as_mut().and_then(|state| {
                state
                    .core
                    .begin_awaiting(&begun.awaiting(), web_time::Instant::now())
            })
        })
    } else {
        None
    };
    bump(version);
    tracing::info!(target: "almadar::client", server_legs = begun.has_legs(), "remote mount begun");
    remote_followups(version);
    if !begun.has_legs() {
        APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.core.finish_mount_page(begun.settle(&kernel));
            }
        });
        bump(version);
        remote_followups(version);
        return;
    }
    spawn_server_leg(
        move || begun.settle(&kernel),
        move |result| {
            if let Err(error) = &result {
                tracing::error!(target: "almadar::client", %error, "remote mount failed");
            }
            if let Ok(response) = &result {
                tracing::info!(target: "almadar::client", success = response.success, emitted = response.emitted_events.len(), paints = response.client_effects_by_trait.as_ref().map_or(0, Vec::len), "remote mount settled");
            }
            APP.with(|cell| {
                if let Some(state) = cell.borrow_mut().as_mut() {
                    if let Some(token) = awaiting {
                        state.core.end_awaiting(token);
                    }
                    state.core.finish_mount_page(result);
                }
            });
            bump(version);
            remote_followups(version);
        },
    );
}

#[cfg(all(feature = "native-http", not(target_arch = "wasm32")))]
fn remote_control(event: String, source: Option<String>, payload: Value, version: Signal<u32>) {
    let begun = APP.with(|cell| {
        let mut borrow = cell.borrow_mut();
        let state = borrow.as_mut()?;
        let kernel = state.core.client_kernel.clone()?;
        match state.core.begin_control(&event, source.as_deref(), payload) {
            Ok(begun) => Some((kernel, begun)),
            Err(error) => {
                tracing::error!(target: "almadar::client", %error, "remote control refused");
                state.core.status_message = Some((format!("Error: {error}"), true));
                None
            }
        }
    });
    let Some((kernel, begun)) = begun else { return };
    let awaiting = if begun.has_legs() {
        APP.with(|cell| {
            cell.borrow_mut().as_mut().and_then(|state| {
                state
                    .core
                    .begin_awaiting(begun.awaiting(), web_time::Instant::now())
            })
        })
    } else {
        None
    };
    bump(version);
    remote_followups(version);
    if !begun.has_legs() {
        APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.core.finish_control(&begun, kernel.settle(&begun));
            }
        });
        remote_followups(version);
        return;
    }
    spawn_server_leg(
        move || {
            let result = kernel.settle(&begun);
            (begun, result)
        },
        move |(begun, result)| {
            if let Err(error) = &result {
                tracing::error!(target: "almadar::client", %error, "remote control failed");
            }
            APP.with(|cell| {
                if let Some(state) = cell.borrow_mut().as_mut() {
                    if let Some(token) = awaiting {
                        state.core.end_awaiting(token);
                    }
                    state.core.finish_control(&begun, result);
                }
            });
            bump(version);
            remote_followups(version);
        },
    );
}

#[cfg(all(feature = "native-http", not(target_arch = "wasm32")))]
fn remote_followups(version: Signal<u32>) {
    if REMOTE_PUMPING.with(|pumping| pumping.replace(true)) {
        return;
    }
    for _ in 0..8 {
        let (navigate, events) = APP.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let Some(state) = borrow.as_mut() else {
                return (false, Vec::new());
            };
            (
                state.core.needs_page_mount(),
                std::mem::take(&mut state.core.pending_events),
            )
        });
        if !navigate && events.is_empty() {
            break;
        }
        tracing::info!(target: "almadar::client", navigate, events = events.len(), "remote followups");
        if navigate {
            remote_mount(version);
        }
        for (event, source, payload) in events {
            remote_control(event, source, payload, version);
        }
    }
    REMOTE_PUMPING.with(|pumping| pumping.set(false));
}

#[cfg(all(feature = "native-http", not(target_arch = "wasm32")))]
fn remote_frame(version: Signal<u32>) {
    let heard = APP.with(|cell| {
        let mut borrow = cell.borrow_mut();
        borrow.as_mut().is_some_and(|state| state.core.drain_host_feed())
    });
    if heard {
        bump(version);
        remote_followups(version);
    }
    if APP.with(|cell| {
        cell.borrow()
            .as_ref()
            .is_some_and(|state| state.core.has_awaiting())
    }) {
        bump(version);
    }
    let begun = APP.with(|cell| {
        let mut borrow = cell.borrow_mut();
        let state = borrow.as_mut()?;
        let kernel = state.core.client_kernel.clone()?;
        match state.core.begin_frame() {
            Ok(Some(begun)) => Some((kernel, begun)),
            Ok(None) => None,
            Err(error) => {
                state.core.finish_frame(Err(error));
                None
            }
        }
    });
    let Some((kernel, begun)) = begun else { return };
    let awaiting = if begun.has_legs() {
        APP.with(|cell| {
            cell.borrow_mut().as_mut().and_then(|state| {
                state
                    .core
                    .begin_awaiting(&begun.awaiting(), web_time::Instant::now())
            })
        })
    } else {
        None
    };
    bump(version);
    remote_followups(version);
    if !begun.has_legs() {
        APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.core.finish_frame(kernel.settle_frame(&begun));
            }
        });
        remote_followups(version);
        return;
    }
    spawn_server_leg(
        move || kernel.settle_frame(&begun),
        move |result| {
            APP.with(|cell| {
                if let Some(state) = cell.borrow_mut().as_mut() {
                    if let Some(token) = awaiting {
                        state.core.end_awaiting(token);
                    }
                    state.core.finish_frame(result);
                }
            });
            bump(version);
            remote_followups(version);
        },
    );
}

/// Everything a running Dioxus Orbital app needs: the shared circuit and the
/// pattern registry that paints its slots.
pub struct AppState {
    /// The ONE event circuit (shared with egui via `orbital-client`).
    pub core: ClientCore,
    /// Dioxus pattern registry (built once at mount from the generated sets).
    pub registry: DioxusPatternRegistry,
    /// Whether this mount delegates server legs over HTTP.
    pub remote: bool,
}

std::thread_local! {
    /// Mounted app state. `None` before `web::start` mounts a schema.
    pub static APP: RefCell<Option<AppState>> = RefCell::new(None);
    /// Re-render poke: the root component's version signal, registered so the
    /// external `web::send_event` (driven from JS, outside a Dioxus render) can
    /// invalidate the component exactly like an in-app click does.
    static VERSION: RefCell<Option<Signal<u32>>> = RefCell::new(None);
    #[cfg(all(feature = "native-http", not(target_arch = "wasm32")))]
    static FRAME_CALLBACK: RefCell<Option<Callback<()>>> = RefCell::new(None);
    /// Stylesheet texts injected into the document head at mount. Set by the
    /// native (Blitz) shell from `--css` files — Blitz resolves `class:`
    /// attributes against them (almadar.css base + Tailwind utilities). Empty
    /// on web, where CSS arrives via `<link>` in the preview HTML instead.
    pub static CSS_TEXT: RefCell<Vec<String>> = RefCell::new(Vec::new());
    /// Active design-system theme (`@almadar/ui` `[data-theme="…"]` blocks in
    /// the generated preset stylesheet). `None` uses the web shell's wireframe-light default.
    /// Read by the root `App`
    /// component into the `data-theme` attribute.
    pub static THEME: RefCell<Option<String>> = RefCell::new(None);
    #[cfg(all(feature = "native-http", not(target_arch = "wasm32")))]
    static REMOTE_PUMPING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Bump the root component's version signal (no-op if the app is not mounted).
/// Called by `web::send_event` after driving the circuit so Dioxus repaints.
pub fn poke_rerender() {
    VERSION.with(|v| {
        if let Some(signal) = v.borrow_mut().as_mut() {
            let next = signal.peek().wrapping_add(1);
            signal.set(next);
        }
    });
}

#[cfg(all(feature = "native-http", not(target_arch = "wasm32")))]
pub fn drive_remote_frame() -> bool {
    let remote = APP.with(|cell| cell.borrow().as_ref().is_some_and(|state| state.remote));
    if !remote {
        return false;
    }
    FRAME_CALLBACK.with(|cell| {
        if let Some(callback) = cell.borrow().as_ref() {
            callback.call(());
        }
    });
    true
}

/// `slot`'s contents as painted: every row-set marker in a prop is replaced by the rows the kernel's one resolver
/// ([`ClientCore::resolve_rows`]) answers now, re-checked for the current viewer. A repaint of a slot holding a
/// reference (`repaint: "rows-changed"`) replaces its content, and this re-resolves it.
pub fn painted_slot_contents(core: &ClientCore, slot: &str) -> Vec<SlotContent> {
    painted_slot_contents_at(core, slot, web_time::Instant::now())
}

fn painted_slot_contents_at(
    core: &ClientCore,
    slot: &str,
    now: web_time::Instant,
) -> Vec<SlotContent> {
    let mut contents = core.slot_contents(slot);
    if slot == "main" {
        contents.extend(core.awaiting_main_contents(now));
    } else if contents.is_empty() {
        contents.extend(core.awaiting_slot_content(slot, now));
    }
    contents
        .into_iter()
        .map(|content| SlotContent {
            props: resolve_row_set_markers(&content.props, &|reference| {
                core.resolve_rows(reference)
            }),
            ..content
        })
        .collect()
}

/// The per-trait composed content (`@trait.X`) as painted, resolved like [`painted_slot_contents`].
pub fn painted_trait_content(core: &ClientCore) -> ValueMap {
    painted_trait_content_at(core, web_time::Instant::now())
}

fn painted_trait_content_at(core: &ClientCore, now: web_time::Instant) -> ValueMap {
    core.trait_content_with_awaiting(now)
        .iter()
        .map(|(name, content)| {
            (
                name.clone(),
                resolve_row_set_markers(content, &|reference| core.resolve_rows(reference)),
            )
        })
        .collect()
}

/// Root Orbital component. Reads [`APP`], paints `ui_slots` through the registry,
/// and wires an [`EventSink`] so button clicks drive the shared circuit.
#[cfg(all(feature = "native-audio", not(target_arch = "wasm32")))]
fn answer_host_ask(
    ask: orbital_core::client::client_core::PendingHostAsk,
    answer: Result<(), crate::audio::AudioFailure>,
    version: Signal<u32>,
) {
    if let Err(error) = &answer {
        tracing::error!(op = %ask.op, source_trait = %ask.source_trait, kind = error.kind, message = %error.error, "native host ask failed");
    }
    let Some(completion) = ask.completion else {
        return;
    };
    let event = match &answer {
        Ok(()) => completion.success.clone(),
        Err(_) => completion.failure.clone(),
    };
    let Some(event) = event else { return };
    let payload = if let Err(error) = &answer {
        orbital_core::client::client_core::host_ask_failure_payload(
            &completion,
            &error.error,
            error.kind,
        )
    } else {
        let mut payload = ValueMap::new();
        if let Some(key) = &completion.key {
            payload.insert("key".into(), Value::string(key.as_str()));
        }
        Value::object(payload)
    };
    let begun = APP.with(|cell| {
        let mut borrow = cell.borrow_mut();
        let state = borrow.as_mut()?;
        let kernel = state.core.client_kernel.clone()?;
        match state
            .core
            .begin_host_completion(&completion, &event, payload)
        {
            Ok(begun) => Some((kernel, begun, state.remote)),
            Err(error) => {
                tracing::error!(%error, op = %ask.op, "host-ask return refused");
                None
            }
        }
    });
    let Some((kernel, begun, remote)) = begun else {
        return;
    };
    bump(version);
    #[cfg(feature = "native-http")]
    if remote && begun.has_legs() {
        spawn_server_leg(
            move || {
                let result = kernel.settle(&begun);
                (begun, result)
            },
            move |(begun, result)| {
                APP.with(|cell| {
                    if let Some(state) = cell.borrow_mut().as_mut() {
                        state.core.finish_control(&begun, result);
                    }
                });
                bump(version);
                remote_followups(version);
            },
        );
        return;
    }
    #[cfg(not(feature = "native-http"))]
    let _ = remote;
    let result = kernel.settle(&begun);
    APP.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            state.core.finish_control(&begun, result);
        }
    });
    bump(version);
    #[cfg(feature = "native-http")]
    remote_followups(version);
}

fn effective_theme(core: &ClientCore, host_theme: Option<&str>) -> (String, ThemeMode) {
    if let Some(key) = host_theme {
        return (
            key.to_string(),
            resolve_preset_key(ThemePresets::installed(), key).mode,
        );
    }
    if let Some(theme) = core.resolved_theme() {
        if core.schema.theme.is_some() {
            return (theme.key, theme.mode);
        }
        return (format!("wireframe-{}", theme.mode.as_str()), theme.mode);
    }
    ("wireframe-light".to_string(), ThemeMode::Light)
}

#[component]
pub fn App() -> Element {
    let mut version = use_signal(|| 0u32);
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    let viewport_width =
        try_use_context::<dioxus_native::NativeViewport>().map(|viewport| viewport.width() as f64);
    #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
    let viewport_width = None;
    #[cfg(all(feature = "native-http", not(target_arch = "wasm32")))]
    use_effect(move || {
        let remote = APP.with(|cell| cell.borrow().as_ref().is_some_and(|state| state.remote));
        if remote {
            remote_mount(version);
        }
    });
    // Register the version signal for external pokes and subscribe to it.
    VERSION.with(|v| *v.borrow_mut() = Some(version));
    #[cfg(all(feature = "native-http", not(target_arch = "wasm32")))]
    FRAME_CALLBACK
        .with(|cell| *cell.borrow_mut() = Some(use_callback(move |()| remote_frame(version))));
    let _tick = *version.read();
    #[cfg(all(feature = "native-audio", not(target_arch = "wasm32")))]
    use_effect(move || {
        let _tick = *version.read();
        let asks = APP.with(|cell| {
            cell.borrow_mut()
                .as_mut()
                .map(|state| std::mem::take(&mut state.core.pending_host_asks))
                .unwrap_or_default()
        });
        for ask in asks {
            if ask.op.starts_with("audio/") {
                let answer = crate::audio::host_ask(
                    ask.source_trait.clone(),
                    ask.op.clone(),
                    ask.args.clone(),
                );
                spawn(async move {
                    let result = answer.await.unwrap_or_else(|_| {
                        Err(crate::audio::AudioFailure {
                            kind: "unavailable",
                            error: "native audio worker stopped before answering".into(),
                        })
                    });
                    answer_host_ask(ask, result, version);
                });
            } else {
                let error = crate::audio::AudioFailure {
                    kind: "unavailable",
                    error: format!("native host has no action for host-ask `{}`", ask.op),
                };
                answer_host_ask(ask, Err(error), version);
            }
        }
    });

    // A control dispatches as the trait whose content rendered it (the egui shell's
    // `RenderContext::queue_event`), the active trait when the content carries no source.
    let routed = use_callback(
        move |(event, source, payload): (String, Option<String>, Value)| {
            #[cfg(all(feature = "native-http", not(target_arch = "wasm32")))]
            if APP.with(|cell| cell.borrow().as_ref().is_some_and(|state| state.remote)) {
                remote_control(event, source, payload, version);
                return;
            }
            APP.with(|cell| {
                if let Some(state) = cell.borrow_mut().as_mut() {
                    state
                        .core
                        .dispatch_control(&event, source.as_deref(), payload);
                    state.core.settle(8);
                }
            });
            let next = version.peek().wrapping_add(1);
            version.set(next);
        },
    );
    let navigate: NavigationSink = use_callback(move |path: String| {
        let remote = APP.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let Some(state) = borrow.as_mut() else {
                return false;
            };
            state.core.navigate_to(&path, None);
            state.remote
        });
        #[cfg(all(feature = "native-http", not(target_arch = "wasm32")))]
        if remote {
            remote_mount(version);
            return;
        }
        #[cfg(not(all(feature = "native-http", not(target_arch = "wasm32"))))]
        let _ = remote;
        APP.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                if state.core.needs_page_mount() {
                    state.core.mount_page_blocking();
                    state.core.settle(8);
                }
            }
        });
        let next = version.peek().wrapping_add(1);
        version.set(next);
    });
    let toggle_theme: Callback<()> = use_callback(move |()| {
        let changed = APP.with(|cell| {
            let borrow = cell.borrow();
            let Some(state) = borrow.as_ref() else {
                return false;
            };
            if state.core.client_kernel.is_none() {
                return false;
            }
            let host_theme = THEME.with(|theme| theme.borrow().clone());
            let (_, current_mode) = effective_theme(&state.core, host_theme.as_deref());
            let next_mode = match current_mode {
                ThemeMode::Light => ThemeMode::Dark,
                ThemeMode::Dark => ThemeMode::Light,
            };
            state.core.set_theme_mode(next_mode);
            if let Some(key) = host_theme {
                let base = resolve_preset_key(ThemePresets::installed(), &key).base;
                THEME.with(|theme| {
                    *theme.borrow_mut() = Some(format!("{base}-{}", next_mode.as_str()));
                });
            }
            true
        });
        if changed {
            let next = version.peek().wrapping_add(1);
            version.set(next);
        }
    });
    // One sink per source trait, made once: a callback made per render lives as long as this root scope.
    let bound = use_hook(|| {
        std::rc::Rc::new(RefCell::new(std::collections::HashMap::<
            Option<String>,
            EventSink,
        >::new()))
    });
    let source_sink = use_callback(move |source: Option<String>| -> EventSink {
        *bound.borrow_mut().entry(source.clone()).or_insert_with(|| {
            Callback::new(move |(event, payload): (String, Value)| {
                routed.call((event, source.clone(), payload))
            })
        })
    });
    let sink = source_sink.call(None);

    APP.with(|cell| {
        let borrow = cell.borrow();
        let Some(state) = borrow.as_ref() else {
            return rsx! {
                div { class: "almadar-orbital-dioxus",
                    p { "Orbital · Dioxus — no schema mounted" }
                }
            };
        };
        let core = &state.core;
        let registry = &state.registry;
        let orbital = core.orbital_name();
        let now = web_time::Instant::now();
        let mut slot_names: Vec<String> = core.ui_slots.keys().cloned().collect();
        slot_names.extend(core.awaiting_slot_contents(now).into_iter().map(|(slot, _)| slot));
        slot_names.push("sidebar".into());
        slot_names.push("main".into());
        slot_names.sort_by_cached_key(|name| slot_paint_rank(name));
        slot_names.dedup();
        let (layout_slots, overlay_slots): (Vec<String>, Vec<String>) = slot_names
            .into_iter()
            .partition(|name| in_layout_row(name));

        let trait_content = painted_trait_content_at(core, now);
        let host_theme = THEME.with(|theme| theme.borrow().clone());
        let (theme, theme_mode) = effective_theme(core, host_theme.as_deref());
        #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
        let reduced_motion = crate::native::reduced_motion();
        #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
        let reduced_motion = false;
        let locale = core.locale();
        let messages = locale
            .as_ref()
            .and_then(|locale| core.schema.messages.as_ref()?.get(locale).cloned())
            .unwrap_or_default();
        crate::i18n::set_locale(locale, messages);
        let view = DioxusView {
            trait_name: &core.current_trait_name,
            orbital_name: &orbital,
            current_state: &core.current_state,
            entities: &core.entities,
            trait_content: &trait_content,
            source_trait: None,
            sink: Some(sink),
            source_sink,
            navigate: Some(navigate),
            dock: None,
            viewport_width,
            canvas_theme: core.canvas_theme(),
            theme_mode: Some(theme_mode),
            toggle_theme: core.client_kernel.as_ref().map(|_| toggle_theme),
        };

        let css_texts = CSS_TEXT.with(|c| c.borrow().clone());
        rsx! {
            for (i, css) in css_texts.iter().enumerate() {
                document::Style { key: "almadar-css-{i}", href: "almadar-css-{i}", "{css}" }
            }
            div { class: "almadar-orbital-dioxus p-4 h-full min-h-full bg-background surface-page text-foreground", "data-state": "{core.current_state}", "data-theme": "{theme}", "data-reduced-motion": "{reduced_motion}",
                onmousedown: move |_| {
                    #[cfg(all(feature = "native-audio", not(target_arch = "wasm32")))]
                    crate::audio::user_gesture();
                },
                onkeydown: move |_| {
                    #[cfg(all(feature = "native-audio", not(target_arch = "wasm32")))]
                    crate::audio::user_gesture();
                },
                div { class: "ui-slot-renderer relative min-h-full",
                    div { class: "flex min-h-full flex-col lg:flex-row",
                        for slot_name in layout_slots.iter() {
                            div { key: "{slot_name}", class: "{slot_wrapper_class(slot_name)}", "data-slot": "{slot_name}",
                                for sc in painted_slot_contents_at(core, slot_name, now).iter() {
                                    {paint_slot_content(registry, sc, &view.for_source(sc.source_trait.clone(), source_sink.call(sc.source_trait.clone())))}
                                }
                                if slot_name == "main" {
                                    if let Some(refusal) = core.circuit_fault_content() {
                                        {paint_slot_content(registry, &refusal, &view.for_source(refusal.source_trait.clone(), source_sink.call(refusal.source_trait.clone())))}
                                    }
                                    if let Some(fault) = core.mount_fault_content() {
                                        {paint_slot_content(registry, &fault, &view.for_source(fault.source_trait.clone(), source_sink.call(fault.source_trait.clone())))}
                                    }
                                }
                            }
                        }
                    }
                    for slot_name in overlay_slots.iter() {
                        div { key: "{slot_name}", class: "{slot_wrapper_class(slot_name)}", "data-slot": "{slot_name}",
                            for sc in painted_slot_contents_at(core, slot_name, now).iter() {
                                {paint_slot_content(registry, sc, &view.for_source(sc.source_trait.clone(), source_sink.call(sc.source_trait.clone())))}
                            }
                        }
                    }
                }
            }
        }
    })
}

/// One slot entry: a bare `@trait.X` embed paints that trait's content, any other entry its pattern.
fn paint_slot_content(
    registry: &DioxusPatternRegistry,
    content: &SlotContent,
    view: &DioxusView<'_>,
) -> Element {
    match &content.props {
        Value::String(_) => view.render_node(registry, &content.props),
        props => registry.render_spec(props, content.entity.as_ref(), view),
    }
}

/// Whether `slot` sits in the sidebar/main row (the web's `flex min-h-full flex-col lg:flex-row`).
fn in_layout_row(slot: &str) -> bool {
    matches!(slot_role(slot), SlotRole::Sidebar | SlotRole::Main)
}

/// Paint order of the web's `UISlotRenderer`: the sidebar/main row, then dock, modal, drawer, overlay, center,
/// toast, hud-top, hud-bottom, floating; any other slot follows by its role, then by name.
fn slot_paint_rank(slot: &str) -> (u8, u8, String) {
    let role = slot_role(slot);
    let group = match role {
        SlotRole::Sidebar => 0,
        SlotRole::Main => 1,
        _ => 2,
    };
    let within = match (role, slot) {
        (SlotRole::Sidebar | SlotRole::Main, "sidebar" | "main") => 0,
        (SlotRole::Sidebar | SlotRole::Main, _) => 1,
        (_, "dock") => 0,
        (_, "modal") => 1,
        (_, "drawer") => 2,
        (_, "overlay") => 3,
        (_, "center") => 4,
        (_, "toast") => 5,
        (_, "hud-top") => 6,
        (_, "hud-bottom") => 7,
        (_, "floating") => 8,
        (SlotRole::Modal, _) => 1,
        (SlotRole::Drawer, _) => 2,
        (SlotRole::Overlay, _) => 3,
        (SlotRole::Toast, _) => 5,
        (SlotRole::HudTop, _) => 6,
        (SlotRole::HudBottom, _) => 7,
        (SlotRole::Floating, _) => 8,
        _ => 9,
    };
    (group, within, slot.to_string())
}

/// The wrapper classes the web's `UISlotRenderer` gives `slot`, chosen by core's `slot_role`.
fn slot_wrapper_class(slot: &str) -> String {
    let region = match slot_role(slot) {
        SlotRole::Sidebar => "ui-slot-sidebar min-w-0 lg:shrink-0",
        SlotRole::Main => "ui-slot-main flex-1 min-w-0 min-h-[200px]",
        SlotRole::HudTop => "fixed top-0 inset-x-0 z-40",
        SlotRole::HudBottom => "fixed bottom-0 inset-x-0 z-40",
        SlotRole::Floating => "fixed z-50",
        _ => "",
    };
    format!("ui-slot ui-slot-{slot} {region}")
        .trim_end()
        .to_string()
}

/// Compile-time proof that the shared substrate links into this crate without
/// egui. Called nowhere at runtime; if it type-checks, the seam holds.
pub fn probe_shared_substrate() {
    let _circuit = std::any::type_name::<crate::client_core::ClientCore>();
    let _ctx = std::any::type_name::<crate::RenderContext<'_>>();
    let _peer = recurse::trait_ref_name("@trait.Search");
}

#[cfg(test)]
mod tests {
    use super::{in_layout_row, slot_paint_rank, slot_wrapper_class};

    #[test]
    fn layout_slots_get_the_web_region_classes() {
        assert_eq!(
            slot_wrapper_class("main"),
            "ui-slot ui-slot-main ui-slot-main flex-1 min-w-0 min-h-[200px]"
        );
        assert_eq!(
            slot_wrapper_class("sidebar"),
            "ui-slot ui-slot-sidebar ui-slot-sidebar min-w-0 lg:shrink-0"
        );
        assert_eq!(
            slot_wrapper_class("hud-top"),
            "ui-slot ui-slot-hud-top fixed top-0 inset-x-0 z-40"
        );
        assert_eq!(
            slot_wrapper_class("hud.health"),
            "ui-slot ui-slot-hud.health fixed top-0 inset-x-0 z-40"
        );
        assert_eq!(
            slot_wrapper_class("hud-bottom"),
            "ui-slot ui-slot-hud-bottom fixed bottom-0 inset-x-0 z-40"
        );
        assert_eq!(
            slot_wrapper_class("floating"),
            "ui-slot ui-slot-floating fixed z-50"
        );
        // control: a portal slot carries only the base classes.
        assert_eq!(slot_wrapper_class("modal"), "ui-slot ui-slot-modal");
    }

    #[test]
    fn slots_paint_in_the_web_order_not_alphabetically() {
        let mut names: Vec<String> = [
            "floating",
            "modal",
            "hud-bottom",
            "main",
            "toast",
            "sidebar",
            "hud-top",
            "drawer",
            "content",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        names.sort_by_cached_key(|n| slot_paint_rank(n));
        assert_eq!(
            names,
            [
                "sidebar",
                "main",
                "content",
                "modal",
                "drawer",
                "toast",
                "hud-top",
                "hud-bottom",
                "floating"
            ]
        );
        assert!(in_layout_row("sidebar") && in_layout_row("main") && in_layout_row("content"));
        assert!(!in_layout_row("modal") && !in_layout_row("floating"));
    }
}
