//! Registry mapping pattern names to Dioxus renderers.
//!
//! Mirrors `orbital-client/src/rendering/desktop.rs:60-121`: replace-by-name on
//! register. A pattern with no renderer gets the declared unsupported-pattern
//! error, never a placeholder.

use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::Value;
use std::collections::HashMap;
use std::sync::Arc;

fn entry_props(spec: &Value) -> Option<Value> {
    let fields = spec.as_object()?;
    if !fields.contains_key("enter") && !fields.contains_key("enterDelay") {
        return None;
    }
    let mut props = fields.clone();
    props.shift_remove("enter");
    props.shift_remove("enterDelay");
    if let Some(animation @ ("fade" | "rise" | "scale" | "slide")) =
        fields.get("enter").and_then(Value::as_str)
    {
        let mut class = fields
            .get("className")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !class.is_empty() {
            class.push(' ');
        }
        class.push_str("almadar-enter-");
        class.push_str(animation);
        if let Some(Value::Number(delay)) = fields.get("enterDelay") {
            if delay.fract() == 0.0 && (1.0..=8.0).contains(delay) {
                class.push_str(&format!(" almadar-enter-delay-{}", *delay as u8));
            }
        }
        props.insert("className".into(), Value::String(class.into()));
    }
    Some(Value::Object(props.into()))
}

/// This renderer's id in the declared unsupported-pattern error.
pub const RENDERER_ID: &str = "rust-dioxus";

/// The error code a pattern this renderer lacks carries.
pub const CODE_PATTERN_UNSUPPORTED: &str = "ORB_PATTERN_UNSUPPORTED_BY_RENDERER";

/// Game patterns this renderer stubs for now (G-ORB-238): they draw the declared
/// unsupported answer, and the render probe reports them as stubbed, not failed.
/// Exactly the registry patterns this renderer lacks — `orbital-integration-tests`
/// `tests/native_snapshot_baselines.rs` keeps the two equal.
pub const STUBBED_PATTERNS: &[&str] = &[
    "action-palette",
    "action-tile",
    "activity-rail",
    "avl-transition-detail",
    "choice-button",
    "control-button",
    "control-grid",
    "dialogue-bubble",
    "draw-fx-layer",
    "draw-group",
    "draw-mesh",
    "draw-shape",
    "draw-shape-layer",
    "draw-skinned-mesh",
    "draw-sprite",
    "draw-sprite-layer",
    "draw-text",
    "draw-text-layer",
    "game-audio-cue",
    "game-audio-toggle",
    "game-menu",
    "game-shell",
    "health-bar",
    "machine-panel",
    "pattern-tile",
    "score-display",
    "sequence-bar",
    "stat-badge",
    "timer-display",
    "trait-slot",
    "video-embed",
];

/// Logs the first occurrence per pattern: the client renders every frame, so
/// repeats leave the log, never the screen.
fn unsupported_pattern_message(pattern: &str) -> String {
    static SEEN: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    let pattern = if pattern.is_empty() {
        "(no type)"
    } else {
        pattern
    };
    let message = format!(
        "pattern `{pattern}` is not supported by renderer {RENDERER_ID} {}",
        env!("CARGO_PKG_VERSION")
    );
    let first = SEEN
        .get_or_init(Default::default)
        .lock()
        .map(|mut seen| seen.insert(pattern.to_string()))
        .unwrap_or(true);
    if first {
        tracing::warn!(code = CODE_PATTERN_UNSUPPORTED, "{message}");
    }
    message
}

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
        let message = unsupported_pattern_message(t);
        rsx! {
            div {
                class: "almadar-unsupported-pattern",
                role: "alert",
                "data-code": CODE_PATTERN_UNSUPPORTED,
                "{message}"
            }
        }
    }
}

/// Registry mapping pattern names to their Dioxus renderers.
#[derive(Clone)]
pub struct DioxusPatternRegistry {
    renderers: Arc<HashMap<String, Arc<dyn DioxusPatternRenderer>>>,
    fallback: Arc<dyn DioxusPatternRenderer>,
}

impl PartialEq for DioxusPatternRegistry {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.renderers, &other.renderers)
            && Arc::ptr_eq(&self.fallback, &other.fallback)
    }
}

impl DioxusPatternRegistry {
    /// Create a registry. Phase 0: the unsupported-pattern answer only. Phase 1+ registers the
    /// generated `components::<target>::register::register_dioxus` sets and the
    /// hand-authored overrides (last, so they win by name).
    pub fn new() -> Self {
        Self {
            renderers: Arc::new(HashMap::new()),
            fallback: Arc::new(UnsupportedDioxusRenderer),
        }
    }

    /// Register a renderer, replacing any existing renderer for the same pattern.
    pub fn register(&mut self, renderer: Box<dyn DioxusPatternRenderer>) {
        let name = renderer.pattern_name().to_string();
        Arc::make_mut(&mut self.renderers).insert(name, Arc::from(renderer));
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
        match entry_props(spec) {
            Some(props) => renderer.render(&props, entity, view, self),
            None => renderer.render(spec, entity, view, self),
        }
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
    #[cfg(all(feature = "research-video", not(target_arch = "wasm32")))]
    registry.register(Box::new(
        crate::components::core::atoms::video_embed::VideoEmbedRenderer,
    ));
    #[cfg(all(
        feature = "research-drag",
        feature = "native",
        not(target_arch = "wasm32")
    ))]
    {
        use crate::components::game::molecules::{
            action_palette, action_tile, sequence_bar, trait_slot,
        };
        registry.register(Box::new(action_palette::ActionPaletteRenderer));
        registry.register(Box::new(action_tile::ActionTileRenderer));
        registry.register(Box::new(sequence_bar::SequenceBarRenderer));
        registry.register(Box::new(trait_slot::TraitSlotRenderer));
    }
    #[cfg(all(
        feature = "research-dock",
        feature = "native",
        not(target_arch = "wasm32")
    ))]
    registry.register(Box::new(
        crate::components::core::molecules::activity_rail::ActivityRailRenderer,
    ));
    registry
}

#[cfg(test)]
mod tests {
    use super::entry_props;
    use orbital_core::Value;

    #[test]
    fn entry_props_apply_the_shared_class_and_stagger_vocabulary() {
        let spec: Value = serde_json::from_str(
            r#"{"type":"score-display","className":"accent","enter":"rise","enterDelay":3}"#,
        )
        .unwrap();
        let rendered = entry_props(&spec).unwrap();
        assert_eq!(
            rendered.get("className").and_then(Value::as_str),
            Some("accent almadar-enter-rise almadar-enter-delay-3")
        );
        assert!(rendered.get("enter").is_none());
        assert!(rendered.get("enterDelay").is_none());
    }

    #[test]
    fn entry_none_and_unpaired_delay_add_no_class() {
        for spec in [
            r#"{"type":"score-display","enter":"none","enterDelay":3}"#,
            r#"{"type":"score-display","enterDelay":3}"#,
        ] {
            let rendered = entry_props(&serde_json::from_str(spec).unwrap()).unwrap();
            assert!(rendered.get("className").is_none());
            assert!(rendered.get("enter").is_none());
            assert!(rendered.get("enterDelay").is_none());
        }
    }
}

impl Default for DioxusPatternRegistry {
    fn default() -> Self {
        Self::new()
    }
}
