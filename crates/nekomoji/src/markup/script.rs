//! 解析の結果と、文字送りの途中で外へ知らせる出来事。

use std::fmt;
use std::ops::Range;

use super::style::Style;

/// 文字送りの途中で外へ知らせる出来事。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TextEvent {
    /// ボイスを再生する(`[voice id]`)
    Voice(String),
    /// 効果音を鳴らす(`[se id]`)
    Se(String),
    /// 利用者定義のイベント(`[event 名前 引数...]`)
    Custom {
        /// イベント名
        name: String,
        /// 引数
        args: Vec<String>,
    },
    /// ページの終わりで入力を待つ(`[p]` の位置)
    WaitInput,
    /// ページの途中で入力を待つ(`[l]` の位置。改ページしない)。
    /// [`crate::Reveal::proceed`] で同じページの続きを出す
    WaitClick,
    /// すべて表示し終えた
    Finished,
    /// 文字送りの音(RPG の「ポポポ」)を鳴らす。音の ID。
    /// [`crate::RevealOptions::blip`] か `[blip]` の範囲で、何文字かごとに出る
    Blip(String),
}

impl fmt::Display for TextEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextEvent::Voice(id) => write!(f, "voice {id}"),
            TextEvent::Se(id) => write!(f, "se {id}"),
            TextEvent::Custom { name, args } => {
                write!(f, "event {name}")?;
                for a in args {
                    write!(f, " {a}")?;
                }
                Ok(())
            }
            TextEvent::WaitInput => write!(f, "入力待ち"),
            TextEvent::WaitClick => write!(f, "クリック待ち"),
            TextEvent::Blip(id) => write!(f, "blip {id}"),
            TextEvent::Finished => write!(f, "終了"),
        }
    }
}

/// 解析結果の要素。スタイルは [`Script::styles`] の添字で持つ。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Item {
    /// 文字の並び
    Text {
        /// 本文
        text: String,
        /// スタイルの添字
        style: usize,
        /// 1文字(`char`)ごとの、元の文章のバイト範囲(エスケープ `[[` などは2バイト分)
        source: Vec<Range<usize>>,
    },
    /// ルビ付きの親文字
    Ruby {
        /// 親文字
        base: String,
        /// 読み
        reading: String,
        /// スタイルの添字
        style: usize,
        /// 親文字の1文字(`char`)ごとの、元の文章のバイト範囲
        source: Vec<Range<usize>>,
    },
    /// 縦中横(`[tcy]…[/tcy]`)。縦書きで横に並べて全角1文字分に収める
    Tcy {
        /// 横に並べる文字
        text: String,
        /// スタイルの添字
        style: usize,
        /// 1文字(`char`)ごとの、元の文章のバイト範囲
        source: Vec<Range<usize>>,
    },
    /// 変数(`[var name]`)。レイアウト時に展開する
    Var {
        /// 変数名
        name: String,
        /// スタイルの添字
        style: usize,
        /// `[var name]` 全体の、元の文章のバイト範囲
        source: Range<usize>,
    },
    /// 文中のアイコン(`[icon id 幅?]`)。レイアウトは場所を空けて ID を渡すだけで、絵は描画側が描く
    Icon {
        /// アイコンの ID
        id: String,
        /// 行の中の向きの長さ(全角1文字を 1.0 とする単位。字の大きさを掛ける)
        width: f32,
        /// スタイルの添字
        style: usize,
        /// `[icon …]` 全体の、元の文章のバイト範囲
        source: Range<usize>,
    },
    /// 待機(秒)
    Wait(f32),
    /// ページの途中で入力を待つ(`[l]`。改ページしない)
    Click,
    /// 改行
    Break,
    /// 改ページ(入力を待つ)
    Page,
    /// 外へ知らせる出来事
    Event(TextEvent),
}

/// 解析済みのマークアップ。
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct Script {
    /// 要素の列
    pub items: Vec<Item>,
    /// スタイルの表(重複なし)。添字 0 は既定のスタイル
    pub styles: Vec<Style>,
}
