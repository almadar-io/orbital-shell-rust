//! The parts of painting a Dioxus document that need no window: the head
//! sink that turns `document::Style` into stylesheets, the embedded fonts,
//! and (native) a one-shot offscreen render to PNG.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// A `<head>` element the VDOM emitted, applied to the document on the next frame.
pub struct HeadOp {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub contents: Option<String>,
}

/// `dioxus_document::Document` provided to the VDOM root: queues head elements
/// (`document::Style` → CSS for Stylo) for the host to apply.
pub struct HeadSink {
    pub ops: Rc<RefCell<VecDeque<HeadOp>>>,
}

impl dioxus_document::Document for HeadSink {
    fn eval(&self, js: String) -> dioxus_document::Eval {
        dioxus_document::NoOpDocument.eval(js)
    }

    fn create_head_element(&self, name: &str, attributes: &[(&str, String)], contents: Option<String>) {
        self.ops.borrow_mut().push_back(HeadOp {
            name: name.to_string(),
            attributes: attributes.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
            contents,
        });
    }
}

/// Inter (OFL) — the sans typeface for `ui-sans-serif`/`sans-serif`.
const INTER_TTF: &[u8] = include_bytes!("../../../../renderers/dioxus/assets/fonts/Inter.ttf");
/// Roboto Mono (OFL) — the mono typeface for `ui-monospace`/`monospace`.
const ROBOTO_MONO_TTF: &[u8] = include_bytes!("../../../../renderers/dioxus/assets/fonts/RobotoMono.ttf");

/// The Parley font context with the embedded typefaces, every CSS generic
/// family almadar.css resolves to mapped onto them (`--almadar-font:
/// ui-sans-serif, system-ui, …, sans-serif`). Cached per thread, cloned per
/// document: rebuilding means re-registering and cold-shaping every run.
pub fn embedded_font_context() -> parley::FontContext {
    use fontique::{Blob, GenericFamily};
    use std::sync::Arc;

    std::thread_local! {
        static FONT_CTX: std::cell::OnceCell<parley::FontContext> = const { std::cell::OnceCell::new() };
    }
    FONT_CTX.with(|cell| {
        cell.get_or_init(|| {
            let mut font_ctx = parley::FontContext::default();
            let sans = font_ctx.collection.register_fonts(Blob::new(Arc::new(INTER_TTF) as _), None);
            let mono = font_ctx.collection.register_fonts(Blob::new(Arc::new(ROBOTO_MONO_TTF) as _), None);
            let sans_ids = sans.iter().map(|(id, _)| *id);
            let mono_ids = mono.iter().map(|(id, _)| *id);
            for generic in [GenericFamily::SansSerif, GenericFamily::SystemUi, GenericFamily::UiSansSerif, GenericFamily::UiRounded] {
                font_ctx.collection.set_generic_families(generic, sans_ids.clone());
            }
            for generic in [GenericFamily::Monospace, GenericFamily::UiMonospace] {
                font_ctx.collection.set_generic_families(generic, mono_ids.clone());
            }
            tracing::info!(target: crate::shell::NS, sans = sans.len(), mono = mono.len(), "fonts embedded (cached per thread)");
            font_ctx
        })
        .clone()
    })
}

/// Paint the mounted app (`crate::shell::mount`) once, `width` × `height`
/// pixels on white, with the stylesheets in `crate::app::CSS_TEXT`, and write
/// it to `path` as a PNG.
#[cfg(not(target_arch = "wasm32"))]
pub fn capture_png(path: &std::path::Path, width: u32, height: u32) -> Result<(), String> {
    use anyrender::ImageRenderer;
    use anyrender_vello_cpu::VelloCpuImageRenderer;
    use blitz_dom::{Document as _, DocumentConfig};
    use blitz_paint::paint_scene;
    use blitz_traits::shell::{ColorScheme, Viewport};
    use dioxus_core::{provide_context, ScopeId, VirtualDom};
    use dioxus_native::DioxusDocument;

    let ops: Rc<RefCell<VecDeque<HeadOp>>> = Rc::new(RefCell::new(VecDeque::new()));
    let vdom = VirtualDom::new(crate::app::App);
    let sink = HeadSink { ops: ops.clone() };
    vdom.in_scope(ScopeId::ROOT, || {
        provide_context(Rc::new(sink) as Rc<dyn dioxus_document::Document>);
    });
    let mut doc = DioxusDocument::new(
        vdom,
        DocumentConfig {
            viewport: Some(Viewport::new(width, height, 1.0, ColorScheme::Light)),
            font_ctx: Some(embedded_font_context()),
            ..Default::default()
        },
    );
    doc.initial_build();
    // Head elements land after a render; poll until the document is quiet.
    for _ in 0..8 {
        let mut changed = false;
        while let Some(op) = ops.borrow_mut().pop_front() {
            doc.create_head_element(&op.name, &op.attributes, &op.contents);
            changed = true;
        }
        changed |= doc.poll(None);
        if !changed {
            break;
        }
    }
    doc.resolve(0.0);

    let mut renderer = VelloCpuImageRenderer::new(width, height);
    let mut buffer = vec![255u8; width as usize * height as usize * 4];
    renderer.render(|painter| paint_scene(painter, &doc, 1.0, width, height), &mut buffer);
    // The rasterizer leaves unpainted pixels transparent (premultiplied); composite onto white.
    for px in buffer.chunks_exact_mut(4) {
        let blank = 255 - px[3];
        for channel in &mut px[..3] {
            *channel = channel.saturating_add(blank);
        }
        px[3] = 255;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&buffer).map_err(|e| e.to_string())
}
