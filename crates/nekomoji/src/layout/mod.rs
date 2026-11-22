//! 行分割・禁則・ルビ配置。
//!
//! [`Layout::build`] が [`Script`] を組む流れ:
//! 1. `paragraph`: 本文を書記素クラスタの塊にし、ページ・段落に分ける(字下げのための段落の種類も決める)
//! 2. `measure`: 塊の幅と文字の位置を測る(約物の詰めも)
//! 3. `line_break`: 禁則を守って段落を行に分ける(長い英単語の折り返しも)
//! 4. `build`: 行に並べ、ルビ・揃え・両端揃え・自動改ページをして [`Page`] にする
//! 5. `decoration`: 取り消し線・下線を行ごとに作る
//!
//! 結果の型は `options`(設定)・`page`(ページ・行・グリフ)・`span`(範囲の矩形・当たり判定)。

mod build;
mod decoration;
mod line_break;
mod measure;
mod options;
mod page;
mod paragraph;
mod span;
#[cfg(test)]
mod tests;

pub use options::{Indent, Kinsoku, LayoutOptions, Vars};
pub use page::{Cue, Decoration, Glyph, Line, Page, RubyRun, TcyPlace, ICON_CHAR};
pub use span::{Extent, MarkRun, Span};

use crate::markup::{Script, Style};
use crate::text::metrics::FontMetrics;
use crate::text::vertical::Writing;
use build::build_pages;
use paragraph::split_pages;

/// レイアウトの結果。不変のデータ。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Layout {
    /// 書字方向([`LayoutOptions::writing`] と同じ)
    pub writing: Writing,
    /// ページ
    pub pages: Vec<Page>,
    /// スタイルの表([`Script::styles`] と同じ)
    pub styles: Vec<Style>,
    /// 値のなかった変数(`[var name]` のまま表示される)
    pub missing_vars: Vec<String>,
    /// 本文に出てきた変数の名前(出てきた順・重複なし)。[`Glyph::var`] はこの添字
    pub vars: Vec<String>,
}

impl Layout {
    /// レイアウトを組む。
    pub fn build(
        script: &Script,
        metrics: &dyn FontMetrics,
        opts: &LayoutOptions,
        vars: &Vars,
    ) -> Layout {
        let mut missing = Vec::new();
        let mut used = Vec::new();
        let sources = split_pages(script, vars, &mut missing, &mut used);
        let pages = sources
            .iter()
            .flat_map(|src| build_pages(src, &script.styles, metrics, opts))
            .collect();
        Layout {
            writing: opts.writing,
            pages,
            styles: script.styles.clone(),
            missing_vars: missing,
            vars: used,
        }
    }

    /// グリフが入っている名前付きの範囲の ID(外側から順)。
    pub fn marks_of(&self, glyph: &Glyph) -> &[String] {
        self.styles.get(glyph.style).map_or(&[], |s| &s.marks)
    }

    /// ページの名前付きの範囲を、ひと続きの文字ごとに返す(最初のグリフの順)。
    ///
    /// 同じ ID の範囲が改行・ルビをまたいでもひと続きなら1つ、別の場所に書いたものや、
    /// 自動改ページなどでページをまたいだものはページごとに分かれる。
    pub fn marks(&self, page: usize) -> Vec<MarkRun> {
        let Some(page) = self.pages.get(page) else {
            return Vec::new();
        };
        let mut out: Vec<MarkRun> = Vec::new();
        // ID ごとに、続けて伸ばせる out の添字
        let mut open: Vec<(String, usize)> = Vec::new();
        for (i, g) in page.glyphs.iter().enumerate() {
            let ids = self.marks_of(g);
            open.retain(|(id, _)| ids.contains(id));
            for (n, id) in ids.iter().enumerate() {
                if ids[..n].contains(id) {
                    // 同じ ID の入れ子は1つの範囲として数える
                    continue;
                }
                match open.iter().find(|(o, _)| o == id) {
                    Some(&(_, k)) => out[k].glyph_count += 1,
                    None => {
                        open.push((id.clone(), out.len()));
                        out.push(MarkRun {
                            id: id.clone(),
                            first_glyph: i,
                            glyph_count: 1,
                        });
                    }
                }
            }
        }
        out.sort_by_key(|m| m.first_glyph);
        out
    }

    /// ページの中で ID が `id` の名前付きの範囲を、行ごとの矩形にする。
    pub fn mark_spans(&self, page: usize, id: &str) -> Vec<Span> {
        let Some(p) = self.pages.get(page) else {
            return Vec::new();
        };
        self.marks(page)
            .iter()
            .filter(|m| m.id == id)
            .flat_map(|m| p.spans(m.first_glyph, m.glyph_count))
            .collect()
    }

    /// 全ページのグリフ数の合計。
    pub fn glyph_count(&self) -> usize {
        self.pages.iter().map(|p| p.glyphs.len()).sum()
    }
}
