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
use crate::renderer::{DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::Value;
use std::cell::RefCell;

/// Everything a running Dioxus Orbital app needs: the shared circuit and the
/// pattern registry that paints its slots.
pub struct AppState {
    /// The ONE event circuit (shared with egui via `orbital-client`).
    pub core: ClientCore,
    /// Dioxus pattern registry (built once at mount from the generated sets).
    pub registry: DioxusPatternRegistry,
    /// Scheduled-tick driver built from the active orbital's `ticks` (the same
    /// scheduler the egui game shell runs; fired by the native frame hook).
    pub tick_scheduler: crate::tick::TickScheduler,
}

std::thread_local! {
    /// Mounted app state. `None` before `web::start` mounts a schema.
    pub static APP: RefCell<Option<AppState>> = RefCell::new(None);
    /// Re-render poke: the root component's version signal, registered so the
    /// external `web::send_event` (driven from JS, outside a Dioxus render) can
    /// invalidate the component exactly like an in-app click does.
    static VERSION: RefCell<Option<Signal<u32>>> = RefCell::new(None);
    /// Stylesheet texts injected into the document head at mount. Set by the
    /// native (Blitz) shell from `--css` files — Blitz resolves `class:`
    /// attributes against them (almadar.css base + Tailwind utilities). Empty
    /// on web, where CSS arrives via `<link>` in the preview HTML instead.
    pub static CSS_TEXT: RefCell<Vec<String>> = RefCell::new(Vec::new());
    /// Active design-system theme (`@almadar/ui` `[data-theme="…"]` blocks in
    /// `almadar-theme.css`). `None` → the `:root:not([data-theme])` fallback
    /// theme applies, exactly like the React shell. Read by the root `App`
    /// component into the `data-theme` attribute.
    pub static THEME: RefCell<Option<String>> = RefCell::new(None);
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

/// Root Orbital component. Reads [`APP`], paints `ui_slots` through the registry,
/// and wires an [`EventSink`] so button clicks drive the shared circuit.
#[component]
pub fn App() -> Element {
    let mut version = use_signal(|| 0u32);
    // Register the version signal for external pokes and subscribe to it.
    VERSION.with(|v| *v.borrow_mut() = Some(version));
    let _tick = *version.read();

    // A control dispatches as the trait whose content rendered it (the egui shell's
    // `RenderContext::queue_event`), the active trait when the content carries no source.
    let routed = use_callback(
        move |(event, source, payload): (String, Option<String>, Value)| {
            APP.with(|cell| {
                if let Some(state) = cell.borrow_mut().as_mut() {
                    state.core.dispatch_control(&event, source.as_deref(), payload);
                    state.core.settle(8);
                }
            });
            let next = version.peek().wrapping_add(1);
            version.set(next);
        },
    );
    // One sink per source trait, made once: a callback made per render lives as long as this root scope.
    let bound = use_hook(|| {
        std::rc::Rc::new(RefCell::new(std::collections::HashMap::<
            Option<String>,
            EventSink,
        >::new()))
    });
    let sink_for = move |source: Option<String>| -> EventSink {
        *bound.borrow_mut().entry(source.clone()).or_insert_with(|| {
            Callback::new(move |(event, payload): (String, Value)| {
                routed.call((event, source.clone(), payload))
            })
        })
    };
    let sink = sink_for(None);

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
        let mut slot_names: Vec<String> = core.ui_slots.keys().cloned().collect();
        slot_names.sort();

        let view = DioxusView {
            trait_name: &core.current_trait_name,
            orbital_name: &orbital,
            current_state: &core.current_state,
            entities: &core.entities,
            trait_content: &core.trait_content,
            source_trait: None,
            sink: Some(sink),
            canvas_theme: core.canvas_theme(),
        };

        let css_texts = CSS_TEXT.with(|c| c.borrow().clone());
        // Empty string = no theme: `:root:not([data-theme])` still matches the
        // html element (the attr lives on this div), so the fallback theme's
        // variables inherit down — same default as the React shell.
        let theme = THEME.with(|t| t.borrow().clone()).unwrap_or_default();
        rsx! {
            for (i, css) in css_texts.iter().enumerate() {
                document::Style { key: "almadar-css-{i}", href: "almadar-css-{i}", "{css}" }
            }
            div { class: "almadar-orbital-dioxus", "data-state": "{core.current_state}", "data-theme": "{theme}",
                for slot_name in slot_names.iter() {
                    div { class: "almadar-slot", "data-slot": "{slot_name}",
                        for sc in core.slot_contents(slot_name).iter() {
                            {registry.render_spec(&sc.props, sc.entity.as_ref(), &view.for_source(sc.source_trait.clone(), sink_for(sc.source_trait.clone())))}
                        }
                    }
                }
            }
        }
    })
}

/// Compile-time proof that the shared substrate links into this crate without
/// egui. Called nowhere at runtime; if it type-checks, the seam holds.
pub fn probe_shared_substrate() {
    let _circuit = std::any::type_name::<crate::client_core::ClientCore>();
    let _ctx = std::any::type_name::<crate::RenderContext<'_>>();
    let _peer = recurse::trait_ref_name("@trait.Search");
}
