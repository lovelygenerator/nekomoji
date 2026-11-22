//! 禁則の判定と、和文・欧文・約物の分類。禁則の文字は [`super::line_class`] の分類の表から決める。

use super::line_class::{line_class, LineClass};

/// 行頭に置けない文字(終わり括弧・句読点・感嘆符・小書きの仮名・長音・ハイフン・後置の記号など)。
///
/// 分類は UAX #14 を JIS X 4051 の和文の約物に合わせて読み替えたもので、マークアップの仕様
/// (`docs/markup.md` の「組版の規則」)の表と同じ。[`Layout::build`](crate::Layout::build) の禁則もこの判定に従う。
pub fn is_line_start_forbidden(ch: char) -> bool {
    matches!(
        line_class(ch),
        LineClass::Close
            | LineClass::Exclam
            | LineClass::Infix
            | LineClass::NonStarter
            | LineClass::Hyphen
            | LineClass::Slash
            | LineClass::Postfix
    )
}

/// 行末に置けない文字(始め括弧・始め引用符・前置の記号)。
///
/// 分類は [`is_line_start_forbidden`] と同じ表による。
pub fn is_line_end_forbidden(ch: char) -> bool {
    matches!(line_class(ch), LineClass::Open | LineClass::Prefix)
}

/// ぶら下げてよい文字(行末からはみ出して置ける句読点)。
pub fn is_hangable(ch: char) -> bool {
    matches!(ch, '、' | '。' | '，' | '．' | ',' | '.')
}

/// 間で行を分けてはいけない組(`……` `――` `‥‥` `——`)。同じ分離禁止の文字が続くとき。
pub fn is_inseparable(a: char, b: char) -> bool {
    a == b && line_class(a) == LineClass::Inseparable
}

/// 後ろ側に半角のアキを持つ全角の約物(句読点・終わり括弧)。約物の詰めで後ろを詰める対象。
pub fn is_closing_punct(ch: char) -> bool {
    matches!(
        ch,
        '、' | '。'
            | '，'
            | '．'
            | '」'
            | '』'
            | '）'
            | '］'
            | '｝'
            | '〉'
            | '》'
            | '】'
            | '〕'
            | '〙'
            | '〗'
    )
}

/// 前側に半角のアキを持つ全角の始め括弧。
pub fn is_opening_bracket(ch: char) -> bool {
    matches!(
        ch,
        '「' | '『' | '（' | '［' | '｛' | '〈' | '《' | '【' | '〔' | '〘' | '〖'
    )
}

/// 和文の文字(漢字・仮名)か。和欧間のアキを入れる相手。
pub fn is_wa(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3041..=0x309F // ひらがな
            | 0x30A1..=0x30FA // カタカナ(・を除く)
            | 0x30FC..=0x30FF
            | 0x31F0..=0x31FF
            | 0x3005..=0x3007 // 々〆〇
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x3FFFD
    )
}

/// 欧文の文字(和欧間のアキを入れる相手。英単語として途中で分けない文字)。
/// 半角の英数字と、アクセント付きのラテン文字(`é` `ü` `ñ` など。U+00C0〜U+024F の字)。
pub fn is_latin(ch: char) -> bool {
    matches!(line_class(ch), LineClass::Alpha | LineClass::Numeric)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 文字表() {
        assert!(is_line_start_forbidden('。'));
        assert!(is_line_start_forbidden('っ'));
        assert!(!is_line_start_forbidden('あ'));
        assert!(is_line_end_forbidden('「'));
        assert!(is_hangable('、'));
        assert!(!is_hangable('」'));
        assert!(is_inseparable('…', '…'));
        assert!(!is_inseparable('…', 'あ'));
    }

    #[test]
    fn 約物と和文の分類() {
        assert!(is_closing_punct('。') && is_closing_punct('」'));
        assert!(!is_closing_punct('！') && !is_closing_punct('「'));
        assert!(is_opening_bracket('「') && !is_opening_bracket('」'));
        assert!(is_wa('あ') && is_wa('漢') && is_wa('ー') && is_wa('々'));
        assert!(!is_wa('・') && !is_wa('、') && !is_wa('a'));
        assert!(is_latin('a') && is_latin('9') && !is_latin('Ａ'));
        assert!(
            is_latin('é') && is_latin('ß') && is_latin('Ł') && !is_latin('×') && !is_latin('÷')
        );
    }
}
