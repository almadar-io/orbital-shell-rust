//! Default-build `video-embed` body: the web VideoEmbed.tsx DOM for a player that is not playing. The research overlay (feature `research-video`) replaces it with the playing anchor.

use crate::registry::DioxusPatternRegistry;
use crate::renderer::{cn, passthrough_attrs, DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;

pub struct VideoFrameRenderer;

impl DioxusPatternRenderer for VideoFrameRenderer {
    fn pattern_name(&self) -> &str {
        "video-embed"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        _view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        let frame = cn(&[
            "aspect-video w-full overflow-hidden rounded-md bg-muted [&>iframe]:h-full [&>iframe]:w-full",
            &get_string_or(props, "className", ""),
        ]);
        let title = get_string(props, "title").filter(|title| !title.is_empty());
        let video_id = get_string(props, "videoId").filter(|id| !id.is_empty());
        let src = get_string(props, "src").filter(|src| !src.is_empty());
        if video_id.is_some() {
            let label = title.unwrap_or_else(|| "Video player".to_string());
            let attrs: Vec<Attribute> = passthrough_attrs(props, &[])
                .into_iter()
                .filter(|attr| attr.name != "role" && attr.name != "aria-label")
                .collect();
            return rsx! {
                div { class: "{frame}", role: "region", aria_label: "{label}", "data-pattern": "video-embed", ..attrs }
            };
        }
        let attrs = passthrough_attrs(props, &[]);
        match src {
            Some(src) => {
                let label = title.unwrap_or_else(|| "Video player".to_string());
                rsx! {
                    div { class: "{frame}", "data-pattern": "video-embed", ..attrs,
                        iframe { src: "{src}", title: "{label}", class: "border-0", allow: "autoplay; encrypted-media; fullscreen; picture-in-picture", allowfullscreen: true }
                    }
                }
            }
            None => {
                let class = cn(&[
                    &frame,
                    "flex items-center justify-center text-sm text-muted-foreground",
                ]);
                let label = get_string(props, "noVideoLabel")
                    .filter(|label| !label.is_empty())
                    .unwrap_or_else(|| "No video selected".to_string());
                rsx! {
                    div { class: "{class}", "data-pattern": "video-embed", ..attrs,
                        span { "{label}" }
                    }
                }
            }
        }
    }
}
