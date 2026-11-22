//! 猫モードの肉球のカーソル。

use nekomoji::{Motion, Writing};

use super::placement::Placement;
use super::{Built, Quad};
use crate::atlas::{Atlas, Shape};
use crate::font::FontSet;

/// 肉球のカーソルの大きさ(倍率 1.0 の文字の高さに対する比)
const PAW: f32 = 0.55;

/// 肉球(大きな掌球と4つの指球)の形を、縁をなめらかにした覆いの割合で作る。
pub(super) fn paw_bitmap(px: u32) -> (u32, u32, Vec<u8>) {
    let n = px.max(4);
    // (中心 x, 中心 y, 半径 x, 半径 y)。単位は一辺 1.0
    const PADS: [(f32, f32, f32, f32); 5] = [
        (0.50, 0.70, 0.26, 0.22),
        (0.16, 0.42, 0.095, 0.12),
        (0.35, 0.21, 0.09, 0.12),
        (0.65, 0.21, 0.09, 0.12),
        (0.84, 0.42, 0.095, 0.12),
    ];
    const SS: u32 = 4;
    let mut data = vec![0u8; (n * n) as usize];
    for y in 0..n {
        for x in 0..n {
            let mut hit = 0;
            for sy in 0..SS {
                for sx in 0..SS {
                    let u = (x as f32 + (sx as f32 + 0.5) / SS as f32) / n as f32;
                    let v = (y as f32 + (sy as f32 + 0.5) / SS as f32) / n as f32;
                    if PADS.iter().any(|&(cx, cy, rx, ry)| {
                        ((u - cx) / rx).powi(2) + ((v - cy) / ry).powi(2) <= 1.0
                    }) {
                        hit += 1;
                    }
                }
            }
            data[(y * n + x) as usize] = (hit * 255 / (SS * SS)) as u8;
        }
    }
    (n, n, data)
}

/// 入力待ちの肉球のカーソル。最後の文字の後ろ(縦書きでは下)に置き、ゆっくり明滅させる
/// (動きなしでは明滅も上下もさせない)。
#[allow(clippy::too_many_arguments)]
pub(super) fn push_paw(
    writing: Writing,
    page: &nekomoji::Page,
    c: &nekomoji::Cursor,
    motion: Motion,
    font: &FontSet,
    atlas: &mut Atlas,
    place: &Placement,
    out: &mut Built,
) {
    let base = font.base_px;
    let size = (base * PAW).round().max(4.0);
    let Some(e) = atlas.get_or_insert(('\u{1}'.into(), size as u32, false, Shape::Plain), || {
        let (w, h, data) = paw_bitmap(size as u32);
        (w, h, 0, 0, data)
    }) else {
        out.overflow = true;
        return;
    };
    let tex = atlas.size() as f32;
    // 倍率 1 の字の枠の始まり(行で最も大きい字とそろえた位置)
    let across = place.glyph_across(writing, page, c.line, 1.0, font.ascent(1.0), base);
    let (pulse, bob) = if motion == Motion::Off {
        (0.9, 0.0)
    } else {
        (
            0.65 + 0.35 * (c.since * 3.0).cos(),
            0.06 * base * (c.since * 3.0).sin(),
        )
    };
    let pos = match writing {
        Writing::Vertical => {
            let right = place.origin[0] - place.line_offset(page, c.line, base) - across;
            [
                right - (base + size) / 2.0,
                place.origin[1] + c.x * base + 0.2 * base + bob,
            ]
        }
        _ => {
            let top = place.origin[1] + place.line_offset(page, c.line, base) + across;
            [
                place.origin[0] + c.x * base + 0.2 * base,
                top + (base - size) / 2.0 + 0.1 * base + bob,
            ]
        }
    };
    let rgb = place.default_color;
    out.quads.push(Quad {
        pos,
        size: [size, size],
        uv_min: [e.x as f32 / tex, e.y as f32 / tex],
        uv_max: [(e.x + e.w) as f32 / tex, (e.y + e.h) as f32 / tex],
        color: [rgb[0], rgb[1], rgb[2], pulse],
        extra: [0.0, 0.0],
    });
}
