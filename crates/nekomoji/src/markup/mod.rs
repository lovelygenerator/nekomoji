//! マークアップの解析。書式は docs/markup.md を正とする。
//!
//! - `style`: 文字の見た目([`Style`] と縁取り・影・揃え・エフェクト)
//! - `script`: 解析の結果([`Script`]・[`Item`])と外へ知らせる出来事([`TextEvent`])
//! - `error`: 解析エラー(行・桁つき)
//! - `parser`: 文字を読んでタグを解釈する([`parse`])

mod error;
mod parser;
mod script;
mod style;
#[cfg(test)]
mod tests;

pub use error::{ErrorKind, ParseError};
pub use parser::{parse, parse_all};
pub use script::{Item, Script, TextEvent};
pub use style::{Align, Baseline, Effect, GlowKind, Outline, Shadow, Style, EMPHASIS_MARK};
