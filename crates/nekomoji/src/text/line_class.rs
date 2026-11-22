//! 行分割での文字の分類。
//!
//! UAX #14(Unicode の行分割)の分類を、JIS X 4051 の和文の約物の扱いに合わせて読み替えたもの。
//! 禁則([`super::kinsoku`])と、英単語・数をひとつの塊にする判定(`layout/paragraph.rs`)は、
//! どちらもこの表から決める。文字を足すときはこの表だけを直す。
//!
//! | 分類 | UAX #14 | 行頭 | 行末 | 例 |
//! |---|---|---|---|---|
//! | `Open` | OP | | 置けない | `「` `（` `(` `“` `«` |
//! | `Close` | CL・CP | 置けない | | `」` `）` `)` `”` `»` `〟` |
//! | `Exclam` | EX | 置けない | | `！` `？` `!` `?` `‼` |
//! | `Infix` | IS | 置けない | | `、` `。` `,` `.` `：` `:` `・` |
//! | `NonStarter` | NS・CJ | 置けない | | 小書きの仮名・`ー`・`〜`・`々`・`ゝ` |
//! | `Inseparable` | IN・B2 | | | `…` `‥` `―` `—`(同じ字の間で分けない) |
//! | `Hyphen` | HY | 置けない | | `-` |
//! | `Slash` | SY | 置けない | | `/` |
//! | `Prefix` | PR | | 置けない | `$` `¥` `€` `£` `₩` `＄` `￥` `№` |
//! | `Postfix` | PO | 置けない | | `%` `‰` `°` `′` `℃` `¢` `％` |
//! | `Numeric` | NU | | | 半角数字 |
//! | `Alpha` | AL | | | 半角英字・アクセント付きのラテン文字 |
//! | `Space` | SP | | | 半角空白 |
//! | `Other` | ID ほか | | | 漢字・仮名・記号 |

/// 行分割での文字の分類。上の表を参照。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LineClass {
    Open,
    Close,
    Exclam,
    Infix,
    NonStarter,
    Inseparable,
    Hyphen,
    Slash,
    Prefix,
    Postfix,
    Numeric,
    Alpha,
    Space,
    Other,
}

/// 文字の分類。
pub(crate) fn line_class(ch: char) -> LineClass {
    use LineClass::*;
    match ch {
        '「' | '『' | '（' | '(' | '［' | '[' | '｛' | '{' | '〈' | '《' | '【' | '〔' | '〘'
        | '〖' | '“' | '‘' | '〝' | '«' | '‹' | '｟' | '｢' => Open,
        '」' | '』' | '）' | ')' | '］' | ']' | '｝' | '}' | '〉' | '》' | '】' | '〕' | '〙'
        | '〗' | '”' | '’' | '〟' | '»' | '›' | '｠' | '｣' => Close,
        '！' | '？' | '!' | '?' | '‼' | '⁇' | '⁈' | '⁉' => Exclam,
        '、' | '。' | '，' | '．' | ',' | '.' | '・' | '：' | '；' | ':' | ';' | '､' | '｡'
        | '･' => Infix,
        'ー' | '～' | '〜' | '゠' | '‐' | '–' | '゛' | '゜' | 'ゝ' | 'ゞ' | 'ヽ' | 'ヾ' | '々'
        | '〻' | 'ぁ' | 'ぃ' | 'ぅ' | 'ぇ' | 'ぉ' | 'っ' | 'ゃ' | 'ゅ' | 'ょ' | 'ゎ' | 'ゕ'
        | 'ゖ' | 'ァ' | 'ィ' | 'ゥ' | 'ェ' | 'ォ' | 'ッ' | 'ャ' | 'ュ' | 'ョ' | 'ヮ' | 'ヵ'
        | 'ヶ' | '\u{31F0}'..='\u{31FF}' // ㇰ〜ㇿ
        | '\u{FF67}'..='\u{FF70}' // 半角の小書きの仮名と長音
        | '\u{FF9E}' | '\u{FF9F}' => NonStarter,
        '…' | '‥' | '―' | '─' | '—' | '⋯' => Inseparable,
        '-' => Hyphen,
        '/' => Slash,
        '$' | '¥' | '£' | '€' | '₩' | '₹' | '₽' | '₺' | '₴' | '₱' | '₫' | '₪' | '฿' | '＄'
        | '￥' | '￡' | '￦' | '№' => Prefix,
        '%' | '‰' | '‱' | '°' | '′' | '″' | '℃' | '℉' | '¢' | '％' | '￠' => Postfix,
        '0'..='9' => Numeric,
        ' ' => Space,
        c if c.is_ascii_alphabetic() || (matches!(c as u32, 0xC0..=0x24F) && c.is_alphabetic()) => {
            Alpha
        }
        _ => Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use LineClass::*;

    #[test]
    fn 分類の表() {
        for (c, k) in [
            ('「', Open),
            ('«', Open),
            ('」', Close),
            ('〟', Close),
            ('！', Exclam),
            ('、', Infix),
            (':', Infix),
            ('っ', NonStarter),
            ('〜', NonStarter),
            ('ｧ', NonStarter),
            ('…', Inseparable),
            ('—', Inseparable),
            ('-', Hyphen),
            ('/', Slash),
            ('$', Prefix),
            ('￥', Prefix),
            ('%', Postfix),
            ('℃', Postfix),
            ('7', Numeric),
            ('é', Alpha),
            (' ', Space),
            ('あ', Other),
            ('漢', Other),
            ('×', Other),
        ] {
            assert_eq!(line_class(c), k, "{c}");
        }
    }
}
