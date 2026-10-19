//! Dioxus renderer backend for Orbital.
//!
//! Separate crate from `orbital-client` (egui). It reuses the shared,
//! backend-agnostic substrate that already lives in `orbital-client` — the
//! `ClientCore` event circuit + `@trait` index and the `recurse` child resolver —
//! by depending on `orbital-client` with `default-features = false` (so egui /
//! eframe / ratatui / wgpu do NOT compile here). No parallel circuit, no second
//! child resolver: one source, two paint backends (egui immediate-mode, Dioxus
//! retained VDOM).

pub mod app;
// Generated pattern renderers: scaffold whose unused imports/params vary by
// atom (a leaf renderer ignores `registry`/`entity`). Scoped here, not
// crate-wide, so hand-written code keeps its dead-code signal. Mirrors the
// `pub mod components;` allow in `orbital-client/src/rendering/mod.rs`.
#[allow(dead_code, unused_variables, unused_imports)]
pub mod components;
pub mod registry;
pub mod renderer;
#[cfg(feature = "native")]
pub mod offscreen;
pub mod probe_target;
pub mod styles;
pub mod shell;

// Browser (wasm32) shell: the native (Blitz) render path painted into a
// <canvas> via vello/wgpu — no DOM rendering (that stays React's job in
// @almadar/ui). Desktop keeps `native::launch_cfg`; wasm runs the glue in
// `wasm.rs` because blitz-shell's winit event loop has no wasm target.
#[cfg(all(feature = "native", target_arch = "wasm32"))]
pub mod wasm;

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub mod native;

/// Game pattern renderers (`canvas` / `canvas-2d` via a shared-device wgpu paint
/// source, `game-hud`) — desktop-only; the GPU substrate has no wasm32 build.
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub mod game;

/// Native behavior catalog (desktop counterpart of the archived ui-wasm browser
/// catalog) — sidebar of every baked behavior + event chips driving the
/// mounted state machine.
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub mod catalog;

// Re-export the shared substrate from one path so generated renderers and `app`
// reach it through one path (keeps the seam grep-able).
pub use orbital_core::client::client_core;
pub use orbital_core::client::helpers;
pub use orbital_core::client::recurse;
pub use orbital_core::client::tick;
pub use orbital_core::client::helpers::{RenderContext, SlotContent};
pub use renderer::{DioxusPatternRenderer, DioxusView, EventSink};
pub use registry::DioxusPatternRegistry;
