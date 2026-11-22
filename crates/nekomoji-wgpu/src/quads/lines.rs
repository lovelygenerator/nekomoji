//! 取り消し線・下線を四角形にする。

use nekomoji::{DecorationInstance, Layout, Reveal, Writing};

use super::placement::{parse_color, Placement};
use super::{Built, Quad};
use crate::atlas::{Atlas, Shape};
use crate::font::FontSet;

/// 取り消し線・下線を四角形にする。文字のアトラスの塗りつぶしの点を使う。
pub(super) fn push_lines(
    layout: &Layout,
    reveal: &Reveal,
    font: &FontSet,
    atlas: &mut Atlas,
    place: &Placement,
    out: &mut Built,
) {
    // 線の描画命令は `out` の領域を使い回す。組み立て中は `out` に四角形を積むので、いったん取り出す
    let mut lines = std::mem::take(&mut out.lines);
    reveal.decorations_into(layout, &mut lines);
    if let (false, Some(page)) = (lines.is_empty(), layout.pages.get(reveal.page())) {
        push_line_quads(layout, page, &lines, font, atlas, place, out);
    }
    out.lines = lines;
}

fn push_line_quads(
    layout: &Layout,
    page: &nekomoji::Page,
    lines: &[DecorationInstance],
    font: &FontSet,
    atlas: &mut Atlas,
    place: &Placement,
    out: &mut Built,
) {
    // 3×3 の塗りつぶし。中央の画素だけを引けば、線形補間でも縁がにじまない
    let Some(e) = atlas.get_or_insert(('\u{0}'.into(), 0, false, Shape::Plain), || {
        (3, 3, 0, 0, vec![255; 9])
    }) else {
        out.overflow = true;
        return;
    };
    let tex = atlas.size() as f32;
    let uv = [(e.x as f32 + 1.5) / tex, (e.y as f32 + 1.5) / tex];
    let base = font.base_px;
    let ascent = font.ascent(1.0);
    for d in lines {
        // 線を引く文字の枠の始まり(行で最も大きい字とそろえた位置)
        let across = place.glyph_across(layout.writing, page, d.line, d.size, ascent, base);
        let start = place.line_offset(page, d.line, base) + across;
        let rgb = d
            .color
            .as_deref()
            .and_then(parse_color)
            .unwrap_or(place.default_color);
        let th = (d.thickness * base).max(1.0);
        let len = (d.x1 - d.x0) * base;
        let (pos, size) = match layout.writing {
            Writing::Horizontal => {
                let top = place.origin[1] + start;
                (
                    [place.origin[0] + d.x0 * base, top + d.y * base - th / 2.0],
                    [len, th],
                )
            }
            Writing::Vertical => {
                let right = place.origin[0] - start;
                (
                    [right - d.y * base - th / 2.0, place.origin[1] + d.x0 * base],
                    [th, len],
                )
            }
        };
        out.quads.push(Quad {
            pos,
            size,
            uv_min: uv,
            uv_max: uv,
            color: [rgb[0], rgb[1], rgb[2], d.alpha],
            extra: [0.0, 0.0],
        });
    }
}
