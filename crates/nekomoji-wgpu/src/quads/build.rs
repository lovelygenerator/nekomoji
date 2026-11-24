//! ページの表示済みの文字(とルビ)を四角形にする。横書きはここで組み、縦書きは `vertical` に任せる。

use nekomoji::{Layout, Neko, Reveal, Writing};

use super::glyph::{entry, is_stretched_dash, trim_dash_uv};
use super::icon::push_icon;
use super::layers::{add_glow, add_outline_shadow, finish_layers};
use super::lines::push_lines;
use super::placement::{color_of, deco_of, Placement};
use super::vertical::{build_vertical, push_centered};
use super::{Built, Quad};
use crate::atlas::Atlas;
use crate::font::{Ch, FontSet};

pub(super) fn build_page(
    layout: &Layout,
    reveal: &Reveal,
    neko: Option<&Neko>,
    font: &FontSet,
    atlas: &mut Atlas,
    place: &Placement,
    out: &mut Built,
) {
    out.reset();
    // 文字の描画命令は `out` の領域を使い回す。組み立て中は `out` に四角形を積むので、いったん取り出す
    let mut glyphs = std::mem::take(&mut out.glyphs);
    let knocked = match neko {
        Some(n) => {
            n.glyphs_into(layout, reveal, &mut glyphs);
            n.knocked(layout, reveal)
        }
        None => {
            reveal.glyphs_into(layout, &mut glyphs);
            Vec::new()
        }
    };
    if let Some(page) = layout.pages.get(reveal.page()) {
        if layout.writing == Writing::Vertical {
            build_vertical(page, &glyphs, &knocked, font, atlas, place, out);
        } else {
            build_horizontal(page, &glyphs, &knocked, font, atlas, place, out);
        }
        push_lines(layout, reveal, font, atlas, place, out);
        finish_layers(out);
    }
    out.glyphs = glyphs;
}

/// 横書きのページの文字とルビを四角形にする。
fn build_horizontal(
    page: &nekomoji::Page,
    glyphs: &[nekomoji::GlyphInstance],
    knocked: &[nekomoji::neko::Knocked],
    font: &FontSet,
    atlas: &mut Atlas,
    place: &Placement,
    out: &mut Built,
) {
    let base = font.base_px;
    let tex = atlas.size() as f32;

    // 行の上端(ルビの場所の先が本文。大きな字のある行は広げる)
    let line_top = |line: usize| place.origin[1] + place.line_offset(page, line, base);
    let ascent = font.ascent(1.0);
    // 大きさ `size` の字の枠の上端(行で最も大きい字とベースラインをそろえる)
    let frame_top = |line: usize, size: f32| {
        line_top(line) + place.glyph_across(Writing::Horizontal, page, line, size, ascent, base)
    };

    let push = |ch: Ch,
                px: f32,
                x: f32,
                top: f32,
                color: [f32; 4],
                rotation: f32,
                atlas: &mut Atlas,
                out: &mut Built| {
        if color[3] <= 0.0 {
            return;
        }
        let Some(e) = entry(font, atlas, ch, px, false) else {
            out.overflow = true;
            return;
        };
        if e.w == 0 || e.h == 0 {
            return;
        }
        let baseline = top + font.ascent(px);
        // プロポーショナル詰め(`palt`)では字形を置く位置もずらす
        let mut qx = x + e.xmin as f32 + font.palt_shift(ch) * px;
        let mut qw = e.w as f32;
        if is_stretched_dash(ch.ch) {
            // 両端の滑らかな縁で継ぎ目が暗くならないよう、1px 重ねる
            qx = x - 0.5;
            qw = nekomoji::FontMetrics::advance(font, ch.ch, 1.0) * px + 1.0;
        }
        let qy = baseline - e.ymin as f32 - e.h as f32;
        let mut q = Quad {
            pos: [qx, qy],
            size: [qw, e.h as f32],
            uv_min: [e.x as f32 / tex, e.y as f32 / tex],
            uv_max: [(e.x + e.w) as f32 / tex, (e.y + e.h) as f32 / tex],
            color,
            extra: [rotation, 0.0],
        };
        if is_stretched_dash(ch.ch) {
            trim_dash_uv(&mut q, 0, tex);
        }
        out.quads.push(q);
    };

    for g in glyphs {
        let px = base * g.size;
        let x = place.origin[0] + (g.x + g.offset.0) * base;
        // 上付き・下付きは周りの文字(baseline_size)のベースラインから rise だけずらす
        let shift = if g.rise != 0.0 || g.baseline_size != g.size {
            font.ascent(base * g.baseline_size) - font.ascent(px) - g.rise * base
        } else {
            0.0
        };
        let top = frame_top(g.line, g.baseline_size) + g.offset.1 * base + shift;
        // アイコン(`[icon]`)は字形ではなく絵を、字の枠(送り幅 × 字の大きさ)に収めて置く
        if g.icon.is_some() {
            push_icon(
                atlas,
                out,
                g,
                [x, top, g.width * base, px],
                color_of(g, place)[3],
            );
            continue;
        }
        let from = out.quads.len();
        push(
            Ch::new(g.ch, &g.tail),
            px,
            x,
            top,
            color_of(g, place),
            g.rotation,
            atlas,
            out,
        );
        let (outline, shadow) = deco_of(g, place);
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
    }

    // ルビ: 親文字の範囲の中央に、親文字の枠のすぐ上へ置く。大きさは親文字に合わせる
    for (ri, r) in page.rubies.iter().enumerate() {
        let Some(head) = glyphs.get(r.first_glyph) else {
            continue; // 親文字がまだ出ていない
        };
        let ruby_px = base * place.ruby_scale * head.baseline_size;
        let head_top = frame_top(r.line, head.baseline_size);
        if r.emphasis {
            let (dx, rot) = match flight_of(knocked, ri) {
                Some(k) => (k.fall.offset.0 * base, k.fall.rotation),
                None => (0.0, 0.0),
            };
            let mark = r.text.chars().next().unwrap_or(nekomoji::EMPHASIS_MARK);
            let center = [
                place.origin[0] + (r.x + r.width / 2.0) * base + dx,
                head_top - ruby_px * 0.4 + head.offset.1 * base,
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
            .map(|&c| nekomoji::FontMetrics::advance(font, c, ruby_px / base) * base)
            .collect();
        let total: f32 = adv.iter().sum();
        let span = r.width * base;
        // 収まるなら中央、はみ出すなら均等に配る(組版側が広げた幅に合わせる)
        let (mut x, gap) = if total <= span || chars.len() == 1 {
            (place.origin[0] + r.x * base + (span - total) / 2.0, 0.0)
        } else {
            (
                place.origin[0] + r.x * base,
                (span - total) / (chars.len() - 1) as f32,
            )
        };
        let top = head_top - ruby_px + head.offset.1 * base;
        // 飛ばされたルビは親文字と一緒に動く(揺れなどの x のずれは付けない)
        let (dx, rot) = match flight_of(knocked, ri) {
            Some(k) => (k.fall.offset.0 * base, k.fall.rotation),
            None => (0.0, 0.0),
        };
        x += dx;
        let (outline, shadow) = deco_of(head, place);
        for (c, a) in chars.iter().zip(&adv) {
            let from = out.quads.len();
            push(
                (*c).into(),
                ruby_px,
                x,
                top,
                color_of(head, place),
                rot,
                atlas,
                out,
            );
            add_outline_shadow(
                font,
                atlas,
                out,
                from,
                (*c).into(),
                ruby_px,
                false,
                outline,
                shadow,
            );
            x += a + gap;
        }
    }
}

/// ルビ `ri` を一緒に飛ばしている文字の動き(二度飛ばされていたら後のほう)。
pub(super) fn flight_of(
    knocked: &[nekomoji::neko::Knocked],
    ri: usize,
) -> Option<&nekomoji::neko::Knocked> {
    knocked.iter().rev().find(|k| k.ruby == Some(ri))
}
