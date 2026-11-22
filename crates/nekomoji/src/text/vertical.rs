//! 縦書きで文字をどう置くかの分類。
//!
//! コアは「どの文字が回転・ずらしの対象か」だけを持つ。実際の字形の置き換え
//! (フォントの `vert`/`vrt2`)と、置き換えが無いときの回転角・ずらし量は描画側が決める。

use crate::text::metrics::is_wide;

/// 書字方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Writing {
    /// 横書き(行は上から下へ並ぶ)
    #[default]
    Horizontal,
    /// 縦書き(行は右から左へ並び、行の中は上から下へ進む)
    Vertical,
}

/// 縦書きでの文字の置き方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VerticalForm {
    /// そのまま立てて置く(漢字・仮名・全角英数など)
    Upright,
    /// 時計回りに 90° 寝かせて置く(半角英数・長音・ダッシュ・リーダー・括弧など)。
    /// 行方向の送りは横書きの送り幅になる
    Sideways,
    /// 句読点(、。,.)。字面の右上へずらす
    Punctuation,
    /// 小書きの仮名(ぁ・っ・ャ など)。字面の右上へ少しずらす
    SmallKana,
}

/// 縦書きでの文字の置き方を返す。
///
/// フォントが縦書き用の字形(`vert`)を持つときは、描画側はそちらを優先し、
/// この分類は置き換えが無い文字の補正にだけ使う。
pub fn vertical_form(ch: char) -> VerticalForm {
    match ch {
        '、' | '。' | '，' | '．' | '､' | '｡' => VerticalForm::Punctuation,
        'ぁ' | 'ぃ' | 'ぅ' | 'ぇ' | 'ぉ' | 'っ' | 'ゃ' | 'ゅ' | 'ょ' | 'ゎ' | 'ゕ' | 'ゖ'
        | 'ァ' | 'ィ' | 'ゥ' | 'ェ' | 'ォ' | 'ッ' | 'ャ' | 'ュ' | 'ョ' | 'ヮ' | 'ヵ' | 'ヶ' => {
            VerticalForm::SmallKana
        }
        '\u{31F0}'..='\u{31FF}' => VerticalForm::SmallKana, // ㇰ〜ㇿ
        'ー' | '―' | '—' | '–' | '‐' | '…' | '‥' | '〜' | '～' | '＝' | '＿' | '｜' | '：'
        | '；' | '「' | '」' | '『' | '』' | '（' | '）' | '［' | '］' | '｛' | '｝' | '〈'
        | '〉' | '《' | '》' | '【' | '】' | '〔' | '〕' | '〖' | '〗' | '〘' | '〙' | '＜'
        | '＞' | '→' | '←' | '⇒' => VerticalForm::Sideways,
        // 半角カナは立てて置く
        '\u{FF61}'..='\u{FF9F}' => VerticalForm::Upright,
        c if !is_wide(c) => VerticalForm::Sideways,
        _ => VerticalForm::Upright,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 漢字と仮名と全角英数は立てる() {
        for c in ['漢', 'あ', 'ア', 'Ａ', '１', '！', '？'] {
            assert_eq!(vertical_form(c), VerticalForm::Upright, "{c}");
        }
    }

    #[test]
    fn 半角英数と長音と括弧は寝かせる() {
        for c in ['a', '1', '-', 'ー', '…', '―', '「', '）', '〜'] {
            assert_eq!(vertical_form(c), VerticalForm::Sideways, "{c}");
        }
    }

    #[test]
    fn 句読点と小書きの仮名はずらす() {
        assert_eq!(vertical_form('、'), VerticalForm::Punctuation);
        assert_eq!(vertical_form('。'), VerticalForm::Punctuation);
        assert_eq!(vertical_form('っ'), VerticalForm::SmallKana);
        assert_eq!(vertical_form('ャ'), VerticalForm::SmallKana);
    }

    #[test]
    fn 半角カナは立てる() {
        assert_eq!(vertical_form('ｱ'), VerticalForm::Upright);
    }
}
