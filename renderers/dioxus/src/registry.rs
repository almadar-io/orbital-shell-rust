//! Registry mapping pattern names to Dioxus renderers.
//!
//! Mirrors `orbital-client/src/rendering/desktop.rs:60-121`: replace-by-name on
//! register. A pattern with no renderer gets the declared unsupported-pattern
//! error (`orbital_core::renderer_coverage`), never a placeholder.

use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::Value;
use std::collections::HashMap;

/// This renderer's id in the declared unsupported-pattern error.
pub const RENDERER_ID: &str = "rust-dioxus";

/// Game patterns this renderer stubs for now (G-ORB-238): they draw the declared
/// unsupported answer, and the render probe reports them as stubbed, not failed.
/// Exactly the registry patterns this renderer lacks — `tests/renderer_coverage.rs`
/// keeps the two equal.
pub const STUBBED_PATTERNS: &[&str] = &[
    "action-palette", "action-tile", "choice-button", "control-button", "control-grid",
    "dialogue-bubble", "draw-fx-layer", "draw-group", "draw-mesh", "draw-shape",
    "draw-shape-layer", "draw-skinned-mesh", "draw-sprite", "draw-sprite-layer", "draw-text",
    "draw-text-layer", "game-audio-cue", "game-audio-toggle", "game-menu", "game-shell",
    "health-bar", "pattern-tile", "score-display", "sequence-bar", "stat-badge",
    "timer-display", "trait-slot",
];

/// The answer for a pattern this client does not implement.
struct UnsupportedDioxusRenderer;

impl DioxusPatternRenderer for UnsupportedDioxusRenderer {
    fn pattern_name(&self) -> &str {
        "*"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        _view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let t = props
            .as_object()
            .and_then(|o| o.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let message = orbital_core::renderer_coverage::UnsupportedPattern::new(
            t,
            RENDERER_ID,
            env!("CARGO_PKG_VERSION"),
        )
        .message();
        rsx! {
            div {
                class: "almadar-unsupported-pattern",
                role: "alert",
                "data-code": orbital_core::renderer_coverage::CODE_PATTERN_UNSUPPORTED,
                "{message}"
            }
        }
    }
}

/// Registry mapping pattern names to their Dioxus renderers.
pub struct DioxusPatternRegistry {
    renderers: HashMap<String, Box<dyn DioxusPatternRenderer>>,
    fallback: Box<dyn DioxusPatternRenderer>,
}

impl DioxusPatternRegistry {
    /// Create a registry. Phase 0: the unsupported-pattern answer only. Phase 1+ registers the
    /// generated `components::<target>::register::register_dioxus` sets and the
    /// hand-authored overrides (last, so they win by name).
    pub fn new() -> Self {
        Self {
            renderers: HashMap::new(),
            fallback: Box::new(UnsupportedDioxusRenderer),
        }
    }

    /// Register a renderer, replacing any existing renderer for the same pattern.
    pub fn register(&mut self, renderer: Box<dyn DioxusPatternRenderer>) {
        let name = renderer.pattern_name().to_string();
        self.renderers.insert(name, renderer);
    }

    /// The patterns this client implements, sorted — its declared coverage list.
    pub fn coverage(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.renderers.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    /// Whether `name` has a renderer here.
    pub fn supports(&self, name: &str) -> bool {
        self.renderers.contains_key(name)
    }

    /// Whether `name` is one of this renderer's declared stubs.
    pub fn is_stub(&self, name: &str) -> bool {
        !self.supports(name) && STUBBED_PATTERNS.contains(&name)
    }

    /// Look up a renderer by pattern name, else the unsupported-pattern answer.
    pub fn get_renderer(&self, name: &str) -> &dyn DioxusPatternRenderer {
        self.renderers
            .get(name)
            .map(|r| r.as_ref())
            .unwrap_or_else(|| self.fallback.as_ref())
    }

    /// Render one pattern spec (a render-ui node: `{ "type": "<pattern>", … }`)
    /// to a Dioxus subtree. Container renderers call this to recurse into their
    /// `children`. Two shared borrows of `self` coexist (the map lookup and the
    /// `self` passed down), which is allowed and keeps the recursion lock-free.
    /// A spec that is an ARRAY of nodes (e.g. `@config.master` holding a node
    /// list) expands element-wise — the same contract the egui `render_child`
    /// hook follows, so a slot never falls back on a list.
    pub fn render_spec(
        &self,
        spec: &Value,
        entity: Option<&Value>,
        view: &DioxusView<'_>,
    ) -> Element {
        if let Value::Array(items) = spec {
            return rsx! {
                for item in items.iter() {
                    {self.render_spec(item, entity, view)}
                }
            };
        }
        let name = spec
            .as_object()
            .and_then(|o| o.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let renderer = self
            .renderers
            .get(name)
            .map(|r| r.as_ref())
            .unwrap_or_else(|| self.fallback.as_ref());
        renderer.render(spec, entity, view, self)
    }
}

/// The registry this client ships: the generated core and game sets, then the
/// hand-authored game overrides (registered last so they win by name; canvas /
/// canvas-2d / game-hud have no generated dioxus body).
pub fn default_registry() -> DioxusPatternRegistry {
    let mut registry = DioxusPatternRegistry::new();
    crate::components::core::register::register_dioxus(&mut registry);
    crate::components::game::register::register_dioxus(&mut registry);
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    crate::game::register_overrides(&mut registry);
    registry
}

impl Default for DioxusPatternRegistry {
    fn default() -> Self {
        Self::new()
    }
}
