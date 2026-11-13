use crate::{BaseDocument, DocumentMutator};
use blitz_traits::events::{
    BlitzDragEvent, BlitzDragTransfer, BlitzMouseButtonEvent, BlitzTouchEvent, BlitzTouchPhase,
    BlitzTouchPoint, BlitzWheelEvent, DomEvent,
    DomEventData, EventState, MouseEventButton, MouseEventButtons, UiEvent,
};
use keyboard_types::Key;
use markup5ever::LocalName;
use std::collections::VecDeque;

pub trait EventHandler {
    fn handle_event(
        &mut self,
        chain: &[usize],
        event: &mut DomEvent,
        mutr: &mut DocumentMutator<'_>,
        event_state: &mut EventState,
    );
}

pub struct NoopEventHandler;
impl EventHandler for NoopEventHandler {
    fn handle_event(
        &mut self,
        _chain: &[usize],
        _event: &mut DomEvent,
        _mutr: &mut DocumentMutator<'_>,
        _event_state: &mut EventState,
    ) {
        // Do nothing
    }
}

pub struct EventDriver<'doc, Handler: EventHandler> {
    mutr: DocumentMutator<'doc>,
    handler: Handler,
}

impl<'doc, Handler: EventHandler> EventDriver<'doc, Handler> {
    fn drag_source(&self, target: usize) -> Option<usize> {
        self.doc().node_chain(target).into_iter().find(|id| {
            self.doc()
                .get_node(*id)
                .and_then(|node| node.attr(LocalName::from("draggable")))
                .is_some_and(|value| value != "false")
        })
    }

    fn dispatch_drag(&mut self, target: usize, data: DomEventData) -> bool {
        if self.doc().get_node(target).is_none() {
            return false;
        }
        let mut event = DomEvent::new(target, data);
        let chain = if event.bubbles {
            self.doc().node_chain(target)
        } else {
            vec![target]
        };
        let mut state = EventState::default();
        self.handler
            .handle_event(&chain, &mut event, &mut self.mutr, &mut state);
        state.is_cancelled()
    }

    fn cancel_drag(&mut self, mouse: BlitzMouseButtonEvent) {
        self.doc_mut().drag_candidate = None;
        if let Some((source, over, transfer, _)) = self.doc_mut().drag_active.take() {
            transfer.set_drop_effect("none");
            if let Some(over) = over {
                self.dispatch_drag(
                    over,
                    DomEventData::DragLeave(BlitzDragEvent {
                        mouse: mouse.clone(),
                        transfer: transfer.clone(),
                    }),
                );
            }
            self.dispatch_drag(
                source,
                DomEventData::DragEnd(BlitzDragEvent { mouse, transfer }),
            );
        }
        self.doc_mut().set_mousedown_node_id(None);
    }

    fn move_drag(&mut self, target: Option<usize>, mouse: BlitzMouseButtonEvent) {
        if !mouse.buttons.contains(MouseEventButtons::Primary) {
            self.cancel_drag(mouse);
            return;
        }
        if self.doc().drag_active.is_none() {
            let Some((source, x, y)) = self.doc().drag_candidate else {
                return;
            };
            if (mouse.x - x).hypot(mouse.y - y) < 8.0 {
                return;
            }
            let transfer = BlitzDragTransfer::default();
            let started = !self.dispatch_drag(
                source,
                DomEventData::DragStart(BlitzDragEvent {
                    mouse: mouse.clone(),
                    transfer: transfer.clone(),
                }),
            );
            self.doc_mut().drag_candidate = None;
            if !started {
                return;
            }
            self.doc_mut().drag_active = Some((source, None, transfer, false));
        }
        let Some((source, old_target, transfer, _)) = self.doc().drag_active.clone() else {
            return;
        };
        if self.doc().get_node(source).is_none() {
            self.cancel_drag(mouse);
            return;
        }
        let target = target.filter(|id| self.doc().get_node(*id).is_some());
        if old_target != target {
            if let Some(old) = old_target {
                self.dispatch_drag(
                    old,
                    DomEventData::DragLeave(BlitzDragEvent {
                        mouse: mouse.clone(),
                        transfer: transfer.clone(),
                    }),
                );
            }
            if let Some(new) = target {
                self.dispatch_drag(
                    new,
                    DomEventData::DragEnter(BlitzDragEvent {
                        mouse: mouse.clone(),
                        transfer: transfer.clone(),
                    }),
                );
            }
        }
        let accepted = target.is_some_and(|id| {
            self.dispatch_drag(
                id,
                DomEventData::DragOver(BlitzDragEvent {
                    mouse,
                    transfer: transfer.clone(),
                }),
            )
        });
        if let Some(active) = &mut self.doc_mut().drag_active {
            active.1 = target;
            active.3 = accepted;
        }
    }

    fn finish_drag(&mut self, mouse: BlitzMouseButtonEvent) -> bool {
        self.doc_mut().drag_candidate = None;
        let Some((source, target, transfer, accepted)) = self.doc_mut().drag_active.take() else {
            return false;
        };
        let allowed = transfer.effect_allowed();
        let effect = transfer.drop_effect();
        let permitted = match effect.as_str() {
            "copy" => allowed == "copy" || allowed == "copyMove" || allowed == "all",
            "move" => allowed == "move" || allowed == "copyMove" || allowed == "all",
            "link" => allowed == "link" || allowed == "all",
            _ => false,
        };
        if accepted && permitted && target.is_some_and(|id| self.doc().get_node(id).is_some()) {
            if let Some(target) = target {
                self.dispatch_drag(
                    target,
                    DomEventData::Drop(BlitzDragEvent {
                        mouse: mouse.clone(),
                        transfer: transfer.clone(),
                    }),
                );
            }
        } else {
            transfer.set_drop_effect("none");
        }
        self.dispatch_drag(
            source,
            DomEventData::DragEnd(BlitzDragEvent { mouse, transfer }),
        );
        self.doc_mut().set_mousedown_node_id(None);
        true
    }
    fn dispatch_mouse_leave(
        &mut self,
        old: Option<usize>,
        new: Option<usize>,
        data: BlitzMouseButtonEvent,
    ) {
        let Some(old) = old else { return };
        let old_chain = self.doc().node_chain(old);
        let new_chain = new.map(|id| self.doc().node_chain(id)).unwrap_or_default();
        for node in old_chain {
            if !new_chain.contains(&node) {
                self.handle_dom_event(DomEvent::new(node, DomEventData::MouseLeave(data.clone())));
            }
        }
    }

    fn doc_mut(&mut self) -> &mut BaseDocument {
        self.mutr.doc
    }

    fn doc(&self) -> &BaseDocument {
        &*self.mutr.doc
    }

    pub fn new(mutr: DocumentMutator<'doc>, handler: Handler) -> Self {
        EventDriver { mutr, handler }
    }

    pub fn handle_ui_event(&mut self, event: UiEvent) {
        let viewport_scroll = self.doc().viewport_scroll();
        let zoom = self.doc().viewport.zoom();

        let mut hover_node_id = self.doc().hover_node_id;
        let previous_hover = hover_node_id;
        let pressed_target = self.doc().mousedown_node_id;
        let focussed_node_id = self.doc().focus_node_id;

        if let UiEvent::MouseLeave(data) = &event {
            self.cancel_drag(data.clone());
            self.dispatch_mouse_leave(
                pressed_target.or(previous_hover),
                None,
                BlitzMouseButtonEvent {
                    x: data.x + viewport_scroll.x as f32 / zoom,
                    y: data.y + viewport_scroll.y as f32 / zoom,
                    ..data.clone()
                },
            );
            self.doc_mut().unactive_node();
            self.doc_mut().set_mousedown_node_id(None);
            self.doc_mut().clear_hover();
            return;
        }

        if let UiEvent::KeyDown(data) = &event {
            if data.key == Key::Escape && self.doc().drag_active.is_some() {
                self.cancel_drag(BlitzMouseButtonEvent {
                    x: 0.0,
                    y: 0.0,
                    button: MouseEventButton::Main,
                    buttons: Default::default(),
                    mods: data.modifiers,
                });
            }
        }

        if let UiEvent::MouseUp(data) = &event {
            if self.finish_drag(data.clone()) {
                self.doc_mut().unactive_node();
                return;
            }
            self.doc_mut().drag_candidate = None;
        }

        // Update document input state (hover, focus, active, etc)
        match &event {
            UiEvent::MouseMove(event) => {
                let dom_x = event.x + viewport_scroll.x as f32 / zoom;
                let dom_y = event.y + viewport_scroll.y as f32 / zoom;
                self.doc_mut().set_hover_to(dom_x, dom_y);
                hover_node_id = self.doc().hover_node_id;
                self.move_drag(hover_node_id, event.clone());
                if previous_hover != hover_node_id {
                    self.dispatch_mouse_leave(
                        previous_hover,
                        hover_node_id,
                        BlitzMouseButtonEvent {
                            x: dom_x,
                            y: dom_y,
                            ..event.clone()
                        },
                    );
                }
            }
            UiEvent::MouseDown(data) => {
                self.doc_mut().active_node();
                self.doc_mut().set_mousedown_node_id(hover_node_id);
                if data.button == MouseEventButton::Main {
                    let source = hover_node_id.and_then(|id| self.drag_source(id));
                    self.doc_mut().drag_candidate = source.map(|id| (id, data.x, data.y));
                }
            }
            UiEvent::MouseUp(_) => {
                self.doc_mut().unactive_node();
            }
            UiEvent::MouseLeave(_) => unreachable!(),
            _ => {}
        };

        let target = match event {
            UiEvent::MouseMove(_) => hover_node_id,
            UiEvent::MouseUp(_) => hover_node_id,
            UiEvent::MouseDown(_) => hover_node_id,
            UiEvent::MouseLeave(_) => unreachable!(),
            UiEvent::KeyUp(_) => focussed_node_id,
            UiEvent::KeyDown(_) => focussed_node_id,
            UiEvent::Ime(_) => focussed_node_id,
            // ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
            UiEvent::Wheel(_) => hover_node_id,
            // ORBITAL PATCH: multi-touch
            UiEvent::Touch(ref touch) => {
                let id = touch.changed.id;
                let scroll_x = viewport_scroll.x as f32 / zoom;
                let scroll_y = viewport_scroll.y as f32 / zoom;
                let started = (touch.phase == BlitzTouchPhase::Start)
                    .then(|| self.doc().hit(touch.changed.x + scroll_x, touch.changed.y + scroll_y))
                    .flatten()
                    .map(|hit| hit.node_id);
                if let Some(node_id) = started {
                    self.doc_mut().touch_targets.insert(id, node_id);
                }
                match touch.phase {
                    BlitzTouchPhase::End | BlitzTouchPhase::Cancel => {
                        self.doc_mut().touch_targets.remove(&id)
                    }
                    _ => self.doc().touch_targets.get(&id).copied(),
                }
            }
        };

        let data = match event {
            UiEvent::MouseMove(data) => DomEventData::MouseMove(BlitzMouseButtonEvent {
                x: data.x + viewport_scroll.x as f32 / zoom,
                y: data.y + viewport_scroll.y as f32 / zoom,
                ..data
            }),
            UiEvent::MouseUp(data) => DomEventData::MouseUp(BlitzMouseButtonEvent {
                x: data.x + viewport_scroll.x as f32 / zoom,
                y: data.y + viewport_scroll.y as f32 / zoom,
                ..data
            }),
            UiEvent::MouseDown(data) => DomEventData::MouseDown(BlitzMouseButtonEvent {
                x: data.x + viewport_scroll.x as f32 / zoom,
                y: data.y + viewport_scroll.y as f32 / zoom,
                ..data
            }),
            UiEvent::MouseLeave(_) => unreachable!(),
            UiEvent::KeyUp(data) => DomEventData::KeyUp(data),
            UiEvent::KeyDown(data) => DomEventData::KeyDown(data),
            UiEvent::Ime(data) => DomEventData::Ime(data),
            // ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
            UiEvent::Wheel(data) => DomEventData::Wheel(BlitzWheelEvent {
                x: data.x + viewport_scroll.x as f32 / zoom,
                y: data.y + viewport_scroll.y as f32 / zoom,
                ..data
            }),
            // ORBITAL PATCH: multi-touch
            UiEvent::Touch(touch) => {
                let scrolled = |point: BlitzTouchPoint| BlitzTouchPoint {
                    x: point.x + viewport_scroll.x as f32 / zoom,
                    y: point.y + viewport_scroll.y as f32 / zoom,
                    ..point
                };
                let phase = touch.phase;
                let moved = BlitzTouchEvent {
                    changed: scrolled(touch.changed),
                    touches: touch.touches.iter().copied().map(scrolled).collect(),
                    ..touch
                };
                match phase {
                    BlitzTouchPhase::Start => DomEventData::TouchStart(moved),
                    BlitzTouchPhase::Move => DomEventData::TouchMove(moved),
                    BlitzTouchPhase::End => DomEventData::TouchEnd(moved),
                    BlitzTouchPhase::Cancel => DomEventData::TouchCancel(moved),
                }
            }
        };

        let target = target.unwrap_or_else(|| self.doc().root_element().id);
        let dom_event = DomEvent::new(target, data);

        self.handle_dom_event(dom_event);
    }

    pub fn handle_dom_event(&mut self, event: DomEvent) {
        let mut queue = VecDeque::with_capacity(4);
        queue.push_back(event);

        while let Some(mut event) = queue.pop_front() {
            let chain = if event.bubbles {
                self.doc().node_chain(event.target)
            } else {
                vec![event.target]
            };

            let mut event_state = EventState::default();
            self.handler
                .handle_event(&chain, &mut event, &mut self.mutr, &mut event_state);

            if !event_state.is_cancelled() {
                self.doc_mut()
                    .handle_dom_event(&mut event, |new_evt| queue.push_back(new_evt));
            }
        }
    }
}

#[cfg(test)]
mod pointer_leave_tests {
    use super::*;
    use crate::{ns, DocumentConfig, QualName};
    use blitz_traits::events::{MouseEventButton, MouseEventButtons};
    use markup5ever::local_name;
    use std::{cell::RefCell, rc::Rc};

    struct Recorder(Rc<RefCell<Vec<usize>>>);

    impl EventHandler for Recorder {
        fn handle_event(
            &mut self,
            _chain: &[usize],
            event: &mut DomEvent,
            _mutr: &mut DocumentMutator<'_>,
            _event_state: &mut EventState,
        ) {
            if matches!(event.data, DomEventData::MouseLeave(_)) {
                self.0.borrow_mut().push(event.target);
            }
        }
    }

    #[test]
    fn cancel_leaves_pressed_child_and_button_without_clicking() {
        let mut doc = BaseDocument::new(DocumentConfig::default());
        let mut mutator = doc.mutate();
        let button = mutator.create_element(
            QualName::new(None, ns!(html), local_name!("button")),
            vec![],
        );
        let child =
            mutator.create_element(QualName::new(None, ns!(html), local_name!("span")), vec![]);
        mutator.append_children(0, &[button]);
        mutator.append_children(button, &[child]);
        drop(mutator);
        doc.hover_node_id = Some(child);
        doc.mousedown_node_id = Some(child);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut driver = EventDriver::new(doc.mutate(), Recorder(seen.clone()));
        driver.handle_ui_event(UiEvent::MouseLeave(BlitzMouseButtonEvent {
            x: 0.0,
            y: 0.0,
            button: MouseEventButton::Main,
            buttons: MouseEventButtons::Primary,
            mods: Default::default(),
        }));
        assert!(seen.borrow().contains(&button));
        assert_eq!(driver.doc().hover_node_id, None);
        assert_eq!(driver.doc().mousedown_node_id, None);
    }
}

#[cfg(test)]
mod native_drag_tests {
    use super::*;
    use crate::{ns, Attribute, DocumentConfig, QualName};
    use blitz_traits::events::MouseEventButtons;
    use markup5ever::local_name;
    use std::{cell::RefCell, rc::Rc};

    const MIME: &str = "application/x-almadar-slot-item";
    const PAYLOAD: &str = r#"{"id":"step-7","name":"Jump"}"#;

    struct Recorder {
        seen: Rc<RefCell<Vec<&'static str>>>,
        cancel_start: bool,
    }

    impl EventHandler for Recorder {
        fn handle_event(
            &mut self,
            _chain: &[usize],
            event: &mut DomEvent,
            _mutr: &mut DocumentMutator<'_>,
            state: &mut EventState,
        ) {
            self.seen.borrow_mut().push(event.name());
            match &event.data {
                DomEventData::DragStart(data) => {
                    data.transfer
                        .set_data(MIME, PAYLOAD)
                        .expect("transfer write");
                    data.transfer.set_effect_allowed("copy");
                    if self.cancel_start {
                        state.prevent_default();
                    }
                }
                DomEventData::DragOver(data) => {
                    assert_eq!(data.transfer.get_data(MIME).as_deref(), Some(PAYLOAD));
                    data.transfer.set_drop_effect("copy");
                    state.prevent_default();
                }
                DomEventData::Drop(data) => {
                    assert_eq!(data.transfer.get_data(MIME).as_deref(), Some(PAYLOAD));
                }
                _ => {}
            }
        }
    }

    fn mouse(x: f32, y: f32) -> BlitzMouseButtonEvent {
        BlitzMouseButtonEvent {
            x,
            y,
            button: MouseEventButton::Main,
            buttons: MouseEventButtons::Primary,
            mods: Default::default(),
        }
    }

    fn document() -> (BaseDocument, usize, usize) {
        let mut doc = BaseDocument::new(DocumentConfig::default());
        let mut mutator = doc.mutate();
        let source = mutator.create_element(
            QualName::new(None, ns!(html), local_name!("div")),
            vec![Attribute {
                name: QualName::new(None, ns!(html), local_name!("draggable")),
                value: "true".into(),
            }],
        );
        let target =
            mutator.create_element(QualName::new(None, ns!(html), local_name!("div")), vec![]);
        mutator.append_children(0, &[source, target]);
        drop(mutator);
        (doc, source, target)
    }

    #[test]
    fn slot_item_mime_survives_source_over_target_drop() {
        let (mut doc, source, target) = document();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut driver = EventDriver::new(
            doc.mutate(),
            Recorder {
                seen: seen.clone(),
                cancel_start: false,
            },
        );
        assert_eq!(driver.drag_source(source), Some(source));
        driver.doc_mut().drag_candidate = Some((source, 0.0, 0.0));
        driver.move_drag(Some(target), mouse(7.0, 0.0));
        assert!(!driver.doc().is_dragging());
        driver.move_drag(Some(target), mouse(9.0, 0.0));
        assert!(driver.doc().is_dragging());
        assert!(driver.finish_drag(mouse(9.0, 0.0)));
        assert!(!driver.doc().is_dragging());
        let events = seen.borrow();
        assert!(events.contains(&"dragstart"));
        assert!(events.contains(&"dragenter"));
        assert!(events.contains(&"dragover"));
        assert!(events.contains(&"drop"));
        assert!(events.contains(&"dragend"));
    }

    #[test]
    fn cancelled_start_never_enters_a_drop_target() {
        let (mut doc, source, target) = document();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut driver = EventDriver::new(
            doc.mutate(),
            Recorder {
                seen: seen.clone(),
                cancel_start: true,
            },
        );
        driver.doc_mut().drag_candidate = Some((source, 0.0, 0.0));
        driver.move_drag(Some(target), mouse(9.0, 0.0));
        assert!(!driver.doc().is_dragging());
        assert_eq!(&*seen.borrow(), &["dragstart"]);
    }

    #[test]
    fn pointer_cancel_ends_drag_without_drop() {
        let (mut doc, source, target) = document();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut driver = EventDriver::new(
            doc.mutate(),
            Recorder {
                seen: seen.clone(),
                cancel_start: false,
            },
        );
        driver.doc_mut().drag_candidate = Some((source, 0.0, 0.0));
        driver.move_drag(Some(target), mouse(9.0, 0.0));
        let transfer = driver.doc().drag_active.as_ref().unwrap().2.clone();
        driver.cancel_drag(mouse(10.0, 0.0));
        assert!(!driver.doc().is_dragging());
        assert_eq!(transfer.drop_effect(), "none");
        let events = seen.borrow();
        assert!(events.contains(&"dragleave"));
        assert!(events.contains(&"dragend"));
        assert!(!events.contains(&"drop"));
    }
}

#[cfg(test)]
mod touch_tests {
    use super::*;
    use crate::DocumentConfig;
    use blitz_traits::events::{BlitzTouchEvent, BlitzTouchPhase, BlitzTouchPoint};
    use std::{cell::RefCell, rc::Rc};

    struct Recorder(Rc<RefCell<Vec<(&'static str, BlitzTouchEvent)>>>);

    impl EventHandler for Recorder {
        fn handle_event(
            &mut self,
            _chain: &[usize],
            event: &mut DomEvent,
            _mutr: &mut DocumentMutator<'_>,
            _event_state: &mut EventState,
        ) {
            match &event.data {
                DomEventData::TouchStart(touch)
                | DomEventData::TouchMove(touch)
                | DomEventData::TouchEnd(touch)
                | DomEventData::TouchCancel(touch) => {
                    self.0.borrow_mut().push((event.name(), touch.clone()));
                }
                _ => {}
            }
        }
    }

    fn document() -> BaseDocument {
        let mut doc = BaseDocument::new(DocumentConfig::default());
        let mut mutator = doc.mutate();
        let html = mutator.create_element(
            crate::QualName::new(None, crate::ns!(html), markup5ever::local_name!("html")),
            vec![],
        );
        mutator.append_children(0, &[html]);
        drop(mutator);
        doc
    }

    fn point(id: u64, x: f32, y: f32) -> BlitzTouchPoint {
        BlitzTouchPoint { id, x, y }
    }

    fn touch(phase: BlitzTouchPhase, changed: BlitzTouchPoint, touches: &[BlitzTouchPoint]) -> UiEvent {
        UiEvent::Touch(BlitzTouchEvent {
            phase,
            changed,
            touches: touches.to_vec(),
            mods: Default::default(),
        })
    }

    #[test]
    fn two_fingers_reach_the_dom_as_touch_events_carrying_every_finger_and_its_id() {
        let mut doc = document();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut driver = EventDriver::new(doc.mutate(), Recorder(seen.clone()));
        driver.handle_ui_event(touch(BlitzTouchPhase::Start, point(7, 10.0, 10.0), &[point(7, 10.0, 10.0)]));
        driver.handle_ui_event(touch(
            BlitzTouchPhase::Start,
            point(9, 60.0, 50.0),
            &[point(7, 10.0, 10.0), point(9, 60.0, 50.0)],
        ));
        driver.handle_ui_event(touch(
            BlitzTouchPhase::Move,
            point(9, 70.0, 50.0),
            &[point(7, 10.0, 10.0), point(9, 70.0, 50.0)],
        ));
        driver.handle_ui_event(touch(BlitzTouchPhase::End, point(9, 70.0, 50.0), &[point(7, 10.0, 10.0)]));
        driver.handle_ui_event(touch(BlitzTouchPhase::Cancel, point(7, 10.0, 10.0), &[]));
        let seen = seen.borrow();
        let names: Vec<_> = seen.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["touchstart", "touchstart", "touchmove", "touchend", "touchcancel"]);
        assert_eq!(seen[1].1.touches.iter().map(|t| t.id).collect::<Vec<_>>(), [7, 9]);
        assert_eq!(seen[1].1.changed.id, 9);
        assert_eq!(seen[2].1.touches[1], point(9, 70.0, 50.0));
        assert_eq!(seen[3].1.touches.iter().map(|t| t.id).collect::<Vec<_>>(), [7]);
    }

    #[test]
    fn control_one_finger_is_one_touch_point_and_no_mouse_event_is_made_of_it() {
        let mut doc = document();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut driver = EventDriver::new(doc.mutate(), Recorder(seen.clone()));
        driver.handle_ui_event(touch(BlitzTouchPhase::Start, point(1, 5.0, 6.0), &[point(1, 5.0, 6.0)]));
        let seen = seen.borrow();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].1.touches, [point(1, 5.0, 6.0)]);
        assert_eq!(driver.doc().hover_node_id, None);
    }
}
