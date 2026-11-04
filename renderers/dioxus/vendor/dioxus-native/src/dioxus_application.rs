use blitz_shell::{BlitzApplication, View};
use dioxus_core::{provide_context, ScopeId};
use dioxus_history::{History, MemoryHistory};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::RwLock;
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::WindowId;

use crate::DioxusNativeWindowRenderer;
use crate::{contexts::DioxusNativeDocument, BlitzShellEvent, DioxusDocument, WindowConfig};

#[derive(Clone)]
pub struct NativeViewport {
    width: Arc<AtomicU32>,
    height: Arc<AtomicU32>,
}

impl NativeViewport {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width: Arc::new(AtomicU32::new(width)),
            height: Arc::new(AtomicU32::new(height)),
        }
    }

    fn set(&self, width: u32, height: u32) {
        self.width.store(width, Ordering::Relaxed);
        self.height.store(height, Ordering::Relaxed);
    }

    pub fn width(&self) -> u32 {
        self.width.load(Ordering::Relaxed)
    }
    pub fn height(&self) -> u32 {
        self.height.load(Ordering::Relaxed)
    }
}

// ORBITAL PATCH (frame hook): one process-wide callback invoked on every
// `RedrawRequested`, BEFORE blitz processes the frame. orbital-client-dioxus
// installs it to drive scheduled game ticks (the kernel tick clock, through ClientCore) on
// the vsync cadence, so `set`-effect loops advance without waiting on a DOM
// event. Native-only (winit has no wasm32 RedrawRequested drive here).
static FRAME_HOOK: RwLock<Option<Box<dyn FnMut() + Send + Sync>>> = RwLock::new(None);
type LayoutHook = fn(&winit::window::Window, &[(usize, [f32; 4])]);
static LAYOUT_HOOK: RwLock<Option<LayoutHook>> = RwLock::new(None);
static VISIBILITY_HOOK: RwLock<Option<LayoutHook>> = RwLock::new(None);
static LAYOUT_WAKE: RwLock<Option<EventLoopProxy<BlitzShellEvent>>> = RwLock::new(None);

pub fn set_layout_hook(hook: Option<LayoutHook>) {
    *LAYOUT_HOOK.write().expect("layout hook poisoned") = hook;
}

pub fn set_visibility_hook(hook: Option<LayoutHook>) {
    *VISIBILITY_HOOK.write().expect("visibility hook poisoned") = hook;
}

pub fn wake_layout() {
    if let Some(proxy) = LAYOUT_WAKE.read().expect("layout wake poisoned").as_ref() {
        let _ = proxy.send_event(BlitzShellEvent::embedder_event(
            DioxusNativeEvent::WakeLayout,
        ));
    }
}

/// Install (or clear, with `None`) the per-frame hook, replacing any previous one.
pub fn set_frame_hook(hook: Option<Box<dyn FnMut() + Send + Sync>>) {
    *FRAME_HOOK.write().expect("frame hook poisoned") = hook;
}

/// Run the installed hook, if any. Takes the hook out so the lock is not held
/// across the callback (the hook may itself clear/replace it, e.g. on unmount),
/// then re-inserts it unless it was replaced meanwhile.
fn run_frame_hook() {
    let mut hook = FRAME_HOOK.write().expect("frame hook poisoned").take();
    if let Some(h) = hook.as_mut() {
        h();
    }
    let mut guard = FRAME_HOOK.write().expect("frame hook poisoned");
    if guard.is_none() {
        *guard = hook;
    }
}

/// Dioxus-native specific event type
pub enum DioxusNativeEvent {
    WakeLayout,
    /// A hotreload event, basically telling us to update our templates.
    #[cfg(all(
        feature = "hot-reload",
        debug_assertions,
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    DevserverEvent(dioxus_devtools::DevserverMsg),

    /// Create a new head element from the Link and Title elements
    ///
    /// todo(jon): these should probabkly be synchronous somehow
    CreateHeadElement {
        window: WindowId,
        name: String,
        attributes: Vec<(String, String)>,
        contents: Option<String>,
    },
}

pub struct DioxusNativeApplication {
    pending_window: Option<WindowConfig<DioxusNativeWindowRenderer>>,
    inner: BlitzApplication<DioxusNativeWindowRenderer>,
    proxy: EventLoopProxy<BlitzShellEvent>,
    viewports: HashMap<WindowId, NativeViewport>,
}

impl DioxusNativeApplication {
    pub fn new(
        proxy: EventLoopProxy<BlitzShellEvent>,
        config: WindowConfig<DioxusNativeWindowRenderer>,
    ) -> Self {
        *LAYOUT_WAKE.write().expect("layout wake poisoned") = Some(proxy.clone());
        Self {
            pending_window: Some(config),
            inner: BlitzApplication::new(proxy.clone()),
            proxy,
            viewports: HashMap::new(),
        }
    }

    pub fn add_window(&mut self, window_config: WindowConfig<DioxusNativeWindowRenderer>) {
        self.inner.add_window(window_config);
    }

    fn handle_blitz_shell_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: &DioxusNativeEvent,
    ) {
        match event {
            DioxusNativeEvent::WakeLayout => {
                for window in self.inner.windows.values_mut() {
                    window.request_redraw();
                }
            }
            #[cfg(all(
                feature = "hot-reload",
                debug_assertions,
                not(target_os = "android"),
                not(target_os = "ios")
            ))]
            DioxusNativeEvent::DevserverEvent(event) => match event {
                dioxus_devtools::DevserverMsg::HotReload(hotreload_message) => {
                    for window in self.inner.windows.values_mut() {
                        let doc = window.downcast_doc_mut::<DioxusDocument>();

                        // Apply changes to vdom
                        dioxus_devtools::apply_changes(&doc.vdom, hotreload_message);

                        // Reload changed assets
                        for asset_path in &hotreload_message.assets {
                            if let Some(url) = asset_path.to_str() {
                                doc.reload_resource_by_href(url);
                            }
                        }

                        window.poll();
                    }
                }
                dioxus_devtools::DevserverMsg::Shutdown => event_loop.exit(),
                dioxus_devtools::DevserverMsg::FullReloadStart => {}
                dioxus_devtools::DevserverMsg::FullReloadFailed => {}
                dioxus_devtools::DevserverMsg::FullReloadCommand => {}
                _ => {}
            },

            DioxusNativeEvent::CreateHeadElement {
                name,
                attributes,
                contents,
                window,
            } => {
                if let Some(window) = self.inner.windows.get_mut(window) {
                    let doc = window.downcast_doc_mut::<DioxusDocument>();
                    doc.create_head_element(name, attributes, contents);
                    window.poll();
                }
            }

            // Suppress unused variable warning
            #[cfg(not(all(
                feature = "hot-reload",
                debug_assertions,
                not(target_os = "android"),
                not(target_os = "ios")
            )))]
            #[allow(unreachable_patterns)]
            _ => {
                let _ = event_loop;
                let _ = event;
            }
        }
    }
}

impl ApplicationHandler<BlitzShellEvent> for DioxusNativeApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(feature = "tracing")]
        tracing::debug!("Injecting document provider into all windows");

        if let Some(config) = self.pending_window.take() {
            let mut window = View::init(config, event_loop, &self.proxy);
            let renderer = window.renderer.clone();
            let window_id = window.window_id();
            let size = window
                .window
                .inner_size()
                .to_logical::<u32>(window.window.scale_factor());
            let viewport = NativeViewport::new(size.width, size.height);
            let doc = window.downcast_doc_mut::<DioxusDocument>();

            doc.vdom.in_scope(ScopeId::ROOT, || {
                let shared: Rc<dyn dioxus_document::Document> =
                    Rc::new(DioxusNativeDocument::new(self.proxy.clone(), window_id));
                provide_context(shared);
                provide_context(viewport.clone());
            });
            self.viewports.insert(window_id, viewport);

            // Add history
            let history_provider: Rc<dyn History> = Rc::new(MemoryHistory::default());
            doc.vdom
                .in_scope(ScopeId::ROOT, move || provide_context(history_provider));

            // Add renderer
            let paint_sources = renderer.paint_source_registry();
            doc.vdom.in_scope(ScopeId::ROOT, move || {
                provide_context(paint_sources);
                provide_context(renderer);
            });

            // Queue rebuild
            doc.initial_build();

            // And then request redraw
            window.request_redraw();

            // todo(jon): we should actually mess with the pending windows instead of passing along the contexts
            self.inner.windows.insert(window_id, window);
        }

        self.inner.resumed(event_loop);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.suspended(event_loop);
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        self.inner.new_events(event_loop, cause);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::Resized(size) = &event {
            if let Some(viewport) = self.viewports.get(&window_id) {
                if let Some(window) = self.inner.windows.get_mut(&window_id) {
                    let logical = size.to_logical::<u32>(window.window.scale_factor());
                    viewport.set(logical.width, logical.height);
                    window
                        .downcast_doc_mut::<DioxusDocument>()
                        .vdom
                        .mark_dirty(ScopeId::APP);
                }
            }
        }
        // ORBITAL PATCH: drive the frame hook on the vsync cadence, before the
        // blitz frame runs, so tick effects land in this frame's render.
        let redraw = matches!(event, WindowEvent::RedrawRequested);
        if redraw {
            run_frame_hook();
        }
        self.inner.window_event(event_loop, window_id, event);
        if redraw {
            let video_hook = *LAYOUT_HOOK.read().expect("layout hook poisoned");
            let visibility_hook = *VISIBILITY_HOOK.read().expect("visibility hook poisoned");
            if video_hook.is_some() || visibility_hook.is_some() {
                if let Some(window) = self.inner.windows.get_mut(&window_id) {
                    let doc = window.downcast_doc_mut::<DioxusDocument>();
                    let scale = doc.inner.viewport().scale();
                    let mut overlays = Vec::new();
                    let mut observed = Vec::new();
                    for (_, node) in doc.inner.tree().iter() {
                        let Some(attrs) = node.attrs() else { continue };
                        let video_id = attrs
                            .iter()
                            .find(|attr| attr.name.local.as_ref() == "data-orbital-video")
                            .and_then(|attr| attr.value.parse::<usize>().ok());
                        let visibility_id = attrs
                            .iter()
                            .find(|attr| attr.name.local.as_ref() == "data-orbital-visibility")
                            .and_then(|attr| attr.value.parse::<usize>().ok());
                        if video_id.is_some() || visibility_id.is_some() {
                            let position = node.absolute_position(0.0, 0.0);
                            let bounds = [
                                position.x * scale,
                                position.y * scale,
                                node.final_layout.size.width * scale,
                                node.final_layout.size.height * scale,
                            ];
                            if let Some(id) = video_id {
                                overlays.push((id, bounds));
                            }
                            if let Some(id) = visibility_id {
                                observed.push((id, bounds));
                            }
                        }
                    }
                    if let Some(hook) = video_hook {
                        hook(&window.window, &overlays);
                    }
                    if let Some(hook) = visibility_hook {
                        hook(&window.window, &observed);
                    }
                }
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: BlitzShellEvent) {
        match event {
            BlitzShellEvent::Embedder(event) => {
                if let Some(event) = event.downcast_ref::<DioxusNativeEvent>() {
                    self.handle_blitz_shell_event(event_loop, event);
                }
            }
            event => self.inner.user_event(event_loop, event),
        }
    }
}
