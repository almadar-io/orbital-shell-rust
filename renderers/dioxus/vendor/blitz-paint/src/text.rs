use anyrender::PaintScene;
use blitz_dom::{
    BaseDocument,
    node::{TextBrush, TextClamp},
    util::ToColorColor,
};
use kurbo::{Affine, Point, Stroke};
use parley::{Line, PositionedLayoutItem};
use peniko::Fill;
use style::values::computed::TextDecorationLine;

pub(crate) fn stroke_text<'a>(
    scale: f64,
    scene: &mut impl PaintScene,
    lines: impl Iterator<Item = Line<'a, TextBrush>>,
    doc: &BaseDocument,
    pos: Point,
) {
    stroke_text_clamped(scale, scene, lines, 0, None, doc, pos);
}

// ORBITAL PATCH (blitz-paint 0.2, parity): the glyph id and advance of U+2026 in a run's own font.
fn ellipsis_glyph(run: &parley::Run<'_, TextBrush>) -> Option<(u32, f32)> {
    let font = run.font();
    let font_ref = parley::swash::FontRef::from_index(font.data.data(), font.index as usize)?;
    let glyph = font_ref.charmap().map('\u{2026}');
    if glyph == 0 {
        return None;
    }
    let advance = font_ref
        .glyph_metrics(run.normalized_coords())
        .scale(run.font_size())
        .advance_width(glyph);
    Some((glyph as u32, advance))
}

// ORBITAL PATCH (blitz-paint 0.2, parity): `line_count` and `clamp` carry -webkit-line-clamp and
// text-overflow: ellipsis. Lines past `clamp.max_lines` are not painted; a line that is the last
// visible one with more text after it, or that is wider than `clamp.width` under text-overflow,
// loses the glyphs that do not fit beside an ellipsis and ends with one. Right-to-left lines are
// painted whole.
pub(crate) fn stroke_text_clamped<'a>(
    scale: f64,
    scene: &mut impl PaintScene,
    lines: impl Iterator<Item = Line<'a, TextBrush>>,
    line_count: usize,
    clamp: Option<TextClamp>,
    doc: &BaseDocument,
    pos: Point,
) {
    let transform = Affine::translate((pos.x * scale, pos.y * scale));
    for (line_index, line) in lines.enumerate() {
        let mut ellipsis_limit = None;
        if let Some(clamp) = clamp {
            if clamp.max_lines.is_some_and(|max| line_index >= max) {
                break;
            }
            let metrics = line.metrics();
            let visible_end = metrics.offset + metrics.advance - metrics.trailing_whitespace;
            let more_lines_follow =
                clamp.max_lines == Some(line_index + 1) && line_count > line_index + 1;
            let overflows = clamp.ellipsis_on_overflow && visible_end > clamp.width + 0.5;
            let rtl = line.items().any(
                |item| matches!(&item, PositionedLayoutItem::GlyphRun(run) if run.run().is_rtl()),
            );
            if (more_lines_follow || overflows) && !rtl {
                ellipsis_limit = Some((clamp.width, visible_end));
            }
        }
        let last_glyph_run = ellipsis_limit.and_then(|_| {
            line.items()
                .enumerate()
                .filter(|(_, item)| matches!(item, PositionedLayoutItem::GlyphRun(_)))
                .map(|(index, _)| index)
                .last()
        });
        let mut line_done = false;
        for (item_index, item) in line.items().enumerate() {
            if line_done {
                break;
            }
            if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                let mut x = glyph_run.offset();
                let y = glyph_run.baseline();

                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                let metrics = run.metrics();
                let style = glyph_run.style();
                let synthesis = run.synthesis();
                let glyph_xform = synthesis
                    .skew()
                    .map(|angle| Affine::skew(angle.to_radians().tan() as f64, 0.0));

                // Styles
                let styles = doc
                    .get_node(style.brush.id)
                    .unwrap()
                    .primary_styles()
                    .unwrap();
                let itext_styles = styles.get_inherited_text();
                let text_styles = styles.get_text();
                let text_color = itext_styles.color.as_color_color();
                let text_decoration_color = text_styles
                    .text_decoration_color
                    .as_absolute()
                    .map(ToColorColor::as_color_color)
                    .unwrap_or(text_color);
                let text_decoration_brush = anyrender::Paint::from(text_decoration_color);
                let text_decoration_line = text_styles.text_decoration_line;
                let has_underline = text_decoration_line.contains(TextDecorationLine::UNDERLINE);
                let has_strikethrough =
                    text_decoration_line.contains(TextDecorationLine::LINE_THROUGH);

                let mut painted_advance = glyph_run.advance();
                if let Some((limit, visible_end)) = ellipsis_limit {
                    let ellipsis = ellipsis_glyph(&run);
                    let ellipsis_advance = ellipsis.map_or(0.0, |(_, advance)| advance);
                    let mut glyphs = Vec::new();
                    let mut cut = false;
                    for glyph in glyph_run.glyphs() {
                        let next = x + glyph.advance;
                        let in_trailing_whitespace = glyph.advance > 0.0 && x >= visible_end - 0.01;
                        if in_trailing_whitespace || next + ellipsis_advance > limit {
                            cut = true;
                            break;
                        }
                        glyphs.push(anyrender::Glyph {
                            id: glyph.id as _,
                            x: x + glyph.x,
                            y: y - glyph.y,
                        });
                        x = next;
                    }
                    if cut || last_glyph_run == Some(item_index) {
                        if let Some((id, advance)) = ellipsis {
                            glyphs.push(anyrender::Glyph { id: id as _, x, y });
                            x += advance;
                        }
                        line_done = true;
                    }
                    painted_advance = x - glyph_run.offset();
                    scene.draw_glyphs(
                        font,
                        font_size,
                        true, // hint
                        run.normalized_coords(),
                        Fill::NonZero,
                        &anyrender::Paint::from(text_color),
                        1.0, // alpha
                        transform,
                        glyph_xform,
                        glyphs.into_iter(),
                    );
                } else {
                    scene.draw_glyphs(
                        font,
                        font_size,
                        true, // hint
                        run.normalized_coords(),
                        Fill::NonZero,
                        &anyrender::Paint::from(text_color),
                        1.0, // alpha
                        transform,
                        glyph_xform,
                        glyph_run.glyphs().map(|glyph| {
                            let gx = x + glyph.x;
                            let gy = y - glyph.y;
                            x += glyph.advance;

                            anyrender::Glyph {
                                id: glyph.id as _,
                                x: gx,
                                y: gy,
                            }
                        }),
                    );
                }

                let mut draw_decoration_line =
                    |offset: f32, size: f32, brush: &anyrender::Paint| {
                        let x = glyph_run.offset() as f64;
                        let w = painted_advance as f64;
                        let y = (glyph_run.baseline() - offset + size / 2.0) as f64;
                        let line = kurbo::Line::new((x, y), (x + w, y));
                        scene.stroke(&Stroke::new(size as f64), transform, brush, None, &line)
                    };

                if has_underline {
                    let offset = metrics.underline_offset;
                    let size = metrics.underline_size;

                    // TODO: intercept line when crossing an descending character like "gqy"
                    draw_decoration_line(offset, size, &text_decoration_brush);
                }
                if has_strikethrough {
                    let offset = metrics.strikethrough_offset;
                    let size = metrics.strikethrough_size;

                    draw_decoration_line(offset, size, &text_decoration_brush);
                }
            }
        }
    }
}
