//! 猫モードの端末での見え方。猫そのものは出さず、飛んだ文字を隠し、入力待ちで肉球を出す。

use std::ops::Range;

use nekomoji::{Cursor, Layout, Neko, Reveal};

/// 端末に重ねる猫モードの表示。見えない猫に飛ばされた文字は空白にし、
/// 入力待ちでは最後の文字の後ろに `🐾` を出す。
#[derive(Debug, Clone, Default)]
pub struct CatView {
    /// 飛ばされて見えなくなっている文字と、一緒に飛んだルビ
    pub hidden: Vec<(Range<usize>, Option<usize>)>,
    /// 入力待ちのカーソル
    pub cursor: Option<Cursor>,
    /// `🐾` の代わりに `=^.^=` で表示する
    pub ascii: bool,
}

impl CatView {
    /// 今の状態から作る。飛んでいる途中で半分より薄い文字は見えないものとして空白にする。
    pub fn new(neko: &Neko, layout: &Layout, reveal: &Reveal, ascii: bool) -> CatView {
        CatView {
            hidden: neko
                .knocked(layout, reveal)
                .into_iter()
                .filter(|k| k.fall.alpha < 0.5 || k.fall.offset != (0.0, 0.0))
                .map(|k| (k.glyphs, k.ruby))
                .collect(),
            cursor: reveal.cursor(layout),
            ascii,
        }
    }

    pub(super) fn cursor_text(&self) -> &'static str {
        if self.ascii {
            "=^.^="
        } else {
            "🐾"
        }
    }

    pub(super) fn is_hidden(&self, i: usize) -> bool {
        self.hidden.iter().any(|(r, _)| r.contains(&i))
    }

    pub(super) fn ruby_hidden(&self, ri: usize) -> bool {
        self.hidden.iter().any(|(_, r)| *r == Some(ri))
    }
}
