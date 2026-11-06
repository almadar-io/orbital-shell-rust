// ORBITAL PATCH (blitz-paint 0.2, parity): anyrender_svg 0.6 cannot paint `fill="url(#pattern)"`
// (`to_brush` returns None for `usvg::Paint::Pattern` and calls the error handler). This tiles
// the pattern's content group across the filled path, clipped to the path. Solid-colour fills
// and strokes only inside the pattern; tiles are not individually clipped.
//
// ORBITAL PATCH (blitz-paint 0.2, parity): anyrender_svg 0.6 silently ignores `<mask>`. A mask
// is applied here to the paths this handler paints (pattern-filled paths): the tiles are drawn
// in an isolated layer, then the mask content is drawn as coverage into a `Compose::DestIn`
// layer over them. Masks on groups holding solid/gradient paths still reach anyrender_svg
// unmasked (limit; fixing that needs anyrender_svg vendored). Nested `mask` on a mask is ignored.

use anyrender::PaintScene;
use kurbo::{Affine, BezPath, Cap, Join, Point, Rect, Stroke};
use color::DynamicColor;
use peniko::{BlendMode, Color, ColorStop, Compose, Fill, Gradient, Mix};
use std::collections::HashMap;
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

type MaskStack<'a> = Vec<(&'a usvg::Mask, Affine)>;

fn collect_masks<'a>(
    group: &'a usvg::Group,
    stack: &mut MaskStack<'a>,
    out: &mut HashMap<usize, MaskStack<'a>>,
) {
    let pushed = if let Some(mask) = group.mask() {
        stack.push((mask, affine(&group.abs_transform())));
        true
    } else {
        false
    };
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => collect_masks(g, stack, out),
            usvg::Node::Path(p) if !stack.is_empty() => {
                out.insert(&**p as *const usvg::Path as usize, stack.clone());
            }
            _ => {}
        }
    }
    if pushed {
        stack.pop();
    }
}

fn luminance(c: usvg::Color) -> f32 {
    (0.2125 * c.red as f32 + 0.7154 * c.green as f32 + 0.0721 * c.blue as f32) / 255.0
}

fn coverage(c: usvg::Color, opacity: f32, kind: usvg::MaskType) -> Color {
    let weight = match kind {
        usvg::MaskType::Luminance => luminance(c),
        usvg::MaskType::Alpha => 1.0,
    };
    Color::from_rgba8(
        255,
        255,
        255,
        (weight * opacity * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn coverage_stops(stops: &[usvg::Stop], opacity: f32, kind: usvg::MaskType) -> Vec<ColorStop> {
    stops
        .iter()
        .map(|stop| ColorStop {
            offset: stop.offset().get(),
            color: DynamicColor::from_alpha_color(coverage(
                stop.color(),
                stop.opacity().get() * opacity,
                kind,
            )),
        })
        .collect()
}

fn render_mask_group(
    scene: &mut impl PaintScene,
    group: &usvg::Group,
    ts: Affine,
    kind: usvg::MaskType,
) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => {
                render_mask_group(scene, g, ts * affine(&g.transform()), kind);
            }
            usvg::Node::Path(p) => {
                if !p.is_visible() {
                    continue;
                }
                let Some(fill) = p.fill() else {
                    continue;
                };
                let shape = bez(p);
                let rule = match fill.rule() {
                    usvg::FillRule::NonZero => Fill::NonZero,
                    usvg::FillRule::EvenOdd => Fill::EvenOdd,
                };
                let opacity = fill.opacity().get();
                match fill.paint() {
                    usvg::Paint::Color(c) => {
                        scene.fill(rule, ts, coverage(*c, opacity, kind), None, &shape);
                    }
                    usvg::Paint::LinearGradient(gr) => {
                        let gradient = Gradient::new_linear(
                            Point::new(gr.x1() as f64, gr.y1() as f64),
                            Point::new(gr.x2() as f64, gr.y2() as f64),
                        )
                        .with_stops(coverage_stops(gr.stops(), opacity, kind).as_slice());
                        scene.fill(
                            rule,
                            ts,
                            anyrender::Paint::Gradient(&gradient),
                            Some(affine(&gr.transform())),
                            &shape,
                        );
                    }
                    usvg::Paint::RadialGradient(gr) => {
                        let gradient = Gradient::new_two_point_radial(
                            Point::new(gr.fx() as f64, gr.fy() as f64),
                            0.0,
                            Point::new(gr.cx() as f64, gr.cy() as f64),
                            gr.r().get(),
                        )
                        .with_stops(coverage_stops(gr.stops(), opacity, kind).as_slice());
                        scene.fill(
                            rule,
                            ts,
                            anyrender::Paint::Gradient(&gradient),
                            Some(affine(&gr.transform())),
                            &shape,
                        );
                    }
                    usvg::Paint::Pattern(_) => {}
                }
            }
            _ => {}
        }
    }
}

fn apply_masks<S: PaintScene>(
    scene: &mut S,
    masks: &[(&usvg::Mask, Affine)],
    bounds: &Rect,
    bounds_ts: Affine,
    body: &mut dyn FnMut(&mut S),
) {
    let Some(((mask, mask_ts), rest)) = masks.split_first() else {
        body(scene);
        return;
    };
    scene.push_layer(
        BlendMode::new(Mix::Normal, Compose::SrcOver),
        1.0,
        bounds_ts,
        bounds,
    );
    apply_masks(scene, rest, bounds, bounds_ts, body);
    scene.push_layer(
        BlendMode::new(Mix::Normal, Compose::DestIn),
        1.0,
        bounds_ts,
        bounds,
    );
    let r = mask.rect();
    let mask_rect = Rect::new(
        r.left() as f64,
        r.top() as f64,
        r.right() as f64,
        r.bottom() as f64,
    );
    scene.push_clip_layer(*mask_ts, &mask_rect);
    render_mask_group(scene, mask.root(), *mask_ts, mask.kind());
    scene.pop_layer();
    scene.pop_layer();
    scene.pop_layer();
}

/// Error handler for `anyrender_svg::render_svg_tree_with`; `global` is the transform passed to it.
fn handle_unsupported<S: PaintScene>(
    scene: &mut S,
    node: &usvg::Node,
    global: Affine,
    masks: &HashMap<usize, MaskStack<'_>>,
) {
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
    let path_masks: MaskStack<'_> = masks
        .get(&(&**path as *const usvg::Path as usize))
        .map(|stack| {
            stack
                .iter()
                .map(|(mask, group_ts)| (*mask, global * *group_ts))
                .collect()
        })
        .unwrap_or_default();
    let pb = path.bounding_box();
    let bounds = Rect::new(
        pb.left() as f64,
        pb.top() as f64,
        pb.right() as f64,
        pb.bottom() as f64,
    );
    maybe_with_layer(scene, true, alpha, full, &local, |scene| {
        apply_masks(scene, &path_masks, &bounds, full, &mut |scene| {
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
    });
}

pub(crate) fn render_svg<S: PaintScene>(scene: &mut S, svg: &usvg::Tree, transform: Affine) {
    let mut masks = HashMap::new();
    collect_masks(svg.root(), &mut Vec::new(), &mut masks);
    anyrender_svg::render_svg_tree_with(scene, svg, transform, &mut |scene, node| {
        handle_unsupported(scene, node, transform, &masks)
    });
}
