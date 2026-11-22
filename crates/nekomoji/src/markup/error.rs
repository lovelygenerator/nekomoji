//! 解析エラー。行・桁は 1 始まりで、メッセージは日本語。

use std::fmt;

/// 解析エラー。行・桁は 1 始まり。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ParseError {
    /// 行(1 始まり)
    pub line: usize,
    /// 桁(1 始まり、文字単位)
    pub col: usize,
    /// エラーの種類
    pub kind: ErrorKind,
}

/// 解析エラーの種類。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// 知らないタグ
    UnknownTag(String),
    /// 必要な引数がない
    MissingArg(String),
    /// 引数を読めない
    BadArg {
        /// タグ名
        tag: String,
        /// 引数
        arg: String,
    },
    /// 閉じタグが対応しない
    UnexpectedClose {
        /// 閉じるべきタグ(なければ `None`)
        expected: Option<String>,
        /// 見つかった閉じタグ
        found: String,
    },
    /// 閉じられていないタグ
    Unclosed(String),
    /// `[` に対応する `]` がない
    UnterminatedTag,
    /// ルビの中にタグ・改行・変数がある
    NestedInRuby,
    /// ルビの親文字が空
    EmptyRuby,
    /// 上付き・下付きの中にルビ、または上付き・下付きの入れ子がある
    NestedInShift,
    /// 縦中横の中にタグ・改行・変数がある
    NestedInTcy,
    /// 縦中横が空
    EmptyTcy,
    /// `[align]` が段落の途中から始まっている
    AlignMidParagraph,
    /// 文字の `]` が二重になっていない(`]]` と書く)
    LoneClose(char),
    /// `[` の直後(閉じタグでは `/` の直後)に空白がある
    SpaceAfterBracket,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.kind)
    }
}

/// 位置を付けない日本語のメッセージ。
impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::UnknownTag(t) => write!(f, "知らないタグ [{t}]"),
            ErrorKind::MissingArg(t) => write!(f, "[{t}] に引数が必要"),
            ErrorKind::BadArg { tag, arg } => write!(f, "[{tag}] の引数 {arg:?} を読めない"),
            ErrorKind::UnexpectedClose { expected, found } => match expected {
                Some(e) => write!(f, "[/{found}] の前に [/{e}] で閉じる必要がある"),
                None => write!(f, "[/{found}] に対応する開きタグがない"),
            },
            ErrorKind::Unclosed(t) => write!(f, "[{t}] が閉じられていない"),
            ErrorKind::UnterminatedTag => write!(f, "[ に対応する ] がない(文字の [ は [[ と書く)"),
            ErrorKind::NestedInRuby => write!(f, "ルビの親文字にタグ・改行・変数は書けない"),
            ErrorKind::EmptyRuby => write!(f, "ルビの親文字が空"),
            ErrorKind::NestedInShift => {
                write!(
                    f,
                    "上付き・下付きの中にルビや上付き・下付き・縦中横は書けない"
                )
            }
            ErrorKind::NestedInTcy => write!(f, "縦中横の中にタグ・改行・変数は書けない"),
            ErrorKind::EmptyTcy => write!(f, "縦中横が空"),
            ErrorKind::AlignMidParagraph => write!(
                f,
                "[align] は段落の頭(行頭・[br]・[p] の直後)に書く(揃えは段落ごと)"
            ),
            ErrorKind::LoneClose(c) => write!(f, "文字の {c} は {c}{c} と書く"),
            ErrorKind::SpaceAfterBracket => {
                write!(f, "[ の直後(閉じタグでは / の直後)に空白は書けない")
            }
        }
    }
}

impl ErrorKind {
    /// 種類を表す変わらない英語の名前(`unknown_tag` など)。機械で読む出力(`check --json`)や、
    /// メッセージの言い回しに頼らずに種類で分けるときに使う。
    pub fn code(&self) -> &'static str {
        match self {
            ErrorKind::UnknownTag(_) => "unknown_tag",
            ErrorKind::MissingArg(_) => "missing_arg",
            ErrorKind::BadArg { .. } => "bad_arg",
            ErrorKind::UnexpectedClose { .. } => "unexpected_close",
            ErrorKind::Unclosed(_) => "unclosed",
            ErrorKind::UnterminatedTag => "unterminated_tag",
            ErrorKind::NestedInRuby => "nested_in_ruby",
            ErrorKind::EmptyRuby => "empty_ruby",
            ErrorKind::NestedInShift => "nested_in_shift",
            ErrorKind::NestedInTcy => "nested_in_tcy",
            ErrorKind::EmptyTcy => "empty_tcy",
            ErrorKind::AlignMidParagraph => "align_mid_paragraph",
            ErrorKind::LoneClose(_) => "lone_close",
            ErrorKind::SpaceAfterBracket => "space_after_bracket",
        }
    }
}

impl std::error::Error for ParseError {}
