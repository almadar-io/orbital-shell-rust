use std::sync::Mutex;

use blitz_dom::BaseDocument;

static EDITABLE_BOUNDS: Mutex<Vec<(f64, f64, f64, f64)>> = Mutex::new(Vec::new());

pub(crate) fn update(doc: &BaseDocument) {
    let mut bounds = Vec::new();
    let scroll = doc.viewport_scroll();
    let scale = doc.viewport().scale_f64();
    doc.visit(|_, node| {
        let Some(element) = node.element_data() else {
            return;
        };
        if element.name.local.as_ref() != "input"
            || !element.has_text_input_type()
            || element.attrs().iter().any(|attr| attr.name.local.as_ref() == "disabled")
        {
            return;
        }
        let position = node.absolute_position(0.0, 0.0);
        let size = node.final_layout.size;
        if size.width <= 0.0 || size.height <= 0.0 {
            return;
        }
        let x = (position.x as f64 - scroll.x) * scale;
        let y = (position.y as f64 - scroll.y) * scale;
        bounds.push((x, y, x + size.width as f64 * scale, y + size.height as f64 * scale));
    });
    *EDITABLE_BOUNDS.lock().unwrap() = bounds;
}

pub fn contains(x: f32, y: f32) -> bool {
    EDITABLE_BOUNDS.lock().unwrap().iter().any(|bounds| {
        bounds.0 <= x as f64
            && x as f64 <= bounds.2
            && bounds.1 <= y as f64
            && y as f64 <= bounds.3
    })
}
