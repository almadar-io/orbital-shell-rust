use crate::registry::DioxusPatternRegistry;
use crate::renderer::DioxusView;
use dioxus::prelude::*;
use orbital_core::Value;
#[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
use orbital_core::ValueMap;

pub fn fill_parent(
    asset: &Value,
    mode: &str,
    view: &DioxusView<'_>,
    registry: &DioxusPatternRegistry,
) -> Element {
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    {
        let _ = registry;
        crate::game::fill_parent_atlas_panel(asset, mode, view)
    }
    #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
    {
        let mut spec = ValueMap::new();
        spec.insert("type".into(), Value::String("atlas-panel".into()));
        spec.insert("asset".into(), asset.clone());
        spec.insert("mode".into(), Value::String(mode.into()));
        registry.render_spec(&Value::Object(spec.into()), None, view)
    }
}
