//! 範囲を行ごとの矩形にする・当たり判定・ページの外寸・名前付きの範囲。

use super::page::Page;

/// 1行の中の範囲(論理座標)。範囲や名前付きの範囲を行ごとの矩形にしたもの。
///
/// 矩形は行の中の `x0`〜`x1`(グリフと同じ論理座標)と、行の番号 `line` で表す。
/// 行と直交する方向(行の高さ・ルビの分)をどこまで含めるかは描画側が決める。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Span {
    /// 行の番号
    pub line: usize,
    /// 始まりの位置(最初のグリフの `x`)
    pub x0: f32,
    /// 終わりの位置(最後のグリフの `x + width`)
    pub x1: f32,
    /// 最初のグリフの添字(ページ内)
    pub first_glyph: usize,
    /// グリフ数
    pub glyph_count: usize,
}

/// ページの外寸(論理座標)。枠の大きさを文字に合わせるときに使う。
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Extent {
    /// 最も長い行の幅(ぶら下げた句読点を含む。全角1文字を 1.0 とする単位)。
    /// ルビが親文字の外へはみ出す分(最大 [`LayoutOptions::ruby_overhang`](super::LayoutOptions::ruby_overhang))は含まない
    pub width: f32,
    /// 行数
    pub lines: usize,
}

/// 名前付きの範囲(`[mark id]`)の、ページの中のひと続きの文字。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MarkRun {
    /// 範囲の ID
    pub id: String,
    /// 最初のグリフの添字(ページ内)
    pub first_glyph: usize,
    /// グリフ数
    pub glyph_count: usize,
}

impl Page {
    /// グリフの範囲 `first..first + count` を、行ごとの矩形にする。
    pub fn spans(&self, first: usize, count: usize) -> Vec<Span> {
        let end = first.saturating_add(count).min(self.glyphs.len());
        let mut out: Vec<Span> = Vec::new();
        for (i, g) in self.glyphs.iter().enumerate().take(end).skip(first) {
            match out.last_mut() {
                Some(s) if s.line == g.line && s.first_glyph + s.glyph_count == i => {
                    s.x0 = s.x0.min(g.x);
                    s.x1 = s.x1.max(g.x + g.width);
                    s.glyph_count += 1;
                }
                _ => out.push(Span {
                    line: g.line,
                    x0: g.x,
                    x1: g.x + g.width,
                    first_glyph: i,
                    glyph_count: 1,
                }),
            }
        }
        out
    }

    /// 論理座標の点 `(x, line)` にあるグリフの添字。
    ///
    /// `line` は小数でよく(`0.0`〜`1.0` が 0 行目)、`x` は行の中の位置。
    /// 字の枠(`x`〜`x + width`)の中にあるグリフを返す。和欧間のアキなど、文字の間では `None`。
    /// 縦中横は塊の最初の文字を返す。
    pub fn hit_test(&self, x: f32, line: f32) -> Option<usize> {
        if line.is_nan() || line < 0.0 || !x.is_finite() {
            return None;
        }
        let ln = self.lines.get(line as usize)?;
        let range = ln.first_glyph..ln.first_glyph + ln.glyph_count;
        range.into_iter().find(|&i| {
            self.glyphs
                .get(i)
                .is_some_and(|g| x >= g.x && x < g.x + g.width)
        })
    }

    /// 元の文章のバイト位置 `byte` から作られたグリフの添字。
    /// 変数から来た文字は `[var name]` 全体が対応し、その最初のグリフを返す。
    pub fn glyph_at_source(&self, byte: usize) -> Option<usize> {
        self.glyphs.iter().position(|g| g.source.contains(&byte))
    }

    /// ページの外寸。
    pub fn extent(&self) -> Extent {
        Extent {
            width: self.lines.iter().map(|l| l.width).fold(0.0, f32::max),
            lines: self.lines.len(),
        }
    }
}
