// ORBITAL PATCH (blitz-paint 0.2, parity): CSS `mask-image` (alpha mode). The element is painted into an
// isolated group, then a DestIn layer paints the mask layers (tiled by the background tiling code) so the
// group keeps only the pixels the mask covers.
use super::background::TileLayers;
use super::ElementCx;
use crate::layers::maybe_with_layer;
use anyrender::PaintScene;
use blitz_dom::node::ImageData;
use peniko::{BlendMode, Compose, Mix};
use style::properties::generated::longhands::{
    background_clip::single_value::computed_value::T as MaskClip,
    background_origin::single_value::computed_value::T as MaskOrigin,
};
use style::values::generics::image::GenericImage;

impl ElementCx<'_> {
    pub(super) fn has_mask(&self) -> bool {
        self.style
            .get_svg()
            .mask_image
            .0
            .iter()
            .any(|image| !matches!(image, GenericImage::None))
    }

    pub(super) fn begin_mask(&self, scene: &mut impl PaintScene) {
        scene.push_layer(
            BlendMode::new(Mix::Normal, Compose::SrcOver),
            1.0,
            self.transform,
            &self.frame.border_box_path(),
        );
    }

    pub(super) fn end_mask(&self, scene: &mut impl PaintScene) {
        let clip = self.frame.border_box_path();
        scene.push_layer(
            BlendMode::new(Mix::Normal, Compose::DestIn),
            1.0,
            self.transform,
            &clip,
        );

        let svg_style = self.style.get_svg();
        let layers = TileLayers::mask(svg_style);
        for (idx, image) in svg_style.mask_image.0.iter().enumerate().rev() {
            maybe_with_layer(scene, true, 1.0, self.transform, &clip, |scene| match image {
                GenericImage::Gradient(gradient) => self.draw_gradient_layer(
                    scene,
                    gradient,
                    idx,
                    MaskClip::BorderBox,
                    MaskOrigin::BorderBox,
                    &layers,
                ),
                GenericImage::Url(_) => {
                    let Some(Some(mask_image)) = self.element.mask_images.get(idx) else {
                        return;
                    };
                    match &mask_image.image {
                        ImageData::Raster(raster) => self.draw_raster_layer(
                            scene,
                            idx,
                            raster,
                            MaskOrigin::BorderBox,
                            &layers,
                        ),
                        #[cfg(feature = "svg")]
                        ImageData::Svg(tree) => self.draw_svg_layer(scene, idx, tree, &layers),
                        _ => {}
                    }
                }
                _ => {}
            });
        }

        scene.pop_layer();
        scene.pop_layer();
    }
}
