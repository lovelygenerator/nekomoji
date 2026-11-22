//! 論理座標の重ね描き。行・字の枠・ルビ・線の上限・カーソル・猫の位置を、細い線と薄い塗りで示す。
//!
//! シナリオを書くときや描画の不具合を探すときに、nekomoji がどこに何を置いたかを目で確かめるためのもの。
//! GPU に触れない純粋な処理で、[`crate::build_quads`] の四角形の後ろ(上)に積んで描く。
//!
//! 論理座標は、行の中の位置 `x`(全角1文字 = 1.0)と行の番号。横書きでは `x` が右へ、行が下へ、
//! 縦書きでは `x` が下へ、行が左へ進む。画面との変換は [`Placement::to_screen`]・
//! [`Placement::to_logical`]。大きな字のある行は行送りが広がるので、変換にはページを渡す。

use crate::atlas::{Atlas, Shape};
use crate::font::FontSet;
use crate::quads::{Placement, Quad};
use nekomoji::{Layout, Neko, Reveal, Span, Writing};

/// 重ね描きの設定。
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Overlay {
    /// 行の長さの上限([`LayoutOptions::max_width`](nekomoji::LayoutOptions::max_width))。あれば上限の位置に線を引く
    pub max_width: Option<f32>,
    /// 1ページの行数の上限([`LayoutOptions::max_lines`](nekomoji::LayoutOptions::max_lines))。あれば枠の下端(縦書きでは左端)に線を引く
    pub max_lines: Option<usize>,
    /// まだ出ていない文字の枠も描くか(薄く描く)
    pub show_hidden: bool,
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            max_width: None,
            max_lines: None,
            show_hidden: true,
        }
    }
}

impl Overlay {
    /// 行の長さの上限([`Overlay::max_width`])。
    #[must_use]
    pub fn max_width(mut self, value: impl Into<Option<f32>>) -> Self {
        self.max_width = value.into();
        self
    }

    /// 1ページの行数の上限([`Overlay::max_lines`])。
    #[must_use]
    pub fn max_lines(mut self, value: impl Into<Option<usize>>) -> Self {
        self.max_lines = value.into();
        self
    }

    /// まだ出ていない文字の枠も描くか([`Overlay::show_hidden`])。
    #[must_use]
    pub fn show_hidden(mut self, value: bool) -> Self {
        self.show_hidden = value;
        self
    }
}

/// 重ね描きの色(RGBA)
mod color {
    pub const SLOT: [f32; 4] = [0.6, 0.6, 0.7, 0.35];
    pub const RUBY_ROOM: [f32; 4] = [0.3, 0.8, 0.9, 0.07];
    pub const CELL: [f32; 4] = [0.3, 0.9, 0.4, 0.55];
    pub const CELL_HIDDEN: [f32; 4] = [0.3, 0.9, 0.4, 0.18];
    pub const RUBY: [f32; 4] = [0.3, 0.8, 0.95, 0.7];
    pub const RULER: [f32; 4] = [1.0, 0.85, 0.3, 0.7];
    pub const LIMIT: [f32; 4] = [1.0, 0.35, 0.35, 0.8];
    pub const CURSOR: [f32; 4] = [1.0, 0.5, 0.85, 0.9];
    pub const CAT: [f32; 4] = [1.0, 0.6, 0.2, 0.9];
}

impl Placement {
    /// 論理座標の矩形を画面の矩形 `[左, 上, 幅, 高さ]`(px)にする。
    ///
    /// `x0..x1` は行の中の範囲(全角1文字 = 1.0)、`across0..across1` は行の始まりの辺
    /// (横書きは行の上端、縦書きは行の右端)から行の内側へ測った距離(px)。行の始まりから
    /// [`Placement::ruby_room`] 分がルビの場所で、その先が本文。行の始まりの辺の位置は
    /// [`Placement::line_offset`](`layout` のページ `page` の行の大きさで決まる)。
    #[allow(clippy::too_many_arguments)]
    pub fn to_screen(
        &self,
        layout: &Layout,
        page: usize,
        base_px: f32,
        line: usize,
        x0: f32,
        x1: f32,
        across0: f32,
        across1: f32,
    ) -> [f32; 4] {
        let start = match layout.pages.get(page) {
            Some(p) => self.line_offset(p, line, base_px),
            None => line as f32 * self.line_pitch * base_px,
        };
        match layout.writing {
            Writing::Vertical => {
                let right = self.origin[0] - start;
                [
                    right - across1,
                    self.origin[1] + x0 * base_px,
                    across1 - across0,
                    (x1 - x0) * base_px,
                ]
            }
            _ => [
                self.origin[0] + x0 * base_px,
                self.origin[1] + start + across0,
                (x1 - x0) * base_px,
                across1 - across0,
            ],
        }
    }

    /// 行ごとの範囲([`Span`]。[`Page::spans`](nekomoji::Page::spans)・[`Layout::mark_spans`](nekomoji::Layout::mark_spans) の結果)を、本文の部分の
    /// 画面の矩形 `[左, 上, 幅, 高さ]`(px)にする。行と直交する方向は、ルビの場所の先から
    /// その行で最も大きい文字の高さまで。範囲の強調やクリックできる語の当たりの表示に使う。
    pub fn span_rects(
        &self,
        layout: &Layout,
        page: usize,
        base_px: f32,
        spans: &[Span],
    ) -> Vec<[f32; 4]> {
        let Some(p) = layout.pages.get(page) else {
            return Vec::new();
        };
        spans
            .iter()
            .map(|s| {
                let size = p.lines.get(s.line).map_or(1.0, |l| l.size);
                let ruby_room = self.ruby_room(p, s.line, base_px);
                self.to_screen(
                    layout,
                    page,
                    base_px,
                    s.line,
                    s.x0,
                    s.x1,
                    ruby_room,
                    ruby_room + size * base_px,
                )
            })
            .collect()
    }

    /// ページの文字(ルビの場所を含む)を囲む画面の矩形 `[左, 上, 幅, 高さ]`(px)。
    /// 幅は [`Page::extent`](nekomoji::Page::extent) の最も長い行、行の方向は最初の行の始まりの辺から最後の行の本文の端まで。
    /// テキストボックスの枠の大きさを文字に合わせるときに使う。行が無ければ大きさ 0。
    pub fn extent_rect(&self, layout: &Layout, page: usize, base_px: f32) -> [f32; 4] {
        let Some(p) = layout.pages.get(page) else {
            return [self.origin[0], self.origin[1], 0.0, 0.0];
        };
        let e = p.extent();
        let Some(last) = p.lines.last() else {
            return [self.origin[0], self.origin[1], 0.0, 0.0];
        };
        let ruby_room = self.ruby_room(p, e.lines - 1, base_px);
        let first = self.to_screen(layout, page, base_px, 0, 0.0, e.width, 0.0, 0.0);
        let end = self.to_screen(
            layout,
            page,
            base_px,
            e.lines - 1,
            0.0,
            e.width,
            0.0,
            ruby_room + last.size * base_px,
        );
        let left = first[0].min(end[0]);
        let top = first[1].min(end[1]);
        let right = (first[0] + first[2]).max(end[0] + end[2]);
        let bottom = (first[1] + first[3]).max(end[1] + end[3]);
        [left, top, right - left, bottom - top]
    }

    /// 画面の点(px)を、`layout` のページ `page` の論理座標 `[x, 行]` にする。行は小数で返す
    /// (0.0〜1.0 が 0 行目の行送りの範囲。大きな字のある行は行送りが広い)。
    /// 負の値や行数を超える値もそのまま返す(行の外は倍率 1 の行送りで数える)。
    pub fn to_logical(
        &self,
        layout: &Layout,
        page: usize,
        base_px: f32,
        point: [f32; 2],
    ) -> [f32; 2] {
        let (x, d) = match layout.writing {
            Writing::Vertical => (point[1] - self.origin[1], self.origin[0] - point[0]),
            _ => (point[0] - self.origin[0], point[1] - self.origin[1]),
        };
        let line = match layout.pages.get(page) {
            Some(p) => self.line_at(p, d, base_px),
            None => d / (self.line_pitch * base_px),
        };
        [x / base_px, line]
    }
}

/// 画面の矩形 `[左, 上, 幅, 高さ]`(px)を塗る(`outline` なら 1px の枠だけ)四角形を作る。
///
/// [`Placement::span_rects`]・[`Placement::extent_rect`] の結果を描くのに使う。塗りには
/// アトラスの 3×3 の塗りの点を使う(線と同じ)。アトラスが満杯なら空を返す。
pub fn build_rects(
    atlas: &mut Atlas,
    rects: &[[f32; 4]],
    color: [f32; 4],
    outline: bool,
) -> Vec<Quad> {
    let Some(e) = atlas.get_or_insert(('\u{0}'.into(), 0, false, Shape::Plain), || {
        (3, 3, 0, 0, vec![255; 9])
    }) else {
        return Vec::new();
    };
    let tex = atlas.size() as f32;
    let uv = [(e.x as f32 + 1.5) / tex, (e.y as f32 + 1.5) / tex];
    let fill = |out: &mut Vec<Quad>, r: [f32; 4]| {
        out.push(Quad {
            pos: [r[0], r[1]],
            size: [r[2].max(1.0), r[3].max(1.0)],
            uv_min: uv,
            uv_max: uv,
            color,
            extra: [0.0, 0.0],
        });
    };
    let mut out = Vec::new();
    for &[x, y, w, h] in rects {
        if outline {
            fill(&mut out, [x, y, w, 1.0]);
            fill(&mut out, [x, y + h - 1.0, w, 1.0]);
            fill(&mut out, [x, y + 1.0, 1.0, (h - 2.0).max(0.0)]);
            fill(&mut out, [x + w - 1.0, y + 1.0, 1.0, (h - 2.0).max(0.0)]);
        } else {
            fill(&mut out, [x, y, w, h]);
        }
    }
    out
}

/// 今のページの論理座標を重ね描きする四角形を作る。`neko` を渡すと見えない猫の位置も示す。
/// 字の大きさと、大きさの違う字をそろえる位置は `font` で決める([`crate::build_quads`] と同じ)。
///
/// 塗りつぶしにはアトラスの 3×3 の塗りの点を使う(線と同じ)。アトラスが満杯なら空を返す。
pub fn build_overlay(
    layout: &Layout,
    reveal: &Reveal,
    neko: Option<&Neko>,
    font: &FontSet,
    atlas: &mut Atlas,
    place: &Placement,
    opts: &Overlay,
) -> Vec<Quad> {
    let base_px = font.base_px;
    let Some(e) = atlas.get_or_insert(('\u{0}'.into(), 0, false, Shape::Plain), || {
        (3, 3, 0, 0, vec![255; 9])
    }) else {
        return Vec::new();
    };
    let tex = atlas.size() as f32;
    let uv = [(e.x as f32 + 1.5) / tex, (e.y as f32 + 1.5) / tex];
    let mut out = Vec::new();
    let Some(page) = layout.pages.get(reveal.page()) else {
        return out;
    };
    let w = layout.writing;
    let base = base_px;
    let ascent = font.ascent(1.0);
    let page_index = reveal.page();
    let fill = |out: &mut Vec<Quad>, r: [f32; 4], color: [f32; 4]| {
        out.push(Quad {
            pos: [r[0], r[1]],
            size: [r[2].max(1.0), r[3].max(1.0)],
            uv_min: uv,
            uv_max: uv,
            color,
            extra: [0.0, 0.0],
        });
    };
    let frame = |out: &mut Vec<Quad>, r: [f32; 4], color: [f32; 4]| {
        let [x, y, rw, rh] = r;
        fill(out, [x, y, rw, 1.0], color);
        fill(out, [x, y + rh - 1.0, rw, 1.0], color);
        fill(out, [x, y + 1.0, 1.0, (rh - 2.0).max(0.0)], color);
        fill(
            out,
            [x + rw - 1.0, y + 1.0, 1.0, (rh - 2.0).max(0.0)],
            color,
        );
    };
    let rect = |line: usize, x0: f32, x1: f32, a0: f32, a1: f32| {
        place.to_screen(layout, page_index, base, line, x0, x1, a0, a1)
    };
    let ruby_room = |line: usize| place.ruby_room(page, line, base);

    // 行の長さ(上限があればそれ、無ければ一番長い行)
    let longest = page.lines.iter().map(|l| l.width).fold(0.0f32, f32::max);
    let span = opts.max_width.unwrap_or(longest).max(longest).max(1.0);
    let line_count = page.lines.len().max(opts.max_lines.unwrap_or(0)).max(1);

    // 行送りの枠とルビの場所
    for li in 0..line_count {
        let pitch = place.line_advance(page, li, base);
        frame(&mut out, rect(li, 0.0, span, 0.0, pitch), color::SLOT);
        fill(
            &mut out,
            rect(li, 0.0, span, 0.0, ruby_room(li)),
            color::RUBY_ROOM,
        );
    }

    // 目盛り: 1行目の始まりの辺に、全角1文字ごと(5 ごとに長く)
    let ticks = span.ceil() as usize;
    for i in 0..=ticks {
        let len = if i % 5 == 0 { 0.3 } else { 0.15 } * base;
        let x = i as f32;
        let r = rect(0, x, x, -len, 0.0);
        let r = match w {
            Writing::Vertical => [r[0], r[1], r[2], 1.0],
            _ => [r[0], r[1], 1.0, r[3]],
        };
        fill(&mut out, r, color::RULER);
    }

    // 行の長さの上限・行数の上限
    if let Some(mw) = opts.max_width {
        let r = rect(0, mw, mw, 0.0, place.line_offset(page, line_count, base));
        let r = match w {
            Writing::Vertical => [r[0], r[1], r[2], 2.0],
            _ => [r[0], r[1], 2.0, r[3]],
        };
        fill(&mut out, r, color::LIMIT);
    }
    if let Some(ml) = opts.max_lines {
        let r = rect(ml, 0.0, span, 0.0, 0.0);
        let r = match w {
            Writing::Vertical => [r[0] - 1.0, r[1], 2.0, r[3]],
            _ => [r[0], r[1] - 1.0, r[2], 2.0],
        };
        fill(&mut out, r, color::LIMIT);
    }

    // 字の枠(送り幅 × 文字の大きさ)。まだ出ていない文字は薄く
    let shown = reveal.visible_count();
    for (i, g) in page.glyphs.iter().enumerate() {
        let hidden = i >= shown;
        if hidden && !opts.show_hidden {
            continue;
        }
        let size = layout.styles.get(g.style).map_or(1.0, |s| s.size);
        let a0 = place.glyph_across(w, page, g.line, size, ascent, base);
        let r = rect(g.line, g.x, g.x + g.width, a0, a0 + size * base);
        let c = if hidden {
            color::CELL_HIDDEN
        } else {
            color::CELL
        };
        frame(&mut out, r, c);
    }

    // ルビ・圏点の範囲
    // 親文字の枠のすぐ外に、親文字の大きさに合わせたルビの場所(描くときと同じ)
    for r in &page.rubies {
        let size = page
            .glyphs
            .get(r.first_glyph)
            .and_then(|g| layout.styles.get(g.style))
            .map_or(1.0, |s| s.size);
        let a1 = place.glyph_across(w, page, r.line, size, ascent, base);
        let a0 = a1 - place.ruby_scale * size * base;
        frame(
            &mut out,
            rect(r.line, r.x, r.x + r.width, a0, a1),
            color::RUBY,
        );
    }

    // 入力待ちのカーソル(最後の文字の右端・縦書きでは下端)
    if let Some(c) = reveal.cursor(layout) {
        let a0 = place.glyph_across(w, page, c.line, 1.0, ascent, base);
        let r = rect(c.line, c.x, c.x, a0, a0 + base);
        let r = match w {
            Writing::Vertical => [r[0], r[1] - 1.0, r[2], 3.0],
            _ => [r[0] - 1.0, r[1], 3.0, r[3]],
        };
        fill(&mut out, r, color::CURSOR);
    }

    // 見えない猫の足元(行の始まりの辺の上の小さな印)
    if let Some(cat) = neko.and_then(|n| n.cat(layout, reveal)) {
        let half = 0.15;
        fill(
            &mut out,
            rect(
                cat.line,
                cat.x - half,
                cat.x + half,
                0.0,
                ruby_room(cat.line) * 0.5,
            ),
            color::CAT,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nekomoji::{parse, CellMetrics, LayoutOptions, Vars};

    fn layout(src: &str, opts: LayoutOptions) -> Layout {
        Layout::build(&parse(src).unwrap(), &CellMetrics, &opts, &Vars::new())
    }

    #[test]
    fn 範囲は本文の部分の画面の矩形になり_外寸の矩形はルビの場所と全行を囲む() {
        let l = layout(
            "あ[mark a]いう[/mark]\nえ",
            LayoutOptions::default().max_width(10.0),
        );
        let p = Placement::default()
            .origin([10.0, 20.0])
            .line_pitch(2.0)
            .ruby_scale(0.5);
        let rects = p.span_rects(&l, 0, 30.0, &l.mark_spans(0, "a"));
        // x は 1〜3 文字、行の上端から 15px(ルビの場所)の先に 30px
        assert_eq!(rects, vec![[40.0, 35.0, 60.0, 30.0]]);
        // 3 文字 × 2 行: 1 行目の上端から 2 行目の本文の下端(60 + 15 + 30)まで
        assert_eq!(p.extent_rect(&l, 0, 30.0), [10.0, 20.0, 90.0, 105.0]);
        let v = layout(
            "あいう\nえ",
            LayoutOptions::default().max_width(10.0).vertical(),
        );
        let pv = p.origin([500.0, 20.0]);
        // 縦書き: 1 行目の右端 500 から 2 行目の本文の左端(500 - 60 - 45)まで
        assert_eq!(pv.extent_rect(&v, 0, 30.0), [395.0, 20.0, 105.0, 90.0]);
    }

    #[test]
    fn 横書きの画面の座標と論理座標は行き来できる() {
        let p = Placement::default().origin([10.0, 20.0]).line_pitch(2.0);
        let h = layout("あいう\nえ", LayoutOptions::default());
        let r = p.to_screen(&h, 0, 30.0, 1, 2.0, 3.0, 0.0, 60.0);
        assert_eq!(r, [70.0, 80.0, 30.0, 60.0]);
        let l = p.to_logical(&h, 0, 30.0, [r[0], r[1]]);
        assert_eq!(l, [2.0, 1.0]);
    }

    #[test]
    fn 縦書きでは行が右から左へ_行の中は上から下へ進む() {
        let p = Placement::default().origin([500.0, 20.0]).line_pitch(2.0);
        let v = layout("あいう\nえ", LayoutOptions::default().vertical());
        let r = p.to_screen(&v, 0, 30.0, 1, 2.0, 3.0, 0.0, 60.0);
        // 1 行目の右端は 500 - 60 = 440、行の内側へ 60 px
        assert_eq!(r, [380.0, 80.0, 60.0, 30.0]);
        let l = p.to_logical(&v, 0, 30.0, [440.0, 80.0]);
        assert_eq!(l, [2.0, 1.0]);
    }

    #[test]
    fn 大きな字のある行は行送りとルビの場所が倍率の分だけ広がる() {
        // 1 行目に 2 倍の字。行送り 2.0 × 30px = 60px、2 倍の行は 120px
        let l = layout(
            "あ[size 2]い[/size]\nう\nえ",
            LayoutOptions::default().max_width(10.0),
        );
        let p = Placement::default()
            .origin([10.0, 20.0])
            .line_pitch(2.0)
            .ruby_scale(0.5);
        let page = &l.pages[0];
        assert_eq!(Placement::line_scale(page, 0), 2.0);
        assert_eq!(p.line_offset(page, 1, 30.0), 120.0);
        assert_eq!(p.line_offset(page, 2, 30.0), 180.0);
        // 行の外は倍率 1 で続ける
        assert_eq!(p.line_offset(page, 4, 30.0), 300.0);
        assert_eq!(p.ruby_room(page, 0, 30.0), 30.0);
        assert_eq!(p.ruby_room(page, 1, 30.0), 15.0);
        // 横書き: 1 倍の字は 2 倍の字とベースラインをそろえ、上端は (2 - 1) × 比 だけ下がる
        let across = p.glyph_across(Writing::Horizontal, page, 0, 1.0, 0.8, 30.0);
        assert!((across - (30.0 + 0.8 * 30.0)).abs() < 1e-4);
        assert_eq!(
            p.glyph_across(Writing::Horizontal, page, 0, 2.0, 0.8, 30.0),
            30.0
        );
        // 縦書きは中心をそろえる
        assert_eq!(
            p.glyph_across(Writing::Vertical, page, 0, 1.0, 0.8, 30.0),
            30.0 + 15.0
        );
        // 範囲の矩形は 2 行目の始まり(20 + 120)からルビの場所(15)の先
        let rects = p.span_rects(&l, 0, 30.0, &page.spans(2, 1));
        assert_eq!(rects, vec![[10.0, 155.0, 30.0, 30.0]]);
        // 画面の点から論理座標へ: 1 行目の半ばは 0.5、2 行目の始まりは 1.0
        assert_eq!(p.to_logical(&l, 0, 30.0, [10.0, 80.0]), [0.0, 0.5]);
        assert_eq!(p.to_logical(&l, 0, 30.0, [10.0, 140.0]), [0.0, 1.0]);
        assert_eq!(p.to_logical(&l, 0, 30.0, [10.0, 170.0]), [0.0, 1.5]);
        let r = p.to_screen(&l, 0, 30.0, 2, 0.0, 1.0, 0.0, 0.0);
        assert_eq!(p.to_logical(&l, 0, 30.0, [r[0], r[1]]), [0.0, 2.0]);
    }
}
