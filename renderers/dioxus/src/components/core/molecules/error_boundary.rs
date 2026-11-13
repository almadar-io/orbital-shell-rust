use crate::registry::DioxusPatternRegistry;
use crate::renderer::{DioxusPatternRenderer, DioxusView, OwnedDioxusView};
use dioxus::prelude::*;
use crate::renderer::merge_class_name;
use orbital_core::client::recurse::child_patterns;
use orbital_core::Value;

pub struct ErrorBoundaryRenderer;

#[component]
fn BoundaryChild(
    spec: Value,
    source: Option<String>,
    view: OwnedDioxusView,
    registry: DioxusPatternRegistry,
) -> Element {
    view.as_view()
        .render_child(&registry, &spec, source.as_deref())
}

impl DioxusPatternRenderer for ErrorBoundaryRenderer {
    fn pattern_name(&self) -> &str {
        "error-boundary"
    }

    fn render(
        &self,
        props: &Value,
        _entity: Option<&Value>,
        view: &DioxusView<'_>,
        registry: &DioxusPatternRegistry,
    ) -> Element {
        let children = child_patterns(props, view.trait_content);
        let fallback = props.get("fallback").cloned();
        let class = merge_class_name("almadar-error-boundary", props);
        let owned_view = view.to_owned();
        let owned_registry = registry.clone();
        rsx! {
            dioxus::prelude::ErrorBoundary {
                handle_error: move |errors: ErrorContext| {
                    if let Some(fallback) = fallback.as_ref() {
                        let node = owned_view.as_view().render_node(&owned_registry, fallback);
                        return rsx! { div { class: "{class}", {node} } };
                    }
                    let message = errors.error().map(|error| error.to_string()).unwrap_or_default();
                    let title = crate::i18n::t("error.somethingWentWrong");
                    rsx! { div { class: "{class}", role: "alert",
                        div { class: "almadar-error-state__icon", "⚠" }
                        div { class: "almadar-error-state__title", "{title}" }
                        if !message.is_empty() { div { class: "almadar-error-state__message", "{message}" } }
                        button { class: "almadar-button", onclick: move |_| errors.clear_errors(), "Retry" }
                    } }
                },
                for (index, (child, source)) in children.iter().enumerate() {
                    BoundaryChild {
                        key: "{index}",
                        spec: child.clone(),
                        source: source.clone(),
                        view: view.to_owned(),
                        registry: registry.clone(),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::core::{ElementId, Mutation, Mutations, VirtualDom};
    use std::sync::atomic::{AtomicBool, Ordering};

    static FAIL_CHILD: AtomicBool = AtomicBool::new(false);
    static AUTHORED_FALLBACK: AtomicBool = AtomicBool::new(false);

    #[component]
    fn RiskyChild() -> Element {
        if FAIL_CHILD.load(Ordering::SeqCst) {
            dioxus::core::bail!("render failure");
        }
        let text = "healthy child";
        rsx! { span { "{text}" } }
    }

    struct RiskyRenderer;

    impl DioxusPatternRenderer for RiskyRenderer {
        fn pattern_name(&self) -> &str {
            "risky-child"
        }

        fn render(
            &self,
            _props: &Value,
            _entity: Option<&Value>,
            _view: &DioxusView<'_>,
            _registry: &DioxusPatternRegistry,
        ) -> Element {
            rsx! { RiskyChild {} }
        }
    }

    struct AuthoredFallbackRenderer;

    impl DioxusPatternRenderer for AuthoredFallbackRenderer {
        fn pattern_name(&self) -> &str {
            "authored-fallback"
        }

        fn render(
            &self,
            _props: &Value,
            _entity: Option<&Value>,
            _view: &DioxusView<'_>,
            _registry: &DioxusPatternRegistry,
        ) -> Element {
            let text = "authored recovery";
            rsx! { span { "{text}" } }
        }
    }

    fn boundary_app() -> Element {
        let spec: Value = if AUTHORED_FALLBACK.load(Ordering::SeqCst) {
            serde_json::from_str(
                r#"{"type":"error-boundary","fallback":{"type":"authored-fallback"},"children":[{"type":"risky-child"}]}"#,
            )
            .unwrap()
        } else {
            serde_json::from_str(r#"{"type":"error-boundary","children":[{"type":"risky-child"}]}"#)
                .unwrap()
        };
        let entities = std::collections::HashMap::new();
        let trait_content = orbital_core::ValueMap::new();
        let source_sink = Callback::new(|_: Option<String>| Callback::new(|_: (String, Value)| {}));
        let view = DioxusView {
            trait_name: "Test",
            orbital_name: "Test",
            current_state: "ready",
            entities: &entities,
            trait_content: &trait_content,
            source_trait: None,
            sink: None,
            source_sink,
            navigate: None,
            dock: None,
            viewport_width: None,
            canvas_theme: None,
            theme_mode: None,
            toggle_theme: None,
        };
        let mut registry = DioxusPatternRegistry::new();
        registry.register(Box::new(ErrorBoundaryRenderer));
        registry.register(Box::new(RiskyRenderer));
        registry.register(Box::new(AuthoredFallbackRenderer));
        registry.render_spec(&spec, None, &view)
    }

    fn text(mutations: &Mutations) -> Vec<&str> {
        mutations
            .edits
            .iter()
            .filter_map(|edit| match edit {
                Mutation::CreateTextNode { value, .. } | Mutation::SetText { value, .. } => {
                    Some(value.as_str())
                }
                _ => None,
            })
            .collect()
    }

    fn settle(dom: &mut VirtualDom) -> Mutations {
        let mut mutations = Mutations::default();
        dom.rebuild(&mut mutations);
        dom.render_immediate(&mut mutations);
        mutations
    }

    #[test]
    fn error_boundary_shows_children_until_failure_then_authored_fallback_and_can_retry_default() {
        let _converter_guard = crate::EVENT_CONVERTER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        FAIL_CHILD.store(false, Ordering::SeqCst);
        AUTHORED_FALLBACK.store(true, Ordering::SeqCst);
        let mut healthy = VirtualDom::new(boundary_app);
        let painted = settle(&mut healthy);
        assert!(text(&painted).contains(&"healthy child"));
        assert!(!text(&painted).contains(&"authored recovery"));
        assert!(!text(&painted).contains(&"Something went wrong"));

        FAIL_CHILD.store(true, Ordering::SeqCst);
        let mut failed = VirtualDom::new(boundary_app);
        let painted = settle(&mut failed);
        assert!(text(&painted).contains(&"authored recovery"));
        assert!(!text(&painted).contains(&"healthy child"));

        AUTHORED_FALLBACK.store(false, Ordering::SeqCst);
        let mut default = VirtualDom::new(boundary_app);
        let painted = settle(&mut default);
        assert!(text(&painted).contains(&"Something went wrong"));
        let retry: ElementId = painted
            .edits
            .iter()
            .find_map(|edit| match edit {
                Mutation::NewEventListener { name, id } if name == "click" => Some(*id),
                _ => None,
            })
            .expect("Retry click listener");
        FAIL_CHILD.store(false, Ordering::SeqCst);
        dioxus::prelude::set_event_converter(Box::new(dioxus_html::SerializedHtmlEventConverter));
        let mouse = std::rc::Rc::new(dioxus::prelude::PlatformEventData::new(Box::<
            dioxus_html::SerializedMouseData,
        >::default(
        ))) as std::rc::Rc<dyn std::any::Any>;
        default
            .runtime()
            .handle_event("click", dioxus::prelude::Event::new(mouse, true), retry);
        let mut retried = Mutations::default();
        default.render_immediate(&mut retried);
        assert!(text(&retried).contains(&"healthy child"));
        assert!(!text(&retried).contains(&"Something went wrong"));
    }
}
