use peniko::ImageData;
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use vello::Renderer as VelloRenderer;
use wgpu::Texture;
pub use wgpu_context::DeviceHandle;

pub trait CustomPaintSource: 'static {
    fn resume(&mut self, device_handle: &DeviceHandle);
    fn suspend(&mut self);
    fn render(
        &mut self,
        ctx: CustomPaintCtx<'_>,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Option<TextureHandle>;
}

static PAINT_SOURCE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Default)]
pub struct PaintSourceRegistry(Rc<RefCell<PaintSourceRegistryInner>>);

#[derive(Default)]
struct PaintSourceRegistryInner {
    device: Option<DeviceHandle>,
    sources: FxHashMap<u64, Box<dyn CustomPaintSource>>,
}

impl PaintSourceRegistry {
    pub fn register(&self, mut source: Box<dyn CustomPaintSource>) -> u64 {
        let mut inner = self.0.borrow_mut();
        if let Some(device) = &inner.device {
            source.resume(device);
        }
        let id = PAINT_SOURCE_ID.fetch_add(1, Ordering::SeqCst);
        inner.sources.insert(id, source);
        id
    }

    pub fn unregister(&self, id: u64) {
        if let Some(mut source) = self.0.borrow_mut().sources.remove(&id) {
            source.suspend();
        }
    }

    pub fn resume(&self, device: &DeviceHandle) {
        let mut inner = self.0.borrow_mut();
        for source in inner.sources.values_mut() {
            source.resume(device);
        }
        inner.device = Some(device.clone());
    }

    pub fn suspend(&self) {
        let mut inner = self.0.borrow_mut();
        for source in inner.sources.values_mut() {
            source.suspend();
        }
        inner.device = None;
    }

    pub(crate) fn with_sources<R>(
        &self,
        use_sources: impl FnOnce(&mut FxHashMap<u64, Box<dyn CustomPaintSource>>) -> R,
    ) -> R {
        use_sources(&mut self.0.borrow_mut().sources)
    }
}

pub struct CustomPaintCtx<'r> {
    pub(crate) renderer: &'r mut VelloRenderer,
}

#[derive(Clone, PartialEq)]
pub struct TextureHandle(pub ImageData);

impl CustomPaintCtx<'_> {
    pub(crate) fn new<'a>(renderer: &'a mut VelloRenderer) -> CustomPaintCtx<'a> {
        CustomPaintCtx { renderer }
    }

    pub fn register_texture(&mut self, texture: Texture) -> TextureHandle {
        TextureHandle(self.renderer.register_texture(texture))
    }

    pub fn unregister_texture(&mut self, handle: TextureHandle) {
        self.renderer.unregister_texture(handle.0);
    }
}
