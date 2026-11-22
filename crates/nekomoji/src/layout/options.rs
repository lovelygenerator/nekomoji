//! レイアウトの設定。

use std::collections::BTreeMap;

use crate::markup::Align;
use crate::text::vertical::Writing;

/// 変数の値(`[var name]` を展開する)。
pub type Vars = BTreeMap<String, String>;

/// 禁則の扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Kinsoku {
    /// 行頭禁則・行末禁則・分離禁止を守る(追い出し)
    #[default]
    Standard,
    /// 禁則なし(幅で機械的に折り返す)
    Off,
}

/// レイアウトの設定。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct LayoutOptions {
    /// 書字方向。縦書きでは `max_width` は1行の高さ(行の中の長さ)になる
    pub writing: Writing,
    /// 1行の最大幅(単位は [`FontMetrics`](crate::FontMetrics) の単位)
    pub max_width: f32,
    /// 禁則
    pub kinsoku: Kinsoku,
    /// 句読点のぶら下げ(行末からはみ出して置く)
    pub hanging: bool,
    /// ルビの大きさ(親文字に対する倍率)
    pub ruby_scale: f32,
    /// ルビが親文字の外へはみ出してよい幅の合計(左右に半分ずつ)。
    /// これを超えて長いルビのときだけ、親文字の間を広げる
    pub ruby_overhang: f32,
    /// 1ページの最大行数。`Some(n)` なら n 行を超えたところで自動で改ページする
    /// (`[p]` と同じく入力を待つ)。`None` なら `[p]` でだけ改ページする
    pub max_lines: Option<usize>,
    /// 約物の詰め。句読点・終わり括弧の後ろに約物(句読点・括弧)が続くとき、
    /// 前の約物の後ろの半角のアキを詰める(`」「` `。」` `、「` など)
    pub punct_squeeze: bool,
    /// 和欧間のアキ。和文(漢字・仮名)と半角英数の間に入れる幅(全角に対する比。0 で入れない)
    pub latin_space: f32,
    /// 両端揃え。段落の最後の行を除き、幅に満たない行の塊の間を均等に広げて行末を揃える
    pub justify: bool,
    /// 字下げ。段落の頭の字下げ、括弧で始まる段落(会話)の扱い、2行目以降の字下げ
    pub indent: Indent,
    /// 揃えの既定。`[align]` の無い段落に使う。既定は行頭揃え
    pub align: Align,
}

/// 字下げ。単位は [`LayoutOptions::max_width`] と同じ(全角1文字 = 1.0)。負の数は 0 とみなす。
///
/// 段落(ソースの改行・`[br]`・`[p]` で区切られた文)を3種類に分けて字下げする:
/// - **地の文**: 括弧で始まらない段落。1行目が `first`、2行目以降が `rest`。
///   全角の空白(`U+3000`)で始まる段落は手で字下げしたものとみなし、1行目に `first` を足さない
/// - **会話**: 始め括弧(`「` `『` `（` など)で始まる段落。1行目が `bracket_first`
///   (0 で天付き、0.5 で半角下げ、1 で全角下げ)、2行目以降が `bracket_rest`(ぶら下げインデント)
/// - **会話の続き**: 前の段落で開いた括弧が閉じないまま改行・改ページした段落。すべての行が `bracket_rest`
///
/// 字下げの分だけ行に入る文字は減る。既定はすべて 0(字下げしない)。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub struct Indent {
    /// 地の文の1行目
    pub first: f32,
    /// 地の文の2行目以降
    pub rest: f32,
    /// 会話(括弧で始まる段落)の1行目。0 で天付き、0.5 で半角下げ、1 で全角下げ
    pub bracket_first: f32,
    /// 会話の2行目以降と、会話の続きの段落(括弧の後ろに揃えるなら 1)
    pub bracket_rest: f32,
}

impl Indent {
    setters! {
        /// 地の文の1行目([`Indent::first`])
        first: f32;
        /// 地の文の2行目以降([`Indent::rest`])
        rest: f32;
        /// 会話の1行目([`Indent::bracket_first`])
        bracket_first: f32;
        /// 会話の2行目以降([`Indent::bracket_rest`])
        bracket_rest: f32;
    }

    /// 小説の組み方: 地の文は1字下げ、会話は天付き(2行目以降も行頭から)。
    pub fn novel() -> Self {
        Self::default().first(1.0)
    }

    /// ゲームの会話によくある組み方: 地の文は字下げせず、会話は天付きで、2行目以降を括弧の後ろ
    /// (全角1文字分)に揃える。
    pub fn dialogue() -> Self {
        Self::default().bracket_rest(1.0)
    }
}

impl LayoutOptions {
    setters! {
        /// 書字方向([`LayoutOptions::writing`])
        writing: Writing;
        /// 1行の最大幅([`LayoutOptions::max_width`])
        max_width: f32;
        /// 禁則([`LayoutOptions::kinsoku`])
        kinsoku: Kinsoku;
        /// 句読点のぶら下げ([`LayoutOptions::hanging`])
        hanging: bool;
        /// ルビの大きさ([`LayoutOptions::ruby_scale`])
        ruby_scale: f32;
        /// ルビのはみ出し([`LayoutOptions::ruby_overhang`])
        ruby_overhang: f32;
        /// 約物の詰め([`LayoutOptions::punct_squeeze`])
        punct_squeeze: bool;
        /// 和欧間のアキ([`LayoutOptions::latin_space`])
        latin_space: f32;
        /// 両端揃え([`LayoutOptions::justify`])
        justify: bool;
        /// 字下げ([`LayoutOptions::indent`])
        indent: Indent;
        /// 揃えの既定([`LayoutOptions::align`])
        align: Align;
    }

    /// 縦書きにする(`writing(Writing::Vertical)` と同じ)。
    #[must_use]
    pub fn vertical(self) -> Self {
        self.writing(Writing::Vertical)
    }

    /// 1ページの最大行数([`LayoutOptions::max_lines`])。`max_lines(3)` か `max_lines(None)`。
    #[must_use]
    pub fn max_lines(mut self, value: impl Into<Option<usize>>) -> Self {
        self.max_lines = value.into();
        self
    }
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            writing: Writing::Horizontal,
            max_width: 20.0,
            kinsoku: Kinsoku::Standard,
            hanging: true,
            ruby_scale: 0.5,
            ruby_overhang: 0.5,
            max_lines: None,
            punct_squeeze: true,
            latin_space: 0.25,
            justify: false,
            indent: Indent::default(),
            align: Align::Start,
        }
    }
}
