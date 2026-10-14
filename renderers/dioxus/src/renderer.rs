//! Dioxus pattern-renderer contract.
//!
//! Mirrors `orbital-client/src/rendering/desktop.rs:42-54` (`PatternRenderer`)
//! but returns a Dioxus [`Element`] (RSX, retained VDOM) instead of painting
//! into `&mut egui::Ui` (immediate-mode).
//!
//! This contract is intentionally *separate* from the shared [`RenderContext`]
//! (which is `&mut`-heavy and egui-shaped). Dioxus renderers are pure functions
//! `props → Element`: they read entity/trait data from a read-only [`DioxusView`],
//! emit user events through an [`EventSink`] (a `Callback` the root `App` wires
//! into `ClientCore::send_event`), and recurse into children through the
//! [`DioxusPatternRegistry`] (no `ui_ptr`, no `&mut`). Nothing here touches the
//! egui substrate — the shared `RenderContext` is left byte-identical.

use crate::registry::DioxusPatternRegistry;
use dioxus::prelude::*;
use orbital_core::Value;
use std::collections::HashMap;

/// Event sink a Dioxus control calls to dispatch a user event back into the
/// Orbital circuit. `(event_name, payload)` — `Callback` is `Copy`, so controls
/// capture it inside `onclick: move |_| …` without cloning.
pub type EventSink = Callback<(String, Value)>;

/// Read-only view handed to a Dioxus renderer. Carries the immutable subset of
/// the shared `RenderContext` renderers actually need (entity cache, `@trait`
/// content, current state) plus the [`EventSink`] for dispatch. `source_trait`
/// mirrors `RenderContext::source_trait` so a button nested under `@trait.X`
/// emits with source `X` (switched per subtree in later phases; Phase 1 atoms
/// have no `@trait.X` nesting).
pub struct DioxusView<'a> {
    /// Name of the currently active trait.
    pub trait_name: &'a str,
    /// Name of the currently active orbital.
    pub orbital_name: &'a str,
    /// Name of the current state machine state.
    pub current_state: &'a str,
    /// Cached entity data returned by the server.
    pub entities: &'a HashMap<String, Vec<Value>>,
    /// Per-trait current render output, keyed by trait name (for `@trait.X`).
    pub trait_content: &'a orbital_core::ValueMap,
    /// The trait that authored the content currently being rendered — the source
    /// of any event a control here dispatches.
    pub source_trait: Option<String>,
    /// Event sink wired by the root `App` into `ClientCore::send_event`.
    pub sink: Option<EventSink>,
    /// The kernel's `canvasTheme`, handed to every Rust-drawn scene (`None` without a kernel).
    pub canvas_theme: Option<serde_json::Value>,
}

/// A named UI pattern rendered to a Dioxus subtree.
impl<'a> DioxusView<'a> {
    /// This view for content `source` rendered, dispatching through `sink` (bound to it).
    pub fn for_source(&self, source: Option<String>, sink: EventSink) -> DioxusView<'a> {
        DioxusView {
            trait_name: self.trait_name,
            orbital_name: self.orbital_name,
            current_state: self.current_state,
            entities: self.entities,
            trait_content: self.trait_content,
            source_trait: source,
            sink: Some(sink),
            canvas_theme: self.canvas_theme.clone(),
        }
    }
}

pub trait DioxusPatternRenderer: Send + Sync {
    /// The pattern this renderer handles (e.g. "button").
    fn pattern_name(&self) -> &str;

    /// Render the pattern to a Dioxus subtree. `registry` is passed so container
    /// renderers (`stack`, grids) can recurse into their `children` by calling
    /// [`DioxusPatternRegistry::render_spec`]; leaf renderers ignore it.
    fn render(
        &self,
        props: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element;
}
