use dioxus::prelude::*;
use dioxus_core::use_hook_with_cleanup;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    pub rect: Option<[f32; 4]>,
    pub viewport: [f32; 2],
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            rect: None,
            viewport: [0.0, 0.0],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Intersection {
    pub visible: bool,
    pub element_fraction: f32,
    pub viewport_height_fraction: f32,
}

pub fn intersection(snapshot: Snapshot, margin: [f32; 4]) -> Intersection {
    let Some([x, y, width, height]) = snapshot.rect else {
        return Intersection {
            visible: false,
            element_fraction: 0.0,
            viewport_height_fraction: 0.0,
        };
    };
    let [viewport_width, viewport_height] = snapshot.viewport;
    let left = x.max(-margin[3]);
    let top = y.max(-margin[0]);
    let right = (x + width).min(viewport_width + margin[1]);
    let bottom = (y + height).min(viewport_height + margin[2]);
    let overlap_width = (right - left).max(0.0);
    let overlap_height = (bottom - top).max(0.0);
    let overlap = overlap_width * overlap_height;
    Intersection {
        visible: overlap > 0.0,
        element_fraction: if width > 0.0 && height > 0.0 {
            overlap / (width * height)
        } else {
            0.0
        },
        viewport_height_fraction: if viewport_height > 0.0 {
            overlap_height / viewport_height
        } else {
            0.0
        },
    }
}

pub fn root_margin(input: &str, viewport: [f32; 2]) -> Option<[f32; 4]> {
    let values = input.split_whitespace().collect::<Vec<_>>();
    if values.is_empty() || values.len() > 4 {
        return None;
    }
    let expanded = match values.as_slice() {
        [a] => [*a, *a, *a, *a],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d] => [*a, *b, *c, *d],
        _ => return None,
    };
    let mut margins = [0.0; 4];
    for (index, value) in expanded.into_iter().enumerate() {
        let reference = if index % 2 == 0 {
            viewport[1]
        } else {
            viewport[0]
        };
        margins[index] = if let Some(px) = value.strip_suffix("px") {
            px.parse::<f32>().ok()?
        } else if let Some(percent) = value.strip_suffix('%') {
            percent.parse::<f32>().ok()? * reference / 100.0
        } else {
            return None;
        };
    }
    Some(margins)
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
thread_local! {
    static OBSERVERS: RefCell<HashMap<usize, Signal<Snapshot>>> = RefCell::new(HashMap::new());
}

pub fn use_visibility() -> (usize, Signal<Snapshot>) {
    let snapshot = use_signal(Snapshot::default);
    let id = use_hook_with_cleanup(
        move || {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            OBSERVERS.with(|observers| {
                observers.borrow_mut().insert(id, snapshot);
            });
            id
        },
        move |id| {
            OBSERVERS.with(|observers| {
                observers.borrow_mut().remove(&id);
            });
        },
    );
    (id, snapshot)
}

pub fn sync_layout(window: &dioxus_native::Window, positions: &[(usize, [f32; 4])]) {
    let size = window.inner_size();
    let viewport = [size.width as f32, size.height as f32];
    let observers = OBSERVERS.with(|observers| {
        observers
            .borrow()
            .iter()
            .map(|(&id, &signal)| (id, signal))
            .collect::<Vec<_>>()
    });
    for (id, mut signal) in observers {
        let rect = positions
            .iter()
            .find(|(node_id, _)| *node_id == id)
            .map(|(_, bounds)| *bounds);
        let next = Snapshot { rect, viewport };
        if *signal.peek() != next {
            signal.set(next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_uses_element_and_viewport_fractions() {
        let snapshot = Snapshot {
            rect: Some([0.0, 70.0, 100.0, 100.0]),
            viewport: [100.0, 100.0],
        };
        let hit = intersection(snapshot, [0.0; 4]);
        assert!(hit.visible);
        assert_eq!(hit.element_fraction, 0.3);
        assert_eq!(hit.viewport_height_fraction, 0.3);
        assert!(
            !intersection(
                Snapshot {
                    rect: Some([0.0, 101.0, 100.0, 10.0]),
                    viewport: [100.0, 100.0]
                },
                [0.0; 4]
            )
            .visible
        );
    }

    #[test]
    fn root_margin_expands_all_four_edges() {
        assert_eq!(
            root_margin("10px 20%", [100.0, 200.0]),
            Some([10.0, 20.0, 10.0, 20.0])
        );
        assert_eq!(root_margin("bad", [100.0, 200.0]), None);
    }
}
