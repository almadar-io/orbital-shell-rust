// ORBITAL PATCH (blitz-paint 0.2, parity): anyrender_svg 0.6 cannot paint `fill="url(#pattern)"`
// (`to_brush` returns None for `usvg::Paint::Pattern` and calls the error handler). This tiles
// the pattern's content group across the filled path, clipped to the path. Solid-colour fills
// and strokes only inside the pattern; tiles are not individually clipped.

use anyrender::PaintScene;
use kurbo::{Affine, BezPath, Cap, Join, Point, Rect, Stroke};
use peniko::{Color, Fill};
use usvg::tiny_skia_path::PathSegment;

use crate::layers::maybe_with_layer;

const MAX_TILES: i64 = 20_000;

fn affine(ts: &usvg::Transform) -> Affine {
    Affine::new([ts.sx, ts.ky, ts.kx, ts.sy, ts.tx, ts.ty].map(f64::from))
}

fn bez(path: &usvg::Path) -> BezPath {
    let mut out = BezPath::new();
    for seg in path.data().segments() {
        match seg {
            PathSegment::MoveTo(p) => out.move_to((p.x as f64, p.y as f64)),
            PathSegment::LineTo(p) => out.line_to((p.x as f64, p.y as f64)),
            PathSegment::QuadTo(a, b) => {
                out.quad_to((a.x as f64, a.y as f64), (b.x as f64, b.y as f64))
            }
            PathSegment::CubicTo(a, b, c) => out.curve_to(
                (a.x as f64, a.y as f64),
                (b.x as f64, b.y as f64),
                (c.x as f64, c.y as f64),
            ),
            PathSegment::Close => out.close_path(),
        }
    }
    out
}

fn solid(paint: &usvg::Paint, opacity: usvg::Opacity) -> Option<Color> {
    match paint {
        usvg::Paint::Color(c) => Some(Color::from_rgba8(
            c.red,
            c.green,
            c.blue,
            (opacity.get() * 255.0).round().clamp(0.0, 255.0) as u8,
        )),
        _ => None,
    }
}

fn render_group(scene: &mut impl PaintScene, group: &usvg::Group, ts: Affine) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => {
                let ts = ts * affine(&g.transform());
                let alpha = g.opacity().get();
                let bb = g.layer_bounding_box();
                let clip = Rect::new(
                    bb.left() as f64,
                    bb.top() as f64,
                    bb.right() as f64,
                    bb.bottom() as f64,
                );
                maybe_with_layer(scene, alpha < 1.0, alpha, ts, &clip, |scene| {
                    render_group(scene, g, ts)
                });
            }
            usvg::Node::Path(p) => {
                if !p.is_visible() {
                    continue;
                }
                let shape = bez(p);
                if let Some(fill) = p.fill() {
                    if let Some(color) = solid(fill.paint(), fill.opacity()) {
                        let rule = match fill.rule() {
                            usvg::FillRule::NonZero => Fill::NonZero,
                            usvg::FillRule::EvenOdd => Fill::EvenOdd,
                        };
                        scene.fill(rule, ts, color, None, &shape);
                    }
                }
                if let Some(stroke) = p.stroke() {
                    if let Some(color) = solid(stroke.paint(), stroke.opacity()) {
                        let cap = match stroke.linecap() {
                            usvg::LineCap::Butt => Cap::Butt,
                            usvg::LineCap::Round => Cap::Round,
                            usvg::LineCap::Square => Cap::Square,
                        };
                        let join = match stroke.linejoin() {
                            usvg::LineJoin::Round => Join::Round,
                            usvg::LineJoin::Bevel => Join::Bevel,
                            _ => Join::Miter,
                        };
                        let st = Stroke::new(stroke.width().get() as f64)
                            .with_caps(cap)
                            .with_join(join)
                            .with_miter_limit(stroke.miterlimit().get() as f64);
                        scene.stroke(&st, ts, color, None, &shape);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Error handler for `anyrender_svg::render_svg_tree_with`; `global` is the transform passed to it.
fn handle_unsupported<S: PaintScene>(scene: &mut S, node: &usvg::Node, global: Affine) {
    let usvg::Node::Path(path) = node else {
        return;
    };
    let Some(fill) = path.fill() else {
        return;
    };
    let usvg::Paint::Pattern(pattern) = fill.paint() else {
        return;
    };
    let full = global * affine(&node.abs_transform());
    let local = bez(path);
    let pt = pattern.transform();
    let Some(inv) = pt.invert() else {
        return;
    };
    let Some(bb) = path.bounding_box().transform(inv) else {
        return;
    };
    let rect = pattern.rect();
    let (w, h) = (rect.width() as f64, rect.height() as f64);
    let (px, py) = (rect.x() as f64, rect.y() as f64);
    let i0 = ((bb.left() as f64 - px) / w).floor() as i64;
    let i1 = ((bb.right() as f64 - px) / w).ceil() as i64;
    let j0 = ((bb.top() as f64 - py) / h).floor() as i64;
    let j1 = ((bb.bottom() as f64 - py) / h).ceil() as i64;
    if (i1 - i0).saturating_mul(j1 - j0) > MAX_TILES {
        return;
    }
    let base = full * affine(&pt);
    let alpha = fill.opacity().get();
    maybe_with_layer(scene, true, alpha, full, &local, |scene| {
        for j in j0..j1 {
            for i in i0..i1 {
                let origin = Point::new(px + i as f64 * w, py + j as f64 * h);
                render_group(
                    scene,
                    pattern.root(),
                    base * Affine::translate(origin.to_vec2()),
                );
            }
        }
    });
}

pub(crate) fn render_svg<S: PaintScene>(scene: &mut S, svg: &usvg::Tree, transform: Affine) {
    anyrender_svg::render_svg_tree_with(scene, svg, transform, &mut |scene, node| {
        handle_unsupported(scene, node, transform)
    });
}
