//! 縦書きの置き方: 縦書き用の字形があれば使い、なければ回転・ずらしをする。縦中横も。

use nekomoji::chars::{vertical_form, VerticalForm};
use nekomoji::{FontMetrics, GlyphInstance, Writing};

use super::build::flight_of;
use super::glyph::{entry, is_stretched_dash, quad, trim_dash_uv};
use super::layers::{add_glow, add_outline_shadow};
use super::placement::{color_of, deco_of, Placement};
use super::Built;
use crate::atlas::Atlas;
use crate::font::{Ch, FontSet};

/// 縦書きで字の枠の上端からベースラインまでの比
pub const VERTICAL_BASELINE: f32 = 0.88;
/// 縦書きで置き換えの無い句読点を右上へずらす量(全角1文字に対する比)
pub const VERTICAL_PUNCT_SHIFT: [f32; 2] = [0.6, -0.6];
/// 縦書きで置き換えの無い小書きの仮名を右上へずらす量(全角1文字に対する比)
pub const VERTICAL_SMALL_KANA_SHIFT: [f32; 2] = [0.1, -0.1];

/// 縦書きで1文字を置く。`(left, top)` は字の枠(幅 `px`)の左上。
#[allow(clippy::too_many_arguments)]
fn push_vertical(
    font: &FontSet,
    atlas: &mut Atlas,
    out: &mut Built,
    ch: Ch,
    px: f32,
    left: f32,
    top: f32,
    color: [f32; 4],
    rotation: f32,
) {
    if color[3] <= 0.0 {
        return;
    }
    let tex = atlas.size() as f32;
    let has_vert = font.vertical_glyph(ch).is_some();
    let Some(e) = entry(font, atlas, ch, px, has_vert) else {
        out.overflow = true;
        return;
    };
    if e.w == 0 || e.h == 0 {
        return;
    }
    // 縦書きでは字の枠を全角の正方形とし、ベースラインを下端から 0.12 の位置に置く
    // (和文フォントの仮想ボディの慣例。行の上下の余白を含む ascent では字が下へずれる)
    let ascent = px * VERTICAL_BASELINE;
    let (w, h) = (e.w as f32, e.h as f32);
    // 横書きの字の枠の中での、ビットマップの左上(枠の左上が原点)
    let bx = e.xmin as f32;
    let by = ascent - e.ymin as f32 - h;
    let form = if has_vert {
        VerticalForm::Upright
    } else {
        vertical_form(ch.ch)
    };
    // 横の送り幅(縦書きでは `palt` で詰めない)
    let adv = font.full_advance(ch.ch) * px;
    match form {
        VerticalForm::Sideways => {
            // 字の枠ごと時計回りに 90° 回す。枠の点 (u, v) は (px - v, u) へ移る
            let (mut cx, cy) = (bx + w / 2.0, by + h / 2.0);
            let mut w = w;
            if is_stretched_dash(ch.ch) {
                // 送り幅いっぱいに引き伸ばす(回した後は行方向)
                cx = adv / 2.0;
                w = adv + 1.0;
            }
            let center = [left + px - cy, top + cx];
            let mut q = quad(
                &e,
                [center[0] - w / 2.0, center[1] - h / 2.0],
                tex,
                color,
                rotation + std::f32::consts::FRAC_PI_2,
            );
            q.size[0] = w;
            if is_stretched_dash(ch.ch) {
                trim_dash_uv(&mut q, 0, tex);
            }
            out.quads.push(q);
        }
        _ => {
            let shift = match form {
                VerticalForm::Punctuation => VERTICAL_PUNCT_SHIFT,
                VerticalForm::SmallKana => VERTICAL_SMALL_KANA_SHIFT,
                _ => [0.0, 0.0],
            };
            // 送り幅が全角より狭い字は列の中央へ寄せる
            let center = (px - adv).max(0.0) / 2.0;
            // プロポーショナル詰め(`vpal`)では字形を置く位置もずらす
            let dy = font.vpal_shift(ch) * px;
            let mut q = quad(
                &e,
                [
                    left + center + bx + shift[0] * px,
                    top + by + shift[1] * px + dy,
                ],
                tex,
                color,
                rotation,
            );
            if has_vert && is_stretched_dash(ch.ch) {
                // 縦書き用のダッシュは字の枠の高さいっぱいに引き伸ばす
                q.pos[1] = top - 0.5;
                q.size[1] = px + 1.0;
                trim_dash_uv(&mut q, 1, tex);
            }
            out.quads.push(q);
        }
    }
}

/// 圏点の記号を、字面(インクの範囲)の中心が `center` に来るように置く。
///
/// 圏点の記号(﹅ など)はフォントによって字の枠の中の位置がまちまちなので、送り幅でなく字面で揃える。
#[allow(clippy::too_many_arguments)]
pub(super) fn push_centered(
    font: &FontSet,
    atlas: &mut Atlas,
    out: &mut Built,
    ch: Ch,
    px: f32,
    center: [f32; 2],
    color: [f32; 4],
    rotation: f32,
) {
    if color[3] <= 0.0 {
        return;
    }
    let tex = atlas.size() as f32;
    let Some(e) = entry(font, atlas, ch, px, false) else {
        out.overflow = true;
        return;
    };
    if e.w == 0 || e.h == 0 {
        return;
    }
    let pos = [center[0] - e.w as f32 / 2.0, center[1] - e.h as f32 / 2.0];
    out.quads.push(quad(&e, pos, tex, color, rotation));
}

/// 縦中横の1文字を置く。立てたまま(縦書き用の字形にせず)、横だけ `t.scale` 倍に縮める。
/// `at` は字の枠(全角の正方形)の左上。
#[allow(clippy::too_many_arguments)]
fn push_tcy(
    font: &FontSet,
    atlas: &mut Atlas,
    out: &mut Built,
    ch: Ch,
    px: f32,
    at: [f32; 2],
    t: nekomoji::TcyPlace,
    color: [f32; 4],
    rotation: f32,
) {
    if color[3] <= 0.0 {
        return;
    }
    let tex = atlas.size() as f32;
    let Some(e) = entry(font, atlas, ch, px, false) else {
        out.overflow = true;
        return;
    };
    if e.w == 0 || e.h == 0 {
        return;
    }
    let ascent = px * VERTICAL_BASELINE;
    let by = ascent - e.ymin as f32 - e.h as f32;
    let x = at[0] + t.offset * px + (e.xmin as f32 + font.palt_shift(ch) * px) * t.scale;
    let mut q = quad(&e, [x, at[1] + by], tex, color, rotation);
    q.size[0] *= t.scale;
    out.quads.push(q);
}

/// 縦書きのページを四角形にする。
pub(super) fn build_vertical(
    page: &nekomoji::Page,
    glyphs: &[GlyphInstance],
    knocked: &[nekomoji::neko::Knocked],
    font: &FontSet,
    atlas: &mut Atlas,
    place: &Placement,
    out: &mut Built,
) {
    let base = font.base_px;
    // 行(列)の右端(大きな字のある行は広げる)
    let right = |line: usize| place.origin[0] - place.line_offset(page, line, base);
    // 大きさ `size` の字の枠の右端(行で最も大きい字と中心をそろえる)
    let frame_right = |line: usize, size: f32| {
        right(line) - place.glyph_across(Writing::Vertical, page, line, size, 0.0, base)
    };
    for g in glyphs {
        // アイコン(`[icon]`)の絵は利用者が描く。字形(.notdef)は描かない
        if g.icon.is_some() {
            continue;
        }
        let px = base * g.size;
        // 上付き・下付きは周りの文字の枠の中央から、右(上付き)・左(下付き)へずらす
        let outer = base * g.baseline_size;
        let left = frame_right(g.line, g.baseline_size) - outer
            + (outer - px) / 2.0
            + g.rise * base
            + g.offset.0 * base;
        let top = place.origin[1] + (g.x + g.offset.1) * base;
        let from = out.quads.len();
        let (outline, shadow) = deco_of(g, place);
        if let Some(t) = g.tcy {
            push_tcy(
                font,
                atlas,
                out,
                Ch::new(g.ch, &g.tail),
                px,
                [left, top],
                t,
                color_of(g, place),
                g.rotation,
            );
            add_outline_shadow(
                font,
                atlas,
                out,
                from,
                Ch::new(g.ch, &g.tail),
                px,
                false,
                outline,
                shadow,
            );
            add_glow(
                font,
                atlas,
                out,
                from,
                Ch::new(g.ch, &g.tail),
                px,
                false,
                g.glow,
            );
            continue;
        }
        push_vertical(
            font,
            atlas,
            out,
            Ch::new(g.ch, &g.tail),
            px,
            left,
            top,
            color_of(g, place),
            g.rotation,
        );
        let vert = font.vertical_glyph(Ch::new(g.ch, &g.tail)).is_some();
        add_outline_shadow(
            font,
            atlas,
            out,
            from,
            Ch::new(g.ch, &g.tail),
            px,
            vert,
            outline,
            shadow,
        );
        add_glow(
            font,
            atlas,
            out,
            from,
            Ch::new(g.ch, &g.tail),
            px,
            vert,
            g.glow,
        );
    }

    // ルビ: 親文字の範囲の中央に、親文字の枠のすぐ右へ縦に並べる。大きさは親文字に合わせる
    for (ri, r) in page.rubies.iter().enumerate() {
        let Some(head) = glyphs.get(r.first_glyph) else {
            continue;
        };
        let ruby_px = base * place.ruby_scale * head.baseline_size;
        let head_right = frame_right(r.line, head.baseline_size);
        if r.emphasis {
            let (mut dx, mut dy, mut rot) = (head.offset.0 * base, 0.0, 0.0);
            if let Some(k) = flight_of(knocked, ri) {
                dx = k.fall.offset.0 * base;
                dy = k.fall.offset.1 * base;
                rot = k.fall.rotation;
            }
            let mark = r.text.chars().next().unwrap_or(nekomoji::EMPHASIS_MARK);
            let center = [
                head_right + ruby_px * 0.6 + dx,
                place.origin[1] + (r.x + r.width / 2.0) * base + dy,
            ];
            let from = out.quads.len();
            push_centered(
                font,
                atlas,
                out,
                mark.into(),
                ruby_px,
                center,
                color_of(head, place),
                rot,
            );
            let (outline, shadow) = deco_of(head, place);
            add_outline_shadow(
                font,
                atlas,
                out,
                from,
                mark.into(),
                ruby_px,
                false,
                outline,
                shadow,
            );
            continue;
        }
        let chars: Vec<char> = r.text.chars().collect();
        if chars.is_empty() {
            continue;
        }
        let adv: Vec<f32> = chars
            .iter()
            .map(|&c| font.vertical_advance(c, ruby_px / base) * base)
            .collect();
        let total: f32 = adv.iter().sum();
        let span = r.width * base;
        let (mut y, gap) = if total <= span || chars.len() == 1 {
            (place.origin[1] + r.x * base + (span - total) / 2.0, 0.0)
        } else {
            (
                place.origin[1] + r.x * base,
                (span - total) / (chars.len() - 1) as f32,
            )
        };
        let (mut dx, mut rot) = (head.offset.0 * base, 0.0);
        if let Some(k) = flight_of(knocked, ri) {
            dx = k.fall.offset.0 * base;
            y += k.fall.offset.1 * base;
            rot = k.fall.rotation;
        }
        let left = head_right + dx;
        let (outline, shadow) = deco_of(head, place);
        for (c, a) in chars.iter().zip(&adv) {
            let from = out.quads.len();
            push_vertical(
                font,
                atlas,
                out,
                (*c).into(),
                ruby_px,
                left,
                y,
                color_of(head, place),
                rot,
            );
            let vert = font.vertical_glyph(*c).is_some();
            add_outline_shadow(
                font,
                atlas,
                out,
                from,
                (*c).into(),
                ruby_px,
                vert,
                outline,
                shadow,
            );
            y += a + gap;
        }
    }
}
