//! 文字幅の計測。描画側が [`FontMetrics`] を実装して渡す。

use crate::text::vertical::{vertical_form, VerticalForm};

/// 線の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LineKind {
    /// 取り消し線(`[s]`)
    Strike,
    /// 下線(`[u]`)。縦書きでは行の右側に引く(傍線)
    Underline,
}

/// 横書きで、線の中心の位置(字の枠の上端から下向き)と太さの既定値(大きさ 1.0 に対する比)。
///
/// フォントが値を持たないときに使う。字の枠の上端からベースラインまでを 0.88 とした値。
pub fn default_line_metrics(kind: LineKind) -> (f32, f32) {
    match kind {
        LineKind::Strike => (0.5, 0.05),
        LineKind::Underline => (0.98, 0.05),
    }
}

/// 文字の送り幅を返す。描画側(フォントを持つ側)が実装する。
pub trait FontMetrics {
    /// 文字 `ch` を大きさ `size` で置いたときの送り幅
    fn advance(&self, ch: char, size: f32) -> f32;

    /// 縦書きで文字 `ch` を大きさ `size` で置いたときの行方向(下向き)の送り。
    ///
    /// 既定では、寝かせる文字([`VerticalForm::Sideways`])は横書きの送り幅、
    /// それ以外は全角1文字(`size`)とする。フォントの縦の送り(`vmtx`)を使うときは上書きする。
    fn vertical_advance(&self, ch: char, size: f32) -> f32 {
        match vertical_form(ch) {
            VerticalForm::Sideways => self.advance(ch, size),
            _ => size,
        }
    }

    /// 横書きで大きさ `size` の文字に引く線の、中心の位置(字の枠の上端から下向き)と太さ。
    ///
    /// 字の枠の上端は、描画側が文字を置くときの基準(行の上端)と同じにする。
    /// 既定は [`default_line_metrics`] の固定比率。フォントの strikeout・underline の値を
    /// 使うときは上書きする。縦書きでは使わない(行の中央と右側に引く)。
    fn line_metrics(&self, kind: LineKind, size: f32) -> (f32, f32) {
        let (y, t) = default_line_metrics(kind);
        (y * size, t * size)
    }
}

/// 描画側の送り幅を使える値にする。有限でない値と負の値は 0 とする。
///
/// [`FontMetrics`] は利用者が実装するので、NaN などが返っても組版・文字送り・猫モードが
/// 止まらないよう、読み取った所で直す。
pub(crate) fn sanitize_advance(v: f32) -> f32 {
    if v.is_finite() && v > 0.0 {
        v
    } else {
        0.0
    }
}

/// 端末向けの計測。全角を 1.0、半角を 0.5 とする(幅の単位は「全角1文字」)。
///
/// CLI・REPL・テストで使う。曖昧幅の文字(…・― など)は半角として扱う(縦書きでは全角)。
#[derive(Debug, Clone, Copy, Default)]
pub struct CellMetrics;

impl FontMetrics for CellMetrics {
    fn advance(&self, ch: char, size: f32) -> f32 {
        if is_wide(ch) {
            size
        } else {
            size * 0.5
        }
    }

    /// 縦書きでは端末の1段に全角1文字を置く。寝かせる半角の英数字(ASCII)だけ 0.5 とし、
    /// 曖昧幅の文字(…・― など)は 1.0 とする(縦書き用の形 ︙ ︱ は全角で表示されるため)。
    fn vertical_advance(&self, ch: char, size: f32) -> f32 {
        if ch.is_ascii() {
            size * 0.5
        } else {
            size
        }
    }
}

/// フォントに字形が無いときに代わりに使ってよい文字(先にあるほど近い)。
///
/// 日本語フォントによって収録がまちまちな記号だけを持つ。描画側はフォントに字形が無いとき
/// この順に探し、`nekomoji subset` は代わりの文字もサブセットに残す。
pub fn fallback_chars(ch: char) -> &'static [char] {
    match ch {
        '―' => &['—', '─', '━'],
        '—' => &['―', '─', '━'],
        '─' => &['―', '—', '━'],
        '﹅' | '﹆' => &['、'],
        '〜' => &['～'],
        '～' => &['〜'],
        '－' => &['−', '-'],
        '−' => &['－', '-'],
        '‥' => &['…'],
        _ => &[],
    }
}

/// 東アジアの全角文字か(端末で2桁を占めるか)。主な範囲だけを見る簡易版。
pub fn is_wide(ch: char) -> bool {
    let c = ch as u32;
    matches!(
        c,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0xFE10..=0xFE19
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1F64F
            | 0x1F900..=0x1F9FF
            | 0x20000..=0x3FFFD
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 字形の代わりは収録がまちまちな記号だけ持つ() {
        assert_eq!(fallback_chars('―')[0], '—');
        assert_eq!(fallback_chars('﹅'), &['、']);
        assert!(fallback_chars('あ').is_empty());
    }

    #[test]
    fn 全角と半角の幅() {
        assert!(is_wide('あ'));
        assert!(is_wide('漢'));
        assert!(is_wide('、'));
        assert!(is_wide('Ａ'));
        assert!(!is_wide('a'));
        assert!(!is_wide('ｱ'));
        assert_eq!(CellMetrics.advance('あ', 1.0), 1.0);
        assert_eq!(CellMetrics.advance('a', 1.0), 0.5);
    }

    #[test]
    fn 縦書きの送りは立てる文字が全角_寝かせる文字は横の送り幅() {
        assert_eq!(CellMetrics.vertical_advance('あ', 1.0), 1.0);
        assert_eq!(CellMetrics.vertical_advance('。', 1.0), 1.0);
        assert_eq!(CellMetrics.vertical_advance('a', 1.0), 0.5);
        assert_eq!(CellMetrics.vertical_advance('ー', 2.0), 2.0);
        assert_eq!(CellMetrics.vertical_advance('…', 1.0), 1.0);
    }
}
