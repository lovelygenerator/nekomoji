//! nekomoji: ゲーム向けの日本語テキスト組版と文字送り。
//!
//! 描画はしない。「どの文字を、どこに、いつ、どう見せるか」を計算し、
//! 描画側(CLI・ゲームエンジン)に渡す。
//!
//! 流れは3段:
//! 1. [`parse`]: マークアップを [`Script`] にする(未知のタグは行・桁つきのエラー。すべてのエラーを集めるなら [`parse_all`])
//! 2. [`Layout::build`]: 行分割・禁則・ルビ配置をして [`Layout`] にする
//! 3. [`Reveal`]: 時間を進めて、表示する文字数と [`TextEvent`] を返す
//!
//! 設定([`LayoutOptions`]・[`RevealOptions`]・[`NekoMode`])は `Default` から builder の
//! メソッドで組み立てる(`LayoutOptions::default().max_width(18.0).vertical()`)。
//! 設定と出力の型は `#[non_exhaustive]` で、項目が増えても利用者のコードは壊れない。
//!
//! [`LayoutOptions::writing`] を [`Writing::Vertical`] にすると縦書きで組む。
//! 配置は横書きと同じく「行頭からの位置 `x`」と「行の番号」で表し、
//! 縦書きでは `x` が行の中を下へ、行が右から左へ並ぶ。画面への写し方は描画側が決める。
//!
//! 猫モード(見えない猫が文字を猫パンチで飛ばす・押して落とす・爆散させる。飛んだ文字は戻る)は [`Neko`] が、
//! [`Layout`] と [`Reveal`] の状態から時刻の関数として計算する。猫の状態や飛ばされた文字の型は [`neko`] にある。
//!
//! 禁則や縦書きの置き方など、文字1つごとの判定は [`chars`] にある。
//!
//! ```
//! use nekomoji::{parse, CellMetrics, Layout, LayoutOptions, Reveal, RevealOptions, Vars};
//!
//! let script = parse("[ruby めいきゅう]迷宮[/ruby]の奥で、何かが動いた。").unwrap();
//! let opts = LayoutOptions::default().max_width(12.0).max_lines(3);
//! let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
//! let mut reveal = Reveal::new(&layout, RevealOptions::default());
//! let _events = reveal.advance(0.5);
//! assert!(reveal.visible_count() > 0);
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// 設定の構造体に builder の設定メソッドを足す(`self` を受けて `Self` を返す)。
macro_rules! setters {
    ($($(#[$meta:meta])* $name:ident: $ty:ty;)*) => {
        $(
            $(#[$meta])*
            #[must_use]
            pub fn $name(mut self, value: $ty) -> Self {
                self.$name = value;
                self
            }
        )*
    };
}

// README の例が通ることをテストで確かめる(crates.io の説明に出るので)
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

mod layout;
mod markup;
pub mod neko;
mod reveal;
mod text;

/// 文字1つ(または書記素クラスタ1つ)ごとの判定と表。
///
/// [`Layout::build`] の禁則・和欧間のアキ・英単語の判定はこの関数と同じ表に従う。
/// 描画側(縦書きの字形の補正・フォントに無い記号の代わり・端末の桁)や、
/// 組版の結果を確かめるテストから使う。
pub mod chars {
    pub use crate::text::{
        fallback_chars, graphemes, is_closing_punct, is_hangable, is_inseparable, is_latin,
        is_line_end_forbidden, is_line_start_forbidden, is_opening_bracket, is_wa, is_wide,
        vertical_form, VerticalForm,
    };
}

pub use layout::{
    Cue, Decoration, Extent, Glyph, Indent, Kinsoku, Layout, LayoutOptions, Line, MarkRun, Page,
    RubyRun, Span, TcyPlace, Vars, ICON_CHAR,
};
pub use markup::{
    parse, parse_all, Align, Baseline, Effect, ErrorKind, GlowKind, Item, Outline, ParseError,
    Script, Shadow, Style, TextEvent, EMPHASIS_MARK,
};
pub use neko::{Neko, NekoMode};
pub use reveal::{
    AutoWait, Blip, Cursor, DecorationInstance, GlyphInstance, Motion, ParseStateError, Reveal,
    RevealOptions, RevealState, Step, StepKind, WaitKind,
};
pub use text::{default_line_metrics, CellMetrics, FontMetrics, LineKind, Writing};
