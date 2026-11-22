//! 塊の幅と文字の位置を測る。約物の詰め。

use super::line_break::{first_char, last_char};
use super::options::LayoutOptions;
use super::page::TcyPlace;
use super::paragraph::Unit;
use crate::markup::Style;
use crate::text::kinsoku::{is_closing_punct, is_opening_bracket};
use crate::text::metrics::{sanitize_advance, FontMetrics};
use crate::text::vertical::Writing;

pub(super) struct Measured {
    /// 塊の幅
    pub(super) width: f32,
    /// 各文字の (行頭からの相対位置, 送り幅)
    pub(super) places: Vec<(f32, f32)>,
    /// 縦中横の置き方(縦書きの縦中横の塊だけ)
    pub(super) tcy: Vec<TcyPlace>,
    /// 最初の文字の字形を描く位置のずれ(始め括弧の連続を詰めたとき負。[`squeeze_punct`])
    pub(super) shift: f32,
}

pub(super) fn measure(
    unit: &Unit,
    styles: &[Style],
    metrics: &dyn FontMetrics,
    opts: &LayoutOptions,
) -> Measured {
    let size = styles.get(unit.style).map_or(1.0, |s| s.glyph_size());
    if let Some((_, w)) = &unit.icon {
        // アイコン: 指定の長さ(字の大きさを掛ける)の場所を空ける。縦書きでも行の中の向きの長さ
        let width = w * size;
        return Measured {
            width,
            places: vec![(0.0, width)],
            tcy: Vec::new(),
            shift: 0.0,
        };
    }
    if unit.tcy && opts.writing == Writing::Vertical {
        // 縦中横: 横の送り幅で並べ、全角1文字分に収まらなければ横に縮める
        let h: Vec<f32> = unit
            .chars
            .iter()
            .map(|c| sanitize_advance(metrics.advance(c.ch, size)) / size)
            .collect();
        let total: f32 = h.iter().sum();
        let scale = if total > 1.0 { 1.0 / total } else { 1.0 };
        let mut offset = (1.0 - total * scale) / 2.0;
        let tcy = h
            .iter()
            .map(|w| {
                let p = TcyPlace { offset, scale };
                offset += w * scale;
                p
            })
            .collect();
        return Measured {
            width: size,
            places: vec![(0.0, size); unit.chars.len()],
            tcy,
            shift: 0.0,
        };
    }
    let adv = |c: char, size: f32| {
        sanitize_advance(match opts.writing {
            Writing::Horizontal => metrics.advance(c, size),
            Writing::Vertical => metrics.vertical_advance(c, size),
        })
    };
    let advances: Vec<f32> = unit.chars.iter().map(|c| adv(c.ch, size)).collect();
    let base: f32 = advances.iter().sum();
    let width = match &unit.ruby {
        Some(r) => {
            let rw: f32 = r.chars().map(|c| adv(c, size * opts.ruby_scale)).sum();
            base.max(rw - opts.ruby_overhang)
        }
        None => base,
    };
    // ルビが親文字より長いときは、親文字を均等に広げる
    let spacing = if width > base {
        (width - base) / advances.len() as f32
    } else {
        0.0
    };
    let mut x = 0.0;
    let mut places = Vec::with_capacity(advances.len());
    for a in advances {
        places.push((x + spacing / 2.0, a));
        x += a + spacing;
    }
    Measured {
        width,
        places,
        tcy: Vec::new(),
        shift: 0.0,
    }
}

/// 約物の詰め。
///
/// - 句読点・終わり括弧の後ろに約物が続くとき、前の約物の後ろの半角を詰める(字形は動かさない)
/// - 始め括弧の後ろに始め括弧が続くとき(`「『`)、後ろの括弧の前の半角を詰める。後ろの括弧は
///   送り幅を半角減らし、字形を描く位置を半角だけ前へずらす(`Glyph::shift`)。禁則が無いと
///   2つの間で改行して行頭の字形が行の外へ出るので、禁則があるときだけ詰める
pub(super) fn squeeze_punct(
    units: &[Unit],
    measured: &mut [Measured],
    glyph_size: &dyn Fn(&Unit) -> f32,
    kinsoku: bool,
) {
    for i in 0..units.len().saturating_sub(1) {
        let (u, next) = (&units[i], &units[i + 1]);
        if u.ruby.is_some() || u.tcy {
            continue;
        }
        let (a, b) = (last_char(u), first_char(next));
        if is_closing_punct(a) && (is_closing_punct(b) || is_opening_bracket(b)) {
            let m = &mut measured[i];
            if let Some(last) = m.places.last_mut() {
                let cut = (0.5 * glyph_size(u)).min(last.1 * 0.5);
                last.1 -= cut;
                m.width -= cut;
            }
        } else if kinsoku
            && is_opening_bracket(a)
            && is_opening_bracket(b)
            && next.ruby.is_none()
            && !next.tcy
            && next.icon.is_none()
        {
            let m = &mut measured[i + 1];
            let Some(first) = m.places.first().map(|p| p.1) else {
                continue;
            };
            let cut = (0.5 * glyph_size(next)).min(first * 0.5);
            m.places[0].1 -= cut;
            for p in &mut m.places[1..] {
                p.0 -= cut;
            }
            m.width -= cut;
            m.shift -= cut;
        }
    }
}
