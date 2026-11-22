//! レイアウトを端末の文字列にする。
//!
//! - `settings`: CLI・REPL の設定([`Settings`])と、オプションの値の読み書き
//! - `cells`: 端末の桁の数え方と、文字の飾り(上付き・線など)
//! - `cat`: 猫モードの見え方([`CatView`]。飛んだ文字を隠し、肉球を出す)
//! - `horizontal`: 横書きのページを行の文字列にする
//! - `vertical`: 縦書きのページを、右から左へ並ぶ列にして行の文字列にする
//! - `log`: 文字送りの時間割と出来事を時刻順の文字列にする
//! - `check`: `check` の出力(人が読む形と JSON)

mod cat;
mod cells;
mod check;
mod horizontal;
mod log;
mod settings;
#[cfg(test)]
mod tests;
mod vertical;

pub use self::log::log;
pub use cat::CatView;
pub use cells::ICON_MARK;
pub use check::{check_json, check_text, Checked};
pub use horizontal::{page_rows, page_rows_with};
pub use settings::{
    align_name, apply_bracket_indent, apply_indent, motion_name, parse_align, parse_blip,
    parse_indent_pair, parse_motion, Settings,
};
pub use vertical::{terminal_vertical_char, vertical_rows};

use nekomoji::ParseError;

use cells::str_cells;

/// 解析エラーをすべて、位置の順に [`format_error`] の形で並べる。2件以上なら最後に件数を足す。
pub fn format_errors(src: &str, errors: &[ParseError]) -> String {
    let mut out: Vec<String> = errors.iter().map(|e| format_error(src, e)).collect();
    if errors.len() > 1 {
        out.push(format!("(エラー {} 件)", errors.len()));
    }
    out.join("\n")
}

/// 解析エラーを、該当行と ^ 付きで表示する。
pub fn format_error(src: &str, e: &ParseError) -> String {
    let line = src.lines().nth(e.line.saturating_sub(1)).unwrap_or("");
    let prefix: String = line.chars().take(e.col.saturating_sub(1)).collect();
    format!(
        "エラー {e}\n  {line}\n  {}^",
        " ".repeat(str_cells(&prefix))
    )
}
