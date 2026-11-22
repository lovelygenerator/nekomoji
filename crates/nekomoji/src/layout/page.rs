//! レイアウトの結果のページ・行・グリフ・ルビ・線。

use std::ops::Range;

use crate::markup::{Align, TextEvent};
use crate::text::metrics::LineKind;

/// 縦中横の1文字の置き方。縦書きでだけ付く。
///
/// 縦中横の文字はどれも同じ `x`(行の中の位置)で、全角1文字分の字の枠に横に並べる。
/// 描画側は文字を立てたまま(回さず、縦書き用の字形にも置き換えず)、横方向だけ `scale` 倍に縮め、
/// 字の枠の左端から `offset`(全角1文字を 1.0 とする単位、字の大きさに比例)の位置を描き始めにする。
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct TcyPlace {
    /// 字の枠の左端から、この文字の描き始め(ペンの位置)まで
    pub offset: f32,
    /// 横方向の縮め率(1.0 なら縮めない)。並べた幅が全角1文字を超えるときだけ縮める
    pub scale: f32,
}

/// 配置済みの1文字。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Glyph {
    /// 文字(書記素クラスタの先頭の文字)。幅・禁則・文字送りの間はこの文字で決める
    pub ch: char,
    /// 書記素クラスタの、`ch` に続く文字(異体字セレクタ・結合文字・ZWJ でつながる絵文字など)。
    /// たいていは空。描画側は `ch` とあわせて1文字として描く([`Glyph::text`])
    pub tail: String,
    /// 行頭からの位置(縦書きでは行の上端からの位置)
    pub x: f32,
    /// 送り幅(縦書きでは行方向の送り)
    pub width: f32,
    /// 字形を描く位置の、`x` からのずれ(行の中の向き。たいていは 0)。
    ///
    /// 始め括弧の連続(`「『`)を詰めたとき、後ろの括弧は字の枠(`x`〜`x + width`)を半角にし、
    /// 字形を半角だけ前へずらして描く(負の値。[`LayoutOptions::punct_squeeze`](super::LayoutOptions::punct_squeeze))。
    /// 字面は字の枠に収まる。[`crate::GlyphInstance::x`] はずれを足した値
    pub shift: f32,
    /// 行の番号(ページ内、0 始まり)
    pub line: usize,
    /// スタイルの添字([`Layout::styles`](super::Layout::styles))
    pub style: usize,
    /// 縦中横の置き方(縦書きの `[tcy]` の文字だけ)
    pub tcy: Option<TcyPlace>,
    /// 元の文章(`parse` に渡した文字列)のバイト範囲。
    /// 変数から来た文字は `[var name]` 全体を指す。エスケープ(`[[`)は2バイト分
    pub source: Range<usize>,
    /// 変数から来た文字なら、その変数の番号([`Layout::vars`](super::Layout::vars) の添字)
    pub var: Option<usize>,
    /// 文中のアイコン(`[icon id]`)なら、その ID。`ch` は [`ICON_CHAR`]。
    /// 絵は描画側が描き、字の枠(`x`〜`x + width`、行の高さは字の大きさ)に収める
    pub icon: Option<String>,
}

/// 文中のアイコン(`[icon]`)のグリフの文字(U+FFFC OBJECT REPLACEMENT CHARACTER)。
/// バックログ([`Page::text`])にもこの文字で出る
pub const ICON_CHAR: char = '\u{FFFC}';

impl Glyph {
    /// 見た目の1文字(`ch` と `tail` をつないだ書記素クラスタ)。
    pub fn text(&self) -> String {
        let mut s = String::with_capacity(self.ch.len_utf8() + self.tail.len());
        s.push(self.ch);
        s.push_str(&self.tail);
        s
    }
}

/// 配置済みのルビ。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RubyRun {
    /// 読み
    pub text: String,
    /// 親文字の範囲の左端
    pub x: f32,
    /// 親文字の範囲の幅
    pub width: f32,
    /// 行の番号
    pub line: usize,
    /// 親文字の最初のグリフの添字(ページ内)
    pub first_glyph: usize,
    /// 親文字のグリフ数
    pub glyph_count: usize,
    /// 圏点(`[em]`)か。圏点は1文字ごとに1つで、`text` は記号1文字。
    /// 描画はルビと同じでよい(横書きでは上、縦書きでは右)
    pub emphasis: bool,
}

/// 1行。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Line {
    /// 最初のグリフの添字(ページ内)
    pub first_glyph: usize,
    /// グリフ数
    pub glyph_count: usize,
    /// 行頭から行末までの幅(字下げとぶら下げた句読点を含む)
    pub width: f32,
    /// 行頭の字下げ(最初のグリフの手前のアキ)。中央揃え・行末揃えの行では、揃えのために
    /// 空けた分
    pub indent: f32,
    /// 行の揃え
    pub align: Align,
    /// 行で最も大きい文字の倍率
    pub size: f32,
    /// 段落(改行・`[br]`・`[p]` で区切られた文)の1行目か。折り返しや自動改ページで
    /// 続いた行では `false`
    pub paragraph_start: bool,
}

/// 取り消し線・下線の1本。行ごと・色と大きさが同じ文字の並びごとに分ける。
///
/// 位置は行の中の範囲 `x0`〜`x1`(グリフと同じ論理座標)と、線の中心 `y`。
/// `y` は横書きでは字の枠の上端から下向き、縦書きでは字の枠の右端から左向き(負なら右へはみ出す)。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Decoration {
    /// 線の種類
    pub kind: LineKind,
    /// 行の番号
    pub line: usize,
    /// 始まりの位置
    pub x0: f32,
    /// 終わりの位置
    pub x1: f32,
    /// 線の中心
    pub y: f32,
    /// 太さ
    pub thickness: f32,
    /// 最初のグリフの添字(ページ内)
    pub first_glyph: usize,
    /// グリフ数
    pub glyph_count: usize,
    /// 色を決めるスタイルの添字
    pub style: usize,
}

/// 文字送りの途中の合図。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Cue {
    /// 待機(秒)
    Wait(f32),
    /// ページの途中で入力を待つ(`[l]`)
    Click,
    /// 外へ知らせる出来事
    Event(TextEvent),
}

/// 1ページ(`[p]` で区切られる)。
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct Page {
    /// 表示順のグリフ
    pub glyphs: Vec<Glyph>,
    /// 行
    pub lines: Vec<Line>,
    /// ルビ
    pub rubies: Vec<RubyRun>,
    /// 取り消し線・下線
    pub decorations: Vec<Decoration>,
    /// 合図。`(n, cue)` は「n 文字を表示した後」に起きる
    pub cues: Vec<(usize, Cue)>,
}

/// ルビの短い書き方 `｜親文字《よみ》` で書いて読み戻せるか(markup.md の「ルビの短い書き方」と同じ条件)。
fn short_ruby_fits(base: &str, reading: &str) -> bool {
    let n = base.chars().count();
    (1..=32).contains(&n)
        && !base
            .chars()
            .any(|c| c.is_whitespace() || "|｜《》[]".contains(c))
        && !reading.trim().is_empty()
        && reading.trim() == reading
        && !reading
            .chars()
            .any(|c| "《》[]".contains(c) || (c.is_whitespace() && c != ' ' && c != '\u{3000}'))
}

impl Page {
    /// バックログ用のテキスト。行ごとに改行し、ルビは `｜親文字《よみ》` で書く。
    ///
    /// 見た目の演出(猫が飛ばした文字など)には影響されない。
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (ln, line) in self.lines.iter().enumerate() {
            if ln > 0 {
                out.push('\n');
            }
            let end = line.first_glyph + line.glyph_count;
            let mut i = line.first_glyph;
            while i < end {
                if let Some(r) = self
                    .rubies
                    .iter()
                    .find(|r| r.first_glyph == i && !r.emphasis)
                {
                    let stop = (i + r.glyph_count).min(end);
                    let base: String = self.glyphs[i..stop]
                        .iter()
                        .flat_map(|g| std::iter::once(g.ch).chain(g.tail.chars()))
                        .collect();
                    if short_ruby_fits(&base, &r.text) {
                        out.push('｜');
                        out.push_str(&base);
                        out.push('《');
                        out.push_str(&r.text);
                        out.push('》');
                    } else {
                        // 短い書き方で読み戻せないもの(親文字が長い・空白や括弧を含む、読みが `《》` を含むなど)
                        out.push_str("[ruby ");
                        out.push_str(&r.text);
                        out.push(']');
                        for c in base.chars() {
                            out.push(c);
                            if c == '[' || c == ']' {
                                out.push(c);
                            }
                        }
                        out.push_str("[/ruby]");
                    }
                    i = stop.max(i + 1);
                } else {
                    out.push(self.glyphs[i].ch);
                    out.push_str(&self.glyphs[i].tail);
                    i += 1;
                }
            }
        }
        out
    }

    /// 読み上げ用のテキスト。ルビの親文字を読みに置き換え、圏点・アイコンは出さない。
    ///
    /// 折り返しや自動改ページで分かれた行はつなぎ、段落([`Line::paragraph_start`])の
    /// 区切りだけ改行にする。読み上げ(TTS)や、読みでの検索に使う。
    pub fn speech_text(&self) -> String {
        let mut out = String::new();
        for (ln, line) in self.lines.iter().enumerate() {
            if ln > 0 && line.paragraph_start {
                out.push('\n');
            }
            let end = line.first_glyph + line.glyph_count;
            let mut i = line.first_glyph;
            while i < end {
                if let Some(r) = self
                    .rubies
                    .iter()
                    .find(|r| r.first_glyph == i && !r.emphasis)
                {
                    out.push_str(&r.text);
                    i = (i + r.glyph_count).min(end).max(i + 1);
                } else {
                    let g = &self.glyphs[i];
                    if g.icon.is_none() {
                        out.push(g.ch);
                        out.push_str(&g.tail);
                    }
                    i += 1;
                }
            }
        }
        out
    }
}
