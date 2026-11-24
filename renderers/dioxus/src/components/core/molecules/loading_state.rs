use crate::renderer::merge_class_name;
use crate::registry::{shared_root_attrs, DioxusPatternRegistry};
use crate::renderer::{DioxusPatternRenderer, DioxusView};
use dioxus::prelude::*;
use orbital_core::client::helpers::*;
use orbital_core::{Value, ValueMap};

pub struct LoadingStateRenderer;

fn typography(variant: &str, content: &str, color: &str, class: &str) -> Value {
    let mut props = ValueMap::new();
    props.insert("type".into(), Value::string("typography"));
    props.insert("variant".into(), Value::string(variant));
    props.insert("content".into(), Value::string(content));
    props.insert("color".into(), Value::string(color));
    props.insert("className".into(), Value::string(class));
    Value::Object(props.into())
}

impl DioxusPatternRenderer for LoadingStateRenderer {
    fn pattern_name(&self) -> &str {
        "loading-state"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
        if get_bool_or(props, "untilReady", false) {
            return rsx! { BootLoader { spec: props.clone() } };
        }
        let title = get_string(props, "title").filter(|title| !title.is_empty());
        let message =
            get_string(props, "message").unwrap_or_else(|| crate::i18n::t("common.loading"));
        let full_page = get_bool_or(props, "fullPage", false);
        let layout = if full_page {
            "fixed inset-0 z-[1000] py-12 bg-background/80 backdrop-blur-sm animate-overlay-in"
        } else {
            "py-12"
        };
        let class = merge_class_name(
            &format!("flex flex-col items-center justify-center {layout}"),
            props,
        );
        let mut spinner_props = ValueMap::new();
        spinner_props.insert("type".into(), Value::string("spinner"));
        spinner_props.insert("size".into(), Value::string("lg"));
        let spinner = Value::Object(spinner_props.into());
        let title_spec = title
            .as_deref()
            .map(|title| typography("h3", title, "primary", "mt-4 text-lg"));
        let message_class = if title.is_some() { "mt-2" } else { "mt-4" };
        let message_spec = typography("small", &message, "muted", message_class);
        rsx! {
            div { class: "contents", "data-loading-state": "", ..shared_root_attrs(props),
                div { class: "{class}",
                    {registry.render_spec(&spinner, None, view)}
                    if let Some(title) = title_spec { {registry.render_spec(&title, None, view)} }
                    {registry.render_spec(&message_spec, None, view)}
                }
            }
        }
    }
}

/// The web's page boot loader (`untilReady`): over the whole viewport until every scene canvas has drawn a frame its
/// program marked ready, then gone. It shows `title`, the `messages` one at a time for `messageMs` each, three dots
/// and `message` as its visible status, in `foreground` over `background`.
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
#[component]
fn BootLoader(spec: Value) -> Element {
    let watcher = use_signal(|| 0u64);
    use_hook(move || crate::game::watch_scenes_ready(watcher));
    use_drop(move || crate::game::unwatch_scenes_ready(watcher));
    let _ = watcher();
    let lifted = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(false)));
    let ready = crate::game::scenes_ready();
    if !loader_covers(lifted.get(), ready) {
        lifted.set(true);
        return rsx! {};
    }
    let props = &spec;
    let title = get_string(props, "title").filter(|title| !title.is_empty());
    let message = get_string(props, "message").unwrap_or_else(|| crate::i18n::t("common.loading"));
    let lines = get_string_array(props, "messages");
    let message_ms = get_number_or(props, "messageMs", 5000.0).max(1.0) as u64;
    let background = get_string(props, "background").unwrap_or_default();
    let foreground = get_string(props, "foreground").unwrap_or_default();
    let cover = format!(
        "position:fixed;inset:0;z-index:1000;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:24px;padding:0 32px;background:{background};color:{foreground}"
    );
    let dot = "width:8px;height:8px;border-radius:50%;background:currentColor;opacity:.6";
    rsx! {
        div { role: "status", aria_live: "polite", "data-orb-loader": "", style: "{cover}", ..shared_root_attrs(props),
            if let Some(title) = title { h3 { class: "text-lg", style: "color:{foreground}", "{title}" } }
            if !lines.is_empty() {
                LoaderLines { lines, message_ms, color: foreground.clone() }
            }
            div { style: "display:flex;flex-direction:column;align-items:center;gap:8px",
                div { "aria-hidden": "true", style: "display:flex;gap:8px;justify-content:center",
                    div { style: "{dot}" }
                    div { style: "{dot}" }
                    div { style: "{dot}" }
                }
                small { style: "color:{foreground};opacity:.75", "{message}" }
            }
        }
    }
}

/// Whether the boot loader covers the page: until the page is first ready, and never again once it lifted (the web's
/// boot loader is gone for good after the first ready frame; a scroll or a late image dropping readiness is not a boot).
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
fn loader_covers(lifted: bool, ready: bool) -> bool {
    !lifted && !ready
}

/// The loader's lines, one up at a time for `message_ms` each, in turn.
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
#[component]
fn LoaderLines(lines: Vec<String>, message_ms: u64, color: String) -> Element {
    let mut shown = use_signal(|| 0usize);
    let count = lines.len();
    use_future(move || async move {
        loop {
            crate::timer::Delay::new(std::time::Duration::from_millis(message_ms)).await;
            let next = (*shown.peek() + 1) % count.max(1);
            shown.set(next);
        }
    });
    let line = lines.get(*shown.read() % count.max(1)).cloned().unwrap_or_default();
    rsx! {
        p { style: "max-width:36rem;text-align:center;color:{color}", "{line}" }
    }
}

#[cfg(all(test, feature = "native", not(target_arch = "wasm32")))]
mod tests {
    use super::loader_covers;

    #[test]
    fn the_boot_loader_lifts_once_and_does_not_return_when_readiness_drops() {
        assert!(loader_covers(false, false), "before the page was ever ready the loader covers it");
        assert!(!loader_covers(false, true), "control: the first ready frame lifts it");
        assert!(!loader_covers(true, false), "a scroll or a late image drops readiness after the lift: the page stays shown");
        assert!(!loader_covers(true, true), "control: lifted and ready stays lifted");
    }
}
