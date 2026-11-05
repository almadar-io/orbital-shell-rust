use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use crate::video_overlay::VideoAnchor;
use dioxus::prelude::*;
use orbital_core::Value;

pub struct VideoEmbedRenderer;

impl DioxusPatternRenderer for VideoEmbedRenderer {
    fn pattern_name(&self) -> &str {
        "video-embed"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { VideoAnchor { props: props.clone(), sink: view.sink } }
    }
}
