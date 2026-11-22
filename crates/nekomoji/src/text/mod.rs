//! 文字そのものの性質: 書記素クラスタ・禁則の文字表・縦書きでの置き方・文字幅。
//!
//! レイアウト([`crate::Layout`])と描画側の両方から使う、文字1つ(または書記素クラスタ1つ)ごとの判定と表。
//!
//! - `grapheme`: 書記素クラスタ(見た目の1文字)
//! - `line_class`: 行分割での文字の分類(UAX #14 を JIS X 4051 に合わせて読み替えた表)
//! - `kinsoku`: 禁則の判定と、和文・欧文・約物の分類
//! - `vertical`: 書字方向と、縦書きで文字をどう置くか
//! - `metrics`: 文字幅([`FontMetrics`] と端末用の [`CellMetrics`])と字形の代わり

pub(crate) mod grapheme;
pub(crate) mod kinsoku;
pub(crate) mod line_class;
pub(crate) mod metrics;
pub(crate) mod vertical;

pub use grapheme::graphemes;
pub use kinsoku::{
    is_closing_punct, is_hangable, is_inseparable, is_latin, is_line_end_forbidden,
    is_line_start_forbidden, is_opening_bracket, is_wa,
};
pub use metrics::{
    default_line_metrics, fallback_chars, is_wide, CellMetrics, FontMetrics, LineKind,
};
pub use vertical::{vertical_form, VerticalForm, Writing};
