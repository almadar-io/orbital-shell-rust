use crate::renderer::merge_class_name;
use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView, EventSink};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::Value;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use crate::visibility::{intersection, root_margin, use_visibility};

pub struct InfiniteScrollSentinelRenderer;

#[derive(Props, Clone, PartialEq)]
struct SentinelProps {
    value: Value,
    sink: Option<EventSink>,
}

#[allow(non_snake_case)]
fn Sentinel(props: SentinelProps) -> Element {
    let is_loading = get_bool_or(&props.value, "isLoading", false);
    let has_more = get_bool_or(&props.value, "hasMore", true);
    let threshold = get_string_or(&props.value, "threshold", "200px");
    let event = get_string(&props.value, "loadMoreEvent");
    let payload = props
        .value
        .as_object()
        .and_then(|fields| fields.get("loadMorePayload"))
        .cloned()
        .unwrap_or_else(|| Value::Object(orbital_core::ValueMap::new().into()));
    let class = merge_class_name(
        "almadar-infinite-scroll-sentinel flex items-center justify-center py-4",
        &props.value,
    );
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    let (visibility_id, snapshot) = use_visibility();
    #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
    let visibility_id = 0_usize;
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    let visible = root_margin(&threshold, snapshot().viewport)
        .is_some_and(|margin| intersection(snapshot(), margin).visible);
    #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
    let visible = false;
    let fired = use_hook(|| Rc::new(RefCell::new(false)));
    let sink = props.sink;
    let mut effect = use_effect(move || {
        let eligible = visible && has_more && !is_loading;
        let mut fired = fired.borrow_mut();
        if eligible && !*fired {
            if let (Some(sink), Some(event)) = (sink, &event) {
                *fired = true;
                sink.call((event.clone(), payload.clone()));
            }
        } else if !eligible {
            *fired = false;
        }
    });
    effect.mark_dirty();
    rsx! {
        div { class: "{class}",
            div { class: "h-1 w-full", "data-orbital-visibility": "{visibility_id}", aria_hidden: "true" }
            if is_loading { span { class: "almadar-spinner" } }
        }
    }
}

impl DioxusPatternRenderer for InfiniteScrollSentinelRenderer {
    fn pattern_name(&self) -> &str {
        "infinite-scroll-sentinel"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        _registry: &DioxusPatternRegistry,
    ) -> Element {
        rsx! { Sentinel { value: props.clone(), sink: view.sink } }
    }
}
