//! 端末の桁の数え方(全角 2 桁・半角 1 桁)と、文字の飾り。

use nekomoji::chars::{graphemes, is_wide};
use nekomoji::{Style, ICON_CHAR};

pub(super) fn cells(ch: char) -> usize {
    if is_combining(ch) {
        0
    } else if is_wide(ch) {
        2
    } else {
        1
    }
}

/// 結合文字(取り消し線 U+0336・下線 U+0332 など)。端末では幅を取らない。
fn is_combining(ch: char) -> bool {
    matches!(ch as u32, 0x0300..=0x036F)
}

/// 端末でアイコン(`[icon]`)の代わりに出す文字(下駄記号。組版で代わりの字を示す記号)
pub const ICON_MARK: char = '〓';

/// 端末での1文字の表記。取り消し線は U+0336、下線は U+0332 を後ろに付ける。
/// 1文字(書記素クラスタ。`ch` と続きの `tail`)に、線の結合文字を付ける。
pub(super) fn decorated(ch: char, tail: &str, style: Option<&Style>) -> String {
    let ch = if ch == ICON_CHAR { ICON_MARK } else { ch };
    let mut s = String::from(ch);
    s.push_str(tail);
    if let Some(st) = style {
        if st.strike {
            s.push('\u{0336}');
        }
        if st.underline {
            s.push('\u{0332}');
        }
    }
    s
}

/// 端末での幅。書記素クラスタごとに先頭の文字の幅で数え、続きの文字(異体字セレクタ・結合文字・
/// ZWJ でつながる絵文字)は幅に数えない。
pub(super) fn str_cells(s: &str) -> usize {
    graphemes(s)
        .into_iter()
        .filter_map(|g| g.chars().next())
        .map(cells)
        .sum()
}

pub(super) fn to_col(x: f32) -> usize {
    (x * 2.0).round().max(0.0) as usize
}

pub(super) fn put(row: &mut String, col: &mut usize, target: usize, text: &str) {
    while *col < target {
        row.push(' ');
        *col += 1;
    }
    row.push_str(text);
    *col += str_cells(text);
}
