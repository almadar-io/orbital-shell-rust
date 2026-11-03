#[cfg(target_os = "android")]
use crate::convert_events::byte_to_utf16_index;
use crate::convert_events::{
    color_scheme_to_theme, theme_to_color_scheme, winit_ime_to_blitz, winit_key_event_to_blitz,
    winit_modifiers_to_kbt_modifiers,
};
use crate::event::{create_waker, BlitzShellEvent};
use crate::BlitzShellProvider;
#[cfg(target_os = "android")]
use android_activity::input::{TextInputState, TextSpan};
use anyrender::WindowRenderer;
use blitz_dom::Document;
use blitz_paint::paint_scene;
use blitz_traits::events::{BlitzMouseButtonEvent, MouseEventButton, MouseEventButtons, UiEvent};
use blitz_traits::shell::Viewport;
use winit::keyboard::PhysicalKey;

#[cfg(feature = "accessibility")]
use std::cell::Cell;
use std::sync::Arc;
use std::task::Waker;
use std::time::Instant;
#[cfg(any(target_os = "android", target_os = "ios"))]
use winit::event::TouchPhase;
use winit::event::{ElementState, MouseButton};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::{Theme, WindowAttributes, WindowId};
use winit::{event::Modifiers, event::WindowEvent, keyboard::KeyCode, window::Window};

#[cfg(feature = "accessibility")]
use crate::accessibility::AccessibilityState;

pub struct WindowConfig<Rend: WindowRenderer> {
    doc: Box<dyn Document>,
    attributes: WindowAttributes,
    renderer: Rend,
}

impl<Rend: WindowRenderer> WindowConfig<Rend> {
    pub fn new(doc: Box<dyn Document>, renderer: Rend) -> Self {
        Self::with_attributes(doc, renderer, Window::default_attributes())
    }

    pub fn with_attributes(
        doc: Box<dyn Document>,
        renderer: Rend,
        attributes: WindowAttributes,
    ) -> Self {
        WindowConfig {
            doc,
            attributes,
            renderer,
        }
    }
}

pub struct View<Rend: WindowRenderer> {
    pub doc: Box<dyn Document>,

    pub renderer: Rend,
    pub waker: Option<Waker>,

    pub event_loop_proxy: EventLoopProxy<BlitzShellEvent>,
    pub window: Arc<Window>,

    /// The state of the keyboard modifiers (ctrl, shift, etc). Winit/Tao don't track these for us so we
    /// need to store them in order to have access to them when processing keypress events
    pub theme_override: Option<Theme>,
    pub keyboard_modifiers: Modifiers,
    pub buttons: MouseEventButtons,
    pub mouse_pos: (f32, f32),
    #[cfg(target_os = "android")]
    pub ime_allowed: bool,
    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub active_touch_id: Option<u64>,
    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub touch_start: Option<(f32, f32)>,
    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub touch_last: Option<(f32, f32)>,
    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub touch_scrolling: bool,
    pub animation_timer: Option<Instant>,
    pub is_visible: bool,

    #[cfg(feature = "accessibility")]
    /// Accessibility adapter for `accesskit`.
    pub accessibility: AccessibilityState,
    #[cfg(feature = "accessibility")]
    accessibility_dirty: Cell<bool>,
}

impl<Rend: WindowRenderer> View<Rend> {
    pub fn init(
        config: WindowConfig<Rend>,
        event_loop: &ActiveEventLoop,
        proxy: &EventLoopProxy<BlitzShellEvent>,
    ) -> Self {
        let winit_window = Arc::from(event_loop.create_window(config.attributes).unwrap());

        #[cfg(not(target_os = "android"))]
        winit_window.set_ime_allowed(true);

        // Create viewport
        let size = winit_window.inner_size();
        let scale = winit_window.scale_factor() as f32;
        let theme = winit_window.theme().unwrap_or(Theme::Light);
        let color_scheme = theme_to_color_scheme(theme);
        let viewport = Viewport::new(size.width, size.height, scale, color_scheme);

        // Create shell provider
        let shell_provider = BlitzShellProvider::new(winit_window.clone());

        let mut doc = config.doc;
        doc.set_viewport(viewport);
        doc.set_shell_provider(Arc::new(shell_provider));

        // If the document title is set prior to the window being created then it will
        // have been sent to a dummy ShellProvider and won't get picked up.
        // So we look for it here and set it if present.
        let title = doc.find_title_node().map(|node| node.text_content());
        if let Some(title) = title {
            winit_window.set_title(&title);
        }

        Self {
            renderer: config.renderer,
            waker: None,
            animation_timer: None,
            keyboard_modifiers: Default::default(),
            event_loop_proxy: proxy.clone(),
            window: winit_window.clone(),
            doc,
            theme_override: None,
            buttons: MouseEventButtons::None,
            mouse_pos: Default::default(),
            #[cfg(target_os = "android")]
            ime_allowed: false,
            #[cfg(any(target_os = "android", target_os = "ios"))]
            active_touch_id: None,
            #[cfg(any(target_os = "android", target_os = "ios"))]
            touch_start: None,
            #[cfg(any(target_os = "android", target_os = "ios"))]
            touch_last: None,
            #[cfg(any(target_os = "android", target_os = "ios"))]
            touch_scrolling: false,
            is_visible: winit_window.is_visible().unwrap_or(true),
            #[cfg(feature = "accessibility")]
            accessibility: AccessibilityState::new(event_loop, &winit_window, proxy.clone()),
            #[cfg(feature = "accessibility")]
            accessibility_dirty: Cell::new(true),
        }
    }

    pub fn replace_document(&mut self, new_doc: Box<dyn Document>, retain_scroll_position: bool) {
        let scroll = self.doc.viewport_scroll();
        let viewport = self.doc.viewport().clone();
        let shell_provider = self.doc.shell_provider.clone();

        self.doc = new_doc;
        self.doc.set_viewport(viewport);
        self.doc.set_shell_provider(shell_provider);
        self.poll();
        self.request_redraw();

        if retain_scroll_position {
            self.doc.set_viewport_scroll(scroll);
        }
    }

    pub fn theme_override(&self) -> Option<Theme> {
        self.theme_override
    }

    pub fn current_theme(&self) -> Theme {
        color_scheme_to_theme(self.doc.viewport().color_scheme)
    }

    pub fn set_theme_override(&mut self, theme: Option<Theme>) {
        self.theme_override = theme;
        let theme = theme.or(self.window.theme()).unwrap_or(Theme::Light);
        self.with_viewport(|v| v.color_scheme = theme_to_color_scheme(theme));
    }

    pub fn downcast_doc_mut<T: 'static>(&mut self) -> &mut T {
        self.doc.as_any_mut().downcast_mut::<T>().unwrap()
    }

    pub fn current_animation_time(&mut self) -> f64 {
        match &self.animation_timer {
            Some(start) => Instant::now().duration_since(*start).as_secs_f64(),
            None => {
                self.animation_timer = Some(Instant::now());
                0.0
            }
        }
    }
}

impl<Rend: WindowRenderer> View<Rend> {
    pub fn resume(&mut self) {
        // Resolve dom
        let animation_time = self.current_animation_time();
        self.doc.resolve(animation_time);
        #[cfg(feature = "accessibility")]
        self.accessibility_dirty.set(true);

        // Resume renderer
        let (width, height) = self.doc.viewport().window_size;
        let scale = self.doc.viewport().scale_f64();
        self.renderer.resume(self.window.clone(), width, height);
        if !self.renderer.is_active() {
            panic!("Renderer failed to resume");
        };

        // Render
        self.renderer
            .render(|scene| paint_scene(scene, &self.doc, scale, width, height));
        #[cfg(feature = "accessibility")]
        if self.accessibility_dirty.replace(false) {
            self.accessibility.update_tree(&self.doc);
        }

        // Set waker
        self.waker = Some(create_waker(&self.event_loop_proxy, self.window_id()));
    }

    pub fn suspend(&mut self) {
        self.waker = None;
        self.renderer.suspend();
    }

    pub fn poll(&mut self) -> bool {
        if let Some(waker) = &self.waker {
            let cx = std::task::Context::from_waker(waker);
            if self.doc.poll(Some(cx)) {
                self.request_redraw();
                return true;
            }
        }

        false
    }

    pub fn request_redraw(&self) {
        #[cfg(feature = "accessibility")]
        self.accessibility_dirty.set(true);
        if self.renderer.is_active() {
            self.window.request_redraw();
        }
    }

    pub fn redraw(&mut self) {
        let animation_time = self.current_animation_time();
        self.doc.resolve(animation_time);
        #[cfg(feature = "accessibility")]
        if self.accessibility_dirty.replace(false) {
            self.accessibility.update_tree(&self.doc);
        }
        #[cfg(target_os = "android")]
        crate::android_text_bounds::update(&self.doc);
        let (width, height) = self.doc.viewport().window_size;
        let scale = self.doc.viewport().scale_f64();
        self.renderer
            .render(|scene| paint_scene(scene, &self.doc, scale, width, height));

        if self.is_visible && self.doc.is_animating() {
            self.window.request_redraw();
        }
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    #[inline]
    pub fn with_viewport(&mut self, cb: impl FnOnce(&mut Viewport)) {
        let mut viewport = self.doc.viewport_mut();
        cb(&mut viewport);
        drop(viewport);
        let (width, height) = self.doc.viewport().window_size;
        if width > 0 && height > 0 {
            self.renderer.set_size(width, height);
            self.request_redraw();
        }
    }

    #[cfg(feature = "accessibility")]
    pub fn build_accessibility_tree(&mut self) {
        self.accessibility.update_tree(&self.doc);
    }

    #[cfg(feature = "accessibility")]
    pub fn handle_accessibility_action(&mut self, request: &accesskit::ActionRequest) {
        if !matches!(
            request.action,
            accesskit::Action::Click | accesskit::Action::Focus
        ) {
            return;
        }
        if self.doc.get_node(request.target_node.0 as usize).is_none() {
            return;
        }
        if request.action == accesskit::Action::Focus {
            self.doc.set_focus_to(request.target_node.0 as usize);
            #[cfg(target_os = "android")]
            self.sync_ime_to_focus();
            self.request_redraw();
            return;
        }
        let Some(node) = self.doc.get_node(request.target_node.0 as usize) else {
            return;
        };
        let position = node.absolute_position(0.0, 0.0);
        let size = node.final_layout.size;
        let scroll = self.doc.viewport_scroll();
        let x = position.x + size.width / 2.0 - scroll.x as f32;
        let y = position.y + size.height / 2.0 - scroll.y as f32;
        self.move_pointer(x, y);
        self.button_pointer(MouseEventButton::Main, ElementState::Pressed);
        self.button_pointer(MouseEventButton::Main, ElementState::Released);
    }

    fn move_pointer(&mut self, x: f32, y: f32) {
        self.mouse_pos = (x, y);
        self.doc
            .handle_ui_event(UiEvent::MouseMove(BlitzMouseButtonEvent {
                x,
                y,
                button: Default::default(),
                buttons: self.buttons,
                mods: winit_modifiers_to_kbt_modifiers(self.keyboard_modifiers.state()),
            }));
        self.request_redraw();
    }

    fn leave_pointer(&mut self) {
        self.doc
            .handle_ui_event(UiEvent::MouseLeave(BlitzMouseButtonEvent {
                x: self.mouse_pos.0,
                y: self.mouse_pos.1,
                button: MouseEventButton::Main,
                buttons: self.buttons,
                mods: winit_modifiers_to_kbt_modifiers(self.keyboard_modifiers.state()),
            }));
        self.buttons = MouseEventButtons::None;
        self.request_redraw();
    }

    #[cfg(target_os = "android")]
    fn sync_ime_to_focus(&mut self) {
        let input_data = self
            .doc
            .get_focussed_node_id()
            .and_then(|id| self.doc.get_node(id))
            .and_then(|node| node.element_data())
            .and_then(|element| element.text_input_data());
        let allowed = input_data.is_some();
        if let Some(input_data) = input_data {
            let editor = &input_data.editor;
            let text = editor.raw_text().to_string();
            let selection = editor.raw_selection();
            let anchor = byte_to_utf16_index(&text, selection.anchor().index());
            let focus = byte_to_utf16_index(&text, selection.focus().index());
            let selection = TextSpan {
                start: anchor.min(focus),
                end: anchor.max(focus),
            };
            let compose_region = editor.raw_compose().as_ref().map(|range| TextSpan {
                start: byte_to_utf16_index(&text, range.start),
                end: byte_to_utf16_index(&text, range.end),
            });
            crate::current_android_app().set_text_input_state(TextInputState {
                text,
                selection,
                compose_region,
            });
        }
        if allowed != self.ime_allowed {
            self.window.set_ime_allowed(allowed);
            self.ime_allowed = allowed;
        }
    }

    fn button_pointer(&mut self, button: MouseEventButton, state: ElementState) {
        match state {
            ElementState::Pressed => self.buttons |= button.into(),
            ElementState::Released => self.buttons.remove(button.into()),
        }
        let event = BlitzMouseButtonEvent {
            x: self.mouse_pos.0,
            y: self.mouse_pos.1,
            button,
            buttons: self.buttons,
            mods: winit_modifiers_to_kbt_modifiers(self.keyboard_modifiers.state()),
        };
        self.doc.handle_ui_event(match state {
            ElementState::Pressed => UiEvent::MouseDown(event),
            ElementState::Released => UiEvent::MouseUp(event),
        });
        #[cfg(target_os = "android")]
        self.sync_ime_to_focus();
        self.request_redraw();
    }

    pub fn handle_winit_event(&mut self, event: WindowEvent) {
        match event {
            // Window lifecycle events
            WindowEvent::Destroyed => {}
            WindowEvent::ActivationTokenDone { .. } => {},
            WindowEvent::CloseRequested => {
                // Currently handled at the level above in application.rs
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
            }

            // Window size/position events
            WindowEvent::Moved(_) => {}
            WindowEvent::Occluded(is_occluded) => {
                self.is_visible = !is_occluded;
                if self.is_visible {
                    self.request_redraw();
                }
            },
            WindowEvent::Resized(physical_size) => {
                self.with_viewport(|v| v.window_size = (physical_size.width, physical_size.height));
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.with_viewport(|v| v.set_hidpi_scale(scale_factor as f32));
            }

            // Theme events
            WindowEvent::ThemeChanged(theme) => {
                let color_scheme = theme_to_color_scheme(self.theme_override.unwrap_or(theme));
                self.doc.viewport_mut().color_scheme = color_scheme;
            }

            // Text / keyboard events
            WindowEvent::Ime(ime_event) => {
                #[cfg(target_os = "android")]
                let full_state_from_ime = matches!(ime_event, winit::event::Ime::TextState { .. });
                self.doc.handle_ui_event(UiEvent::Ime(winit_ime_to_blitz(ime_event)));
                #[cfg(target_os = "android")]
                if !full_state_from_ime {
                    self.sync_ime_to_focus();
                }
                self.request_redraw();
            },
            WindowEvent::ModifiersChanged(new_state) => {
                // Store new keyboard modifier (ctrl, shift, etc) state for later use
                self.keyboard_modifiers = new_state;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(key_code) = event.physical_key else {
                    return;
                };

                if event.state.is_pressed() {
                    let ctrl = self.keyboard_modifiers.state().control_key();
                    let meta = self.keyboard_modifiers.state().super_key();
                    let alt = self.keyboard_modifiers.state().alt_key();

                    // Ctrl/Super keyboard shortcuts
                    if ctrl | meta {
                        match key_code {
                            KeyCode::Equal => self.doc.viewport_mut().zoom_by(0.1),
                            KeyCode::Minus => self.doc.viewport_mut().zoom_by(-0.1),
                            KeyCode::Digit0 => self.doc.viewport_mut().set_zoom(1.0),
                            _ => {}
                        };
                    }

                    // Alt keyboard shortcuts
                    if alt {
                        match key_code {
                            KeyCode::KeyD => {
                                self.doc.devtools_mut().toggle_show_layout();
                                self.request_redraw();
                            }
                            KeyCode::KeyH => {
                                self.doc.devtools_mut().toggle_highlight_hover();
                                self.request_redraw();
                            }
                            KeyCode::KeyT => self.doc.print_taffy_tree(),
                            _ => {}
                        };
                    }

                }

                // Unmodified keypresses
                let key_event_data = winit_key_event_to_blitz(&event, self.keyboard_modifiers.state());
                let event = if event.state.is_pressed() {
                    UiEvent::KeyDown(key_event_data)
                } else {
                    UiEvent::KeyUp(key_event_data)
                };

                self.doc.handle_ui_event(event);
                #[cfg(target_os = "android")]
                self.sync_ime_to_focus();
                self.request_redraw();
            }


            // Mouse/pointer events
            WindowEvent::CursorEntered { /*device_id*/.. } => {}
            WindowEvent::CursorLeft { /*device_id*/.. } => self.leave_pointer(),
            WindowEvent::CursorMoved { position, .. } => {
                let winit::dpi::LogicalPosition::<f32> { x, y } = position.to_logical(self.window.scale_factor());
                self.move_pointer(x, y);
            }
            WindowEvent::MouseInput { button, state, .. } => {
                let button = match button {
                    MouseButton::Left => MouseEventButton::Main,
                    MouseButton::Right => MouseEventButton::Secondary,
                    _ => return,
                };

                self.button_pointer(button, state);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (scroll_x, scroll_y)= match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (x as f64 * 20.0, y as f64 * 20.0),
                    winit::event::MouseScrollDelta::PixelDelta(offsets) => (offsets.x, offsets.y)
                };

                let has_changed = if let Some(hover_node_id) = self.doc.get_hover_node_id() {
                    self.doc.scroll_node_by_has_changed(hover_node_id, scroll_x, scroll_y)
                } else {
                    self.doc.scroll_viewport_by_has_changed(scroll_x, scroll_y)
                };

                if has_changed {
                    self.request_redraw();
                }
            }

            // File events
            WindowEvent::DroppedFile(_) => {}
            WindowEvent::HoveredFile(_) => {}
            WindowEvent::HoveredFileCancelled => {}
            WindowEvent::Focused(true) => self.request_redraw(),
            WindowEvent::Focused(false) => self.leave_pointer(),

            // Touch and motion events
            #[cfg(any(target_os = "android", target_os = "ios"))]
            WindowEvent::Touch(touch) => {
                match touch.phase {
                    TouchPhase::Started if self.active_touch_id.is_none() => {
                        self.active_touch_id = Some(touch.id);
                    }
                    TouchPhase::Started => return,
                    _ if self.active_touch_id != Some(touch.id) => return,
                    _ => {}
                }
                let winit::dpi::LogicalPosition::<f32> { x, y } = touch.location.to_logical(self.window.scale_factor());
                self.move_pointer(x, y);
                match touch.phase {
                    TouchPhase::Started => {
                        self.touch_start = Some((x, y));
                        self.touch_last = Some((x, y));
                        self.touch_scrolling = false;
                        self.button_pointer(MouseEventButton::Main, ElementState::Pressed);
                    }
                    TouchPhase::Moved => {
                        if self.doc.is_dragging() {
                            self.touch_last = Some((x, y));
                            return;
                        }
                        if let (Some((start_x, start_y)), Some((last_x, last_y))) = (self.touch_start, self.touch_last) {
                            if !self.touch_scrolling && (x - start_x).hypot(y - start_y) > 8.0 {
                                self.touch_scrolling = true;
                                self.leave_pointer();
                            }
                            if self.touch_scrolling {
                                let changed = if let Some(node_id) = self.doc.get_hover_node_id() {
                                    self.doc.scroll_node_by_has_changed(node_id, (x - last_x) as f64, (y - last_y) as f64)
                                } else {
                                    self.doc.scroll_viewport_by_has_changed((x - last_x) as f64, (y - last_y) as f64)
                                };
                                if changed {
                                    self.request_redraw();
                                }
                            }
                        }
                        self.touch_last = Some((x, y));
                    }
                    TouchPhase::Ended => {
                        if !self.touch_scrolling || self.doc.is_dragging() {
                            self.button_pointer(MouseEventButton::Main, ElementState::Released);
                        }
                        self.active_touch_id = None;
                        self.touch_start = None;
                        self.touch_last = None;
                        self.touch_scrolling = false;
                    }
                    TouchPhase::Cancelled => {
                        self.leave_pointer();
                        self.active_touch_id = None;
                        self.touch_start = None;
                        self.touch_last = None;
                        self.touch_scrolling = false;
                        self.request_redraw();
                    }
                }
            }
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            WindowEvent::Touch(_) => {}
            WindowEvent::TouchpadPressure { .. } => {}
            WindowEvent::AxisMotion { .. } => {}
            WindowEvent::PinchGesture { .. } => {},
            WindowEvent::PanGesture { .. } => {},
            WindowEvent::DoubleTapGesture { .. } => {},
            WindowEvent::RotationGesture { .. } => {},
        }
    }
}
