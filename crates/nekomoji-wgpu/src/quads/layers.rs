//! 発光・縁取り・影の形をアトラスに作り、後ろから 影・発光・縁取り・文字 の順に積む。

use nekomoji::{Outline, Shadow};

use super::glyph::{entry, raster};
use super::placement::parse_color;
use super::{Built, Quad};
use crate::atlas::{Atlas, Shape};
use crate::font::{Ch, FontSet};

/// 縁取りで太らせる半径の上限(px)。
const MAX_OUTLINE_PX: f32 = 64.0;

/// 発光の光の層: (広がり(文字の大きさに対する比), 強さ1のときの不透明度, 層の番号)。
/// 外へ広がる淡い光の上に、芯の近くの強い光を重ねてネオンらしくする(描く順)
const GLOW_LAYERS: [(f32, f32, u8); 2] = [(0.38, 0.6, 0), (0.12, 0.9, 1)];

/// 字形の覆いをぼかして、発光の光の形を作る。周りに `pad` px の余白を足す。
pub(super) fn glow_bitmap(w: u32, h: u32, src: &[u8], pad: u32) -> (u32, u32, Vec<u8>) {
    let (gw, gh) = (w + 2 * pad, h + 2 * pad);
    let mut a: Vec<f32> = vec![0.0; (gw * gh) as usize];
    for y in 0..h {
        for x in 0..w {
            a[((y + pad) * gw + x + pad) as usize] = src[(y * w + x) as usize] as f32;
        }
    }
    // 箱型のぼかしを縦横に3回かけて、ガウスぼかしに近づける
    let r = (pad / 2).max(1) as i32;
    let mut b = vec![0.0f32; a.len()];
    for _ in 0..3 {
        for (step, len, lines, stride) in [(1i32, gw, gh, gw), (gw as i32, gh, gw, 1)] {
            for l in 0..lines {
                let base = if step == 1 { l * stride } else { l };
                for i in 0..len as i32 {
                    let mut sum = 0.0;
                    for k in -r..=r {
                        let j = i + k;
                        if j >= 0 && j < len as i32 {
                            sum += a[(base as i32 + j * step) as usize];
                        }
                    }
                    b[(base as i32 + i * step) as usize] = sum / (2 * r + 1) as f32;
                }
            }
            std::mem::swap(&mut a, &mut b);
        }
    }
    // 光は字形より薄く広がるので、少し持ち上げる
    let data = a.iter().map(|v| (v * 2.2).min(255.0) as u8).collect();
    (gw, gh, data)
}

/// 直前に描いた文字の四角形(`out.quads[from..]`)に、発光の光を後ろへ足し、芯を少し白くする。
#[allow(clippy::too_many_arguments)]
pub(super) fn add_glow(
    font: &FontSet,
    atlas: &mut Atlas,
    out: &mut Built,
    from: usize,
    ch: Ch,
    px: f32,
    vertical: bool,
    glow: f32,
) {
    if glow <= 0.0 || from >= out.quads.len() {
        return;
    }
    let Some(e) = entry(font, atlas, ch, px, vertical) else {
        return;
    };
    if e.w == 0 || e.h == 0 {
        return;
    }
    let tex = atlas.size() as f32;
    let strength = glow.min(2.0);
    for (spread, opacity, mark) in GLOW_LAYERS {
        let pad = (px * spread).ceil().max(2.0) as u32;
        let key = (ch, px.round().max(1.0) as u32, vertical, Shape::Glow(mark));
        // 光は字の周りに余白を足すので、その分も一辺に入らなければ作らない(ぼかしも重い)
        if (key.1 as u64) + 2 * (pad as u64) + 2 > atlas.size() as u64 {
            atlas.reject(key);
            continue;
        }
        let Some(h) = atlas.get_or_insert(key, || {
            let r = raster(font, ch, px, vertical);
            let (w, hh, data) = glow_bitmap(r.width, r.height, &r.bitmap, pad);
            (w, hh, 0, 0, data)
        }) else {
            out.overflow = true;
            return;
        };
        for i in from..out.quads.len() {
            let q = out.quads[i];
            // 引き伸ばし・縮めた字形にも合わせる(中心は同じなので回転もそのまま)
            let (sx, sy) = (q.size[0] / e.w as f32, q.size[1] / e.h as f32);
            let [r, g, b, a] = q.color;
            out.halos.push(Quad {
                pos: [q.pos[0] - pad as f32 * sx, q.pos[1] - pad as f32 * sy],
                size: [h.w as f32 * sx, h.h as f32 * sy],
                uv_min: [h.x as f32 / tex, h.y as f32 / tex],
                uv_max: [(h.x + h.w) as f32 / tex, (h.y + h.h) as f32 / tex],
                color: [r, g, b, (opacity * strength).min(1.0) * a],
                extra: q.extra,
            });
        }
    }
    for i in from..out.quads.len() {
        let [r, g, b, a] = out.quads[i].color;
        // ネオンの芯: 光が強いほど白に寄せる
        let white = 0.35 * strength.min(1.0);
        out.quads[i].color = [
            r + (1.0 - r) * white,
            g + (1.0 - g) * white,
            b + (1.0 - b) * white,
            a,
        ];
    }
}

/// 字形の覆いを、横 `rx`・縦 `ry` px の楕円で太らせて縁取りの形を作る(縁はなめらかにする)。
/// 周りに `rx + 1`・`ry + 1` px の余白を足す(余白の外周は 0 なので、アトラスで隣とにじまない)。
pub(super) fn outline_bitmap(w: u32, h: u32, src: &[u8], rx: u32, ry: u32) -> (u32, u32, Vec<u8>) {
    let (padx, pady) = (rx + 1, ry + 1);
    let (ow, oh) = (w + 2 * padx, h + 2 * pady);
    // 楕円の中で 1、縁の 1px で 1 → 0 へ下がる重み。円なら clamp(r + 0.5 - 距離, 0, 1)
    let (ex, ey) = (rx as f32 + 0.5, ry as f32 + 0.5);
    let m = ex.min(ey);
    let weight = |dx: i32, dy: i32| {
        let n = ((dx as f32 / ex).powi(2) + (dy as f32 / ey).powi(2)).sqrt();
        ((1.0 - n) * m).clamp(0.0, 1.0)
    };
    let mut out = vec![0u8; (ow * oh) as usize];
    // 字の中(覆いが 255 の画素)は、各行で最も近い画素までの横の距離を先に求め、縦だけを調べる。
    // 重みは距離が近いほど大きいので、全部の画素を楕円で広げるのと同じ結果になる
    // (字の大きさの4乗だった手間が3乗になる)
    let none = u32::MAX;
    let mut near = vec![none; (h * ow) as usize];
    for y in 0..h {
        let row = &mut near[(y * ow) as usize..((y + 1) * ow) as usize];
        let full = |x: u32| src[(y * w + x) as usize] == 255;
        // 左から右へ、右から左へ、いちばん近い字の中の画素までの距離
        let mut last: Option<u32> = None;
        for (ox, d) in row.iter_mut().enumerate() {
            let ox = ox as u32;
            if ox >= padx && ox - padx < w && full(ox - padx) {
                last = Some(ox);
            }
            if let Some(l) = last {
                *d = ox - l;
            }
        }
        let mut last: Option<u32> = None;
        for ox in (0..ow).rev() {
            if ox >= padx && ox - padx < w && full(ox - padx) {
                last = Some(ox);
            }
            if let Some(l) = last {
                let d = &mut row[ox as usize];
                *d = (*d).min(l - ox);
            }
        }
    }
    for oy in 0..oh {
        for ox in 0..ow {
            let mut k = 0.0f32;
            for dy in -(ry as i32)..=ry as i32 {
                let sy = oy as i32 - pady as i32 - dy;
                if sy < 0 || sy >= h as i32 {
                    continue;
                }
                let d = near[(sy as u32 * ow + ox) as usize];
                if d == none || d > rx {
                    continue;
                }
                k = k.max(weight(d as i32, dy));
            }
            if k > 0.0 {
                out[(oy * ow + ox) as usize] = (255.0 * k).round() as u8;
            }
        }
    }
    // 字の縁(覆いが 0 と 255 の間)は数が少ないので、1つずつ楕円で広げる
    let mut kernel = Vec::new();
    for dy in -(ry as i32)..=ry as i32 {
        for dx in -(rx as i32)..=rx as i32 {
            let k = weight(dx, dy);
            if k > 0.0 {
                kernel.push((dx, dy, k));
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            let v = src[(y * w + x) as usize];
            if v == 0 || v == 255 {
                continue;
            }
            let (cx, cy) = ((x + padx) as i32, (y + pady) as i32);
            for &(dx, dy, k) in &kernel {
                let i = ((cy + dy) as u32 * ow + (cx + dx) as u32) as usize;
                let a = (v as f32 * k).round() as u8;
                if a > out[i] {
                    out[i] = a;
                }
            }
        }
    }
    (ow, oh, out)
}

/// 直前に描いた文字の四角形(`out.quads[from..]`)に、縁取りと影を後ろへ足す。
///
/// 縁取りは字形を太らせた形を、影は縁取り(無ければ字形)と同じ形をずらして塗る。
/// どちらも文字の四角形と中心・回転・引き伸ばしを揃えるので、縦書きの回した字・縦中横・
/// 上付き・下付き・飛ばされた文字にもそのまま効く。不透明度は文字に従う。
#[allow(clippy::too_many_arguments)]
pub(super) fn add_outline_shadow(
    font: &FontSet,
    atlas: &mut Atlas,
    out: &mut Built,
    from: usize,
    ch: Ch,
    px: f32,
    vertical: bool,
    outline: Option<&Outline>,
    shadow: Option<&Shadow>,
) {
    if from >= out.quads.len() || (outline.is_none() && shadow.is_none()) {
        return;
    }
    let tex = atlas.size() as f32;
    let first_outline = out.outlines.len();
    if let Some(o) = outline.filter(|o| o.width > 0.0) {
        let rgb = parse_color(&o.color).unwrap_or([0.0; 3]);
        let radius = o.width * px;
        let Some(e) = entry(font, atlas, ch, px, vertical) else {
            out.overflow = true;
            return;
        };
        if e.w == 0 || e.h == 0 {
            return;
        }
        for i in from..out.quads.len() {
            let q = out.quads[i];
            // 引き伸ばし・縮めた字形(ダッシュ・縦中横)でも、画面での太さが揃うように
            let (sx, sy) = (q.size[0] / e.w as f32, q.size[1] / e.h as f32);
            // 太らせる半径は MAX_OUTLINE_PX までにする(大きな字の太い縁取りで時間がかかりすぎないように)
            let rx = (radius / sx).round().clamp(1.0, MAX_OUTLINE_PX) as u32;
            let ry = (radius / sy).round().clamp(1.0, MAX_OUTLINE_PX) as u32;
            let key = (
                ch,
                px.round().max(1.0) as u32,
                vertical,
                Shape::Outline(rx, ry),
            );
            let Some(h) = atlas.get_or_insert(key, || {
                let r = raster(font, ch, px, vertical);
                let (w, hh, data) = outline_bitmap(r.width, r.height, &r.bitmap, rx, ry);
                (w, hh, 0, 0, data)
            }) else {
                out.overflow = true;
                return;
            };
            out.outlines.push(Quad {
                pos: [
                    q.pos[0] - (rx + 1) as f32 * sx,
                    q.pos[1] - (ry + 1) as f32 * sy,
                ],
                size: [h.w as f32 * sx, h.h as f32 * sy],
                uv_min: [h.x as f32 / tex, h.y as f32 / tex],
                uv_max: [(h.x + h.w) as f32 / tex, (h.y + h.h) as f32 / tex],
                color: [rgb[0], rgb[1], rgb[2], q.color[3]],
                extra: q.extra,
            });
        }
    }
    if let Some(s) = shadow {
        let rgb = parse_color(&s.color).unwrap_or([0.0; 3]);
        // ずれは画面の向き(縦書きでも右・下が正)。回転は中心が軸なので、位置をずらすだけでよい
        let d = [s.x * px, s.y * px];
        let src = if out.outlines.len() > first_outline {
            first_outline..out.outlines.len()
        } else {
            from..out.quads.len()
        };
        for i in src.clone() {
            let q = if out.outlines.len() > first_outline {
                out.outlines[i]
            } else {
                out.quads[i]
            };
            out.shadows.push(Quad {
                pos: [q.pos[0] + d[0], q.pos[1] + d[1]],
                color: [rgb[0], rgb[1], rgb[2], q.color[3]],
                ..q
            });
        }
    }
}

/// 影・発光の光・縁取りを、すべての文字より先(後ろ)に、後ろから この順で置く。
pub(super) fn finish_layers(out: &mut Built) {
    // 組み立て中の領域は次のフレームに使い回すので、取り出さずに中身だけを移す
    let back = out
        .shadows
        .drain(..)
        .chain(out.halos.drain(..))
        .chain(out.outlines.drain(..));
    out.quads.splice(0..0, back);
}
