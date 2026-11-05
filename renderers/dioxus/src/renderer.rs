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
use orbital_core::client::recurse::trait_ref_name;
use orbital_core::Value;
use std::collections::HashMap;

/// Event sink a Dioxus control calls to dispatch a user event back into the
/// Orbital circuit. `(event_name, payload)` — `Callback` is `Copy`, so controls
/// capture it inside `onclick: move |_| …` without cloning.
pub type EventSink = Callback<(String, Value)>;
pub type SourceSinkFactory = Callback<Option<String>, EventSink>;

pub(crate) fn emit_empty_event(sink: Option<EventSink>, name: Option<&str>) {
    if let (Some(sink), Some(name)) = (sink, name.filter(|name| !name.is_empty())) {
        sink.call((name.to_string(), Value::Null));
    }
}

/// A row action's route; a route that fails to evaluate is logged and the action draws no link.
pub(crate) fn logged_row_route(action: &Value, row: &Value) -> Option<String> {
    orbital_core::client::helpers::row_action_route(action, row, None).unwrap_or_else(|error| {
        tracing::error!(%error, "row action route");
        None
    })
}

/// A row action's payload; a payload that fails to evaluate is logged and the action is not drawn or dispatched.
pub(crate) fn logged_row_payload(action: &Value, row: &Value) -> Option<Value> {
    orbital_core::client::helpers::row_action_payload(action, row, None)
        .map_err(|error| tracing::error!(%error, "row action payload"))
        .ok()
}

/// A sub-pattern spec `{ type: name, ..fields }` a body hands to `registry.render_spec`, so it composes the
/// pattern's own renderer instead of re-drawing it.
pub(crate) fn pattern_spec<K: AsRef<str>>(name: &str, fields: impl IntoIterator<Item = (K, Value)>) -> Value {
    let mut spec = orbital_core::ValueMap::new();
    spec.insert("type".into(), Value::string(name));
    for (key, value) in fields {
        spec.insert(key.as_ref().into(), value);
    }
    Value::Object(spec.into())
}

/// An object value from `(key, value)` pairs (an event payload, a nested prop).
pub(crate) fn object_value<K: AsRef<str>>(fields: impl IntoIterator<Item = (K, Value)>) -> Value {
    let mut map = orbital_core::ValueMap::new();
    for (key, value) in fields {
        map.insert(key.as_ref().into(), value);
    }
    Value::Object(map.into())
}

pub type NavigationSink = Callback<String>;

#[derive(Clone, Copy, PartialEq)]
pub enum DockCommand {
    ShowPanel,
    CloseMenu,
}

#[derive(Clone, PartialEq)]
pub struct DockContext {
    pub compact: bool,
    pub command: Callback<DockCommand>,
}

/// Read-only view handed to a Dioxus renderer. Carries the immutable subset of
/// the shared `RenderContext` renderers actually need (entity cache, `@trait`
/// content, current state) plus the [`EventSink`] for dispatch. `source_trait`
/// mirrors `RenderContext::source_trait` so a button nested under `@trait.X`
/// emits with source `X`.
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
    pub source_sink: SourceSinkFactory,
    pub navigate: Option<NavigationSink>,
    pub dock: Option<DockContext>,
    pub viewport_width: Option<f64>,
    /// The kernel's `canvasTheme`, handed to every Rust-drawn scene (`None` without a kernel).
    pub canvas_theme: Option<serde_json::Value>,
    pub theme_mode: Option<orbital_core::ThemeMode>,
    pub toggle_theme: Option<Callback<()>>,
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
            source_sink: self.source_sink,
            navigate: self.navigate,
            dock: self.dock.clone(),
            viewport_width: self.viewport_width,
            canvas_theme: self.canvas_theme.clone(),
            theme_mode: self.theme_mode,
            toggle_theme: self.toggle_theme,
        }
    }

    pub fn for_trait_source(&self, source: String) -> DioxusView<'a> {
        let sink = self.source_sink.call(Some(source.clone()));
        self.for_source(Some(source), sink)
    }

    pub fn render_child(
        &self,
        registry: &DioxusPatternRegistry,
        child: &Value,
        source: Option<&str>,
    ) -> Element {
        match source {
            Some(source) => {
                registry.render_spec(child, None, &self.for_trait_source(source.to_string()))
            }
            None => registry.render_spec(child, None, self),
        }
    }

    pub fn render_node(&self, registry: &DioxusPatternRegistry, node: &Value) -> Element {
        match node {
            Value::Array(items) => rsx! {
                for item in items.iter() {
                    {self.render_node(registry, item)}
                }
            },
            Value::String(text) => {
                if let Some(source) = trait_ref_name(text) {
                    if let Some(content) = self.trait_content.get(&source) {
                        registry.render_spec(content, None, &self.for_trait_source(source))
                    } else {
                        rsx! {}
                    }
                } else {
                    rsx! { "{text}" }
                }
            }
            Value::Object(_) => registry.render_spec(node, None, self),
            _ => rsx! {},
        }
    }

    pub fn to_owned(&self) -> OwnedDioxusView {
        OwnedDioxusView {
            trait_name: self.trait_name.to_string(),
            orbital_name: self.orbital_name.to_string(),
            current_state: self.current_state.to_string(),
            entities: self.entities.clone(),
            trait_content: self.trait_content.clone(),
            source_trait: self.source_trait.clone(),
            sink: self.sink,
            source_sink: self.source_sink,
            navigate: self.navigate,
            dock: self.dock.clone(),
            viewport_width: self.viewport_width,
            canvas_theme: self.canvas_theme.clone(),
            theme_mode: self.theme_mode,
            toggle_theme: self.toggle_theme,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct OwnedDioxusView {
    pub trait_name: String,
    pub orbital_name: String,
    pub current_state: String,
    pub entities: HashMap<String, Vec<Value>>,
    pub trait_content: orbital_core::ValueMap,
    pub source_trait: Option<String>,
    pub sink: Option<EventSink>,
    pub source_sink: SourceSinkFactory,
    pub navigate: Option<NavigationSink>,
    pub dock: Option<DockContext>,
    pub viewport_width: Option<f64>,
    pub canvas_theme: Option<serde_json::Value>,
    pub theme_mode: Option<orbital_core::ThemeMode>,
    pub toggle_theme: Option<Callback<()>>,
}

impl OwnedDioxusView {
    pub fn as_view(&self) -> DioxusView<'_> {
        DioxusView {
            trait_name: &self.trait_name,
            orbital_name: &self.orbital_name,
            current_state: &self.current_state,
            entities: &self.entities,
            trait_content: &self.trait_content,
            source_trait: self.source_trait.clone(),
            sink: self.sink,
            source_sink: self.source_sink,
            navigate: self.navigate,
            dock: self.dock.clone(),
            viewport_width: self.viewport_width,
            canvas_theme: self.canvas_theme.clone(),
            theme_mode: self.theme_mode,
            toggle_theme: self.toggle_theme,
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
