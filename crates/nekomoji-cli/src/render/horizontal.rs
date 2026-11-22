//! 横書きのページを、ルビの行と本文の行の文字列にする。

use nekomoji::{Baseline, Layout, Writing};

use super::cat::CatView;
use super::cells::{decorated, put, str_cells, to_col};
use super::vertical::vertical_rows;

/// ページの行を端末の行にする。`visible` 文字目までを表示する。
///
/// ルビのある行は、その上にルビの行を置く(ルビの行は行番号なし)。
/// 文字送りの途中でも行数は変わらない。
pub fn page_rows(layout: &Layout, page: usize, visible: usize) -> Vec<String> {
    page_rows_with(layout, page, visible, &CatView::default())
}

/// [`page_rows`] に猫モードの表示を重ねる(飛ばされた文字は空白、入力待ちは `🐾`)。
///
/// 縦書きのレイアウトは [`vertical_rows`] で縦に組む。
pub fn page_rows_with(layout: &Layout, page: usize, visible: usize, view: &CatView) -> Vec<String> {
    if layout.writing == Writing::Vertical {
        return vertical_rows(layout, page, visible, view);
    }
    let Some(p) = layout.pages.get(page) else {
        return Vec::new();
    };
    let hidden = |i: usize| view.is_hidden(i);
    let mut rows = Vec::new();
    for (ln, line) in p.lines.iter().enumerate() {
        let rubies: Vec<_> = p
            .rubies
            .iter()
            .enumerate()
            .filter(|(_, r)| r.line == ln)
            .collect();
        if !rubies.is_empty() {
            let mut row = String::new();
            let mut col = 0;
            for (ri, r) in rubies.iter().filter(|(_, r)| r.first_glyph < visible) {
                if view.ruby_hidden(*ri) {
                    continue;
                }
                let center = to_col(r.x + r.width / 2.0);
                let start = center.saturating_sub(str_cells(&r.text) / 2);
                // 端末ではルビを小さくできないので、重なるときは1桁空けて後ろへずらす
                let target = if col > 0 { start.max(col + 1) } else { start };
                put(&mut row, &mut col, target, &r.text);
            }
            rows.push(format!("   | {row}").trim_end().to_string());
        }
        let mut row = String::new();
        let mut col = 0;
        let end = (line.first_glyph + line.glyph_count).min(visible);
        // 上付き・下付きは ^{…} _{…} で示す
        let mut shift = Baseline::Normal;
        for (i, g) in p
            .glyphs
            .iter()
            .enumerate()
            .take(end.max(line.first_glyph))
            .skip(line.first_glyph)
        {
            if hidden(i) {
                continue;
            }
            let style = layout.styles.get(g.style);
            let b = style.map_or(Baseline::Normal, |s| s.baseline);
            let mut text = String::new();
            if b != shift {
                if shift != Baseline::Normal {
                    text.push('}');
                }
                match b {
                    Baseline::Sup => text.push_str("^{"),
                    Baseline::Sub => text.push_str("_{"),
                    Baseline::Normal => {}
                }
                shift = b;
            }
            // 折り返した行の後ろに残る半角空白には線を付けない(コアの線と同じ)
            let trailing_space = g.ch == ' '
                && p.glyphs[i + 1..line.first_glyph + line.glyph_count]
                    .iter()
                    .all(|h| h.ch == ' ');
            text.push_str(&decorated(g.ch, &g.tail, style.filter(|_| !trailing_space)));
            put(&mut row, &mut col, to_col(g.x), &text);
        }
        if shift != Baseline::Normal {
            put(&mut row, &mut col, 0, "}");
        }
        if let Some(c) = view.cursor.filter(|c| c.line == ln) {
            put(&mut row, &mut col, to_col(c.x) + 1, view.cursor_text());
        }
        rows.push(format!("{:>2} | {row}", ln + 1).trim_end().to_string());
    }
    rows
}
