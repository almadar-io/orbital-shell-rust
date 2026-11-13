mod driver;
mod ime;
mod keyboard;
mod mouse;

use blitz_traits::events::{DomEvent, DomEventData, WheelDeltaMode};
pub use driver::{EventDriver, EventHandler, NoopEventHandler};
pub(crate) use ime::handle_ime_event;
pub(crate) use keyboard::handle_keypress;
use mouse::handle_mouseup;
pub(crate) use mouse::{handle_click, handle_mousedown, handle_mousemove};

use crate::BaseDocument;

pub(crate) fn handle_dom_event<F: FnMut(DomEvent)>(
    doc: &mut BaseDocument,
    event: &mut DomEvent,
    dispatch_event: F,
) {
    let target_node_id = event.target;

    match &event.data {
        DomEventData::MouseMove(mouse_event) => {
            let changed = handle_mousemove(
                doc,
                target_node_id,
                mouse_event.x,
                mouse_event.y,
                mouse_event.buttons,
            );
            if changed {
                doc.shell_provider.request_redraw();
            }
        }
        DomEventData::MouseDown(event) => {
            handle_mousedown(doc, target_node_id, event.x, event.y);
        }
        DomEventData::MouseLeave(_) => {}
        DomEventData::MouseUp(event) => {
            handle_mouseup(doc, target_node_id, event, dispatch_event);
        }
        DomEventData::Click(event) => {
            handle_click(doc, target_node_id, event, dispatch_event);
        }
        DomEventData::KeyDown(event) => {
            handle_keypress(doc, target_node_id, event.clone(), dispatch_event);
        }
        DomEventData::KeyPress(_) => {
            // Do nothing (no default action)
        }
        DomEventData::KeyUp(_) => {
            // Do nothing (no default action)
        }
        DomEventData::Ime(event) => {
            handle_ime_event(doc, event.clone(), dispatch_event);
        }
        DomEventData::Input(_) => {
            // Do nothing (no default action)
        }
        DomEventData::DragStart(_)
        | DomEventData::DragEnter(_)
        | DomEventData::DragOver(_)
        | DomEventData::DragLeave(_)
        | DomEventData::Drop(_)
        | DomEventData::DragEnd(_) => {}
        // ORBITAL PATCH: multi-touch
        DomEventData::TouchStart(_)
        | DomEventData::TouchMove(_)
        | DomEventData::TouchEnd(_)
        | DomEventData::TouchCancel(_) => {}

        // ORBITAL PATCH: wheel events (G-ORBITAL-SHELL-RUST-033)
        DomEventData::Wheel(wheel) => {
            let px = match wheel.delta_mode {
                WheelDeltaMode::Pixel => 1.0,
                WheelDeltaMode::Line => 20.0,
            };
            // `scroll_*_by` takes winit's sign (positive scrolls up); DOM deltas are the opposite.
            let (dx, dy) = (-wheel.delta_x * px, -wheel.delta_y * px);
            let changed = if let Some(hover_node_id) = doc.get_hover_node_id() {
                doc.scroll_node_by_has_changed(hover_node_id, dx, dy)
            } else {
                doc.scroll_viewport_by_has_changed(dx, dy)
            };
            if changed {
                doc.shell_provider.request_redraw();
            }
        }
    }
}
