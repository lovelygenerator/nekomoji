//! 書記素クラスタ(見た目の1文字)の切り分け。
//!
//! Unicode の拡張書記素クラスタ(UAX #29)のうち、日本語のテキストで出会うものだけを手書きで扱う
//! (nekomoji は外部の crate に依存しないため)。
//!
//! - 結合文字・異体字セレクタ(IVS の `辻󠄀` = U+8FBB U+E0100、`FE00`〜`FE0F`)・結合用の濁点・半濁点
//!   (U+3099・U+309A)・絵文字の肌の色(U+1F3FB〜U+1F3FF)・タグ文字は、前の文字に付ける
//! - ZWJ(U+200D)で絵文字がつながる並び(`👨‍👩‍👧`)は1つにする
//! - 国旗(地域指示記号 U+1F1E6〜U+1F1FF)は2つずつ1つにする
//!
//! ハングルの字母の組み合わせや、インド系の文字の SpacingMark・Prepend は扱わない
//! (ゲームの日本語テキストではまず出ない)。

/// 文字列を書記素クラスタ(見た目の1文字)に分ける。
///
/// 返す各要素は空でない。つなぐと元の文字列に戻る。
pub fn graphemes(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    // 今のクラスタの状態
    let mut prev: Option<char> = None;
    let mut pictographic = false; // クラスタが絵文字で始まった(GB11)
    let mut ri_count = 0; // クラスタの中の地域指示記号の数(GB12・GB13)
    for (i, ch) in s.char_indices() {
        let join = match prev {
            None => false,
            Some(p) => {
                is_extend(ch)
                    || ch == ZWJ
                    || (p == ZWJ && pictographic && is_pictographic(ch))
                    || (is_regional(ch) && is_regional(p) && ri_count % 2 == 1)
            }
        };
        if !join {
            if i > start {
                out.push(&s[start..i]);
            }
            start = i;
            pictographic = is_pictographic(ch);
            ri_count = 0;
        }
        if is_regional(ch) {
            ri_count += 1;
        }
        prev = Some(ch);
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

const ZWJ: char = '\u{200D}';

/// 前の文字に付ける文字か(Grapheme_Cluster_Break=Extend のうち、扱うもの)。
pub(crate) fn is_extend(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0300..=0x036F     // 結合ダイアクリティカルマーク
            | 0x0483..=0x0489
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x200C            // ZWNJ
            | 0x20D0..=0x20FF   // 記号用の結合マーク(囲み・キーキャップ)
            | 0x302A..=0x302F   // 漢字の声調記号
            | 0x3099..=0x309A   // 結合用の濁点・半濁点
            | 0x180B..=0x180F   // モンゴル文字の異体字セレクタ
            | 0xFE00..=0xFE0F   // 異体字セレクタ
            | 0xFE20..=0xFE2F
            | 0x1F3FB..=0x1F3FF // 絵文字の肌の色
            | 0xE0020..=0xE007F // タグ文字
            | 0xE0100..=0xE01EF // 異体字セレクタ(IVS)
    )
}

fn is_regional(ch: char) -> bool {
    matches!(ch as u32, 0x1F1E6..=0x1F1FF)
}

/// 絵文字か(Extended_Pictographic のおおよそ)。
fn is_pictographic(ch: char) -> bool {
    matches!(
        ch as u32,
        0x00A9
            | 0x00AE
            | 0x203C
            | 0x2049
            | 0x2122
            | 0x2139
            | 0x2194..=0x21AA
            | 0x231A..=0x23FF
            | 0x24C2
            | 0x25AA..=0x25FE
            | 0x2600..=0x27BF
            | 0x2934..=0x2935
            | 0x2B05..=0x2B55
            | 0x3030
            | 0x303D
            | 0x3297
            | 0x3299
            | 0x1F000..=0x1F1E5
            | 0x1F200..=0x1F3FA
            | 0x1F400..=0x1FAFF
            | 0x1FC00..=0x1FFFD
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 普通の文字は1文字ずつに分ける() {
        assert_eq!(graphemes("あいう"), vec!["あ", "い", "う"]);
        assert_eq!(graphemes(""), Vec::<&str>::new());
    }

    #[test]
    fn 異体字セレクタは前の文字に付ける() {
        assert_eq!(graphemes("辻\u{E0100}が"), vec!["辻\u{E0100}", "が"]);
        assert_eq!(graphemes("葛\u{FE00}"), vec!["葛\u{FE00}"]);
    }

    #[test]
    fn 結合文字は前の文字に付ける() {
        assert_eq!(graphemes("か\u{3099}"), vec!["か\u{3099}"]);
        assert_eq!(graphemes("e\u{301}t"), vec!["e\u{301}", "t"]);
        assert_eq!(graphemes("1\u{FE0F}\u{20E3}"), vec!["1\u{FE0F}\u{20E3}"]);
    }

    #[test]
    fn 先頭の結合文字は単独の1文字() {
        assert_eq!(graphemes("\u{3099}あ"), vec!["\u{3099}", "あ"]);
    }

    #[test]
    fn zwjでつながる絵文字は1文字() {
        let family = "👨\u{200D}👩\u{200D}👧";
        assert_eq!(graphemes(&format!("{family}と")), vec![family, "と"]);
        assert_eq!(graphemes("👍\u{1F3FD}👍"), vec!["👍\u{1F3FD}", "👍"]);
    }

    #[test]
    fn zwjの後が絵文字でなければつなげない() {
        assert_eq!(graphemes("a\u{200D}b"), vec!["a\u{200D}", "b"]);
    }

    #[test]
    fn 国旗は2つずつ() {
        let jp = "\u{1F1EF}\u{1F1F5}";
        let us = "\u{1F1FA}\u{1F1F8}";
        assert_eq!(graphemes(&format!("{jp}{us}")), vec![jp, us]);
        assert_eq!(graphemes("\u{1F1EF}"), vec!["\u{1F1EF}"]);
    }

    #[test]
    fn つなぐと元に戻る() {
        let s = "辻\u{E0100}👨\u{200D}👩\u{200D}👧\u{1F1EF}\u{1F1F5}か\u{3099}e\u{301}";
        assert_eq!(graphemes(s).concat(), s);
    }
}
