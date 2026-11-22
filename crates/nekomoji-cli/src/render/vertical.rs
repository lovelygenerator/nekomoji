//! 縦書きのページを、右から左へ並ぶ列にして行の文字列にする。

use nekomoji::chars::{vertical_form, VerticalForm};
use nekomoji::Layout;

use super::cat::CatView;
use super::cells::{decorated, str_cells};

/// 端末で縦書きを表すときの字形。縦書き用の互換文字があればそれに替える。
///
/// 端末では文字を回転できないので、句読点・括弧・長音などは Unicode の縦書き用の形で示す。
pub fn terminal_vertical_char(ch: char) -> char {
    match ch {
        '、' | '､' => '︑',
        '。' | '｡' => '︒',
        '，' => '︐',
        '：' => '︓',
        '；' => '︔',
        '…' => '︙',
        '‥' => '︰',
        '―' | '—' => '︱',
        '–' => '︲',
        'ー' => '｜',
        '（' => '︵',
        '）' => '︶',
        '｛' => '︷',
        '｝' => '︸',
        '〔' => '︹',
        '〕' => '︺',
        '【' => '︻',
        '】' => '︼',
        '《' => '︽',
        '》' => '︾',
        '〈' => '︿',
        '〉' => '﹀',
        '「' => '﹁',
        '」' => '﹂',
        '『' => '﹃',
        '』' => '﹄',
        '［' => '﹇',
        '］' => '﹈',
        '〜' | '～' => '≀',
        c => c,
    }
}

/// 位置(全角1文字 = 1.0)を端末の段に変える。
fn to_row(x: f32) -> usize {
    (x + 1e-4).floor().max(0.0) as usize
}

/// 縦書きの1列(上から下へのマス)。
#[derive(Default)]
struct Column {
    pub(super) cells: Vec<String>,
    header: String,
}

impl Column {
    fn set(&mut self, row: usize, text: &str) {
        if self.cells.len() <= row {
            self.cells.resize(row + 1, String::new());
        }
        self.cells[row].push_str(text);
    }

    pub(super) fn width(&self) -> usize {
        self.cells
            .iter()
            .map(|c| str_cells(c))
            .chain([str_cells(&self.header), 2])
            .max()
            .unwrap_or(2)
    }
}

/// 縦書きのページを端末の行にする。行は右から左へ並び、1段が全角1文字。
///
/// 1行目は行の番号。各行の右にルビの列(ルビがある行だけ)を置く。半角の文字は1段に2つまで入る。
/// 猫モードの入力待ちでは、最後の文字の下に `🐾` を出す。
/// 文字送りの途中でも列の数は変わらない。
pub fn vertical_rows(layout: &Layout, page: usize, visible: usize, view: &CatView) -> Vec<String> {
    let Some(p) = layout.pages.get(page) else {
        return Vec::new();
    };
    let hidden = |i: usize| view.is_hidden(i);
    // 右から左への列
    let mut cols: Vec<Column> = Vec::new();
    let mut height = 0usize;
    for (ln, line) in p.lines.iter().enumerate() {
        let rubies: Vec<_> = p
            .rubies
            .iter()
            .enumerate()
            .filter(|(_, r)| r.line == ln)
            .collect();
        if !rubies.is_empty() {
            let mut col = Column::default();
            let mut next = 0usize;
            for (ri, r) in &rubies {
                let n = r.text.chars().count();
                // 端末ではルビを小さくできないので、親文字の中央から1文字1段で置き、重なれば下へずらす
                let center = r.x + r.width / 2.0;
                let start = to_row((center - n as f32 / 4.0).max(0.0)).max(next);
                next = start + n;
                height = height.max(next);
                if r.first_glyph >= visible || view.ruby_hidden(*ri) {
                    continue;
                }
                for (k, c) in r.text.chars().enumerate() {
                    col.set(start + k, &terminal_vertical_char(c).to_string());
                }
            }
            cols.push(col);
        }
        let mut col = Column {
            header: format!("{:>2}", ln + 1),
            ..Column::default()
        };
        let end = line.first_glyph + line.glyph_count;
        for (i, g) in p.glyphs.iter().enumerate().take(end).skip(line.first_glyph) {
            height = height.max(to_row(g.x) + 1);
            if i >= visible || hidden(i) {
                continue;
            }
            if g.tcy.is_some() {
                // 縦中横は立てたまま1段に横に並べる(同じ段に続けて書く)
                col.set(
                    to_row(g.x),
                    &decorated(g.ch, &g.tail, layout.styles.get(g.style)),
                );
                continue;
            }
            let c = match vertical_form(g.ch) {
                VerticalForm::Sideways | VerticalForm::Punctuation => terminal_vertical_char(g.ch),
                _ => g.ch,
            };
            col.set(
                to_row(g.x),
                &decorated(
                    c,
                    &g.tail,
                    // 折り返した行の後ろに残る半角空白には線を付けない
                    layout.styles.get(g.style).filter(|_| {
                        !(g.ch == ' ' && p.glyphs[i + 1..end].iter().all(|h| h.ch == ' '))
                    }),
                ),
            );
        }
        if let Some(c) = view.cursor.filter(|c| c.line == ln) {
            col.set(to_row(c.x), view.cursor_text());
        }
        cols.push(col);
    }
    height = height.max(cols.iter().map(|c| c.cells.len()).max().unwrap_or(0));
    let widths: Vec<usize> = cols.iter().map(Column::width).collect();
    let mut rows = Vec::with_capacity(height + 1);
    for r in 0..=height {
        let mut row = String::new();
        // 左の列(最後の行)から書く
        for (ci, col) in cols.iter().enumerate().rev() {
            let text = if r == 0 {
                col.header.as_str()
            } else {
                col.cells.get(r - 1).map_or("", String::as_str)
            };
            let w = widths[ci];
            row.push(' ');
            // 文字は列の右に寄せる(行の番号と揃える)
            row.push_str(&" ".repeat(w.saturating_sub(str_cells(text))));
            row.push_str(text);
        }
        rows.push(row.trim_end().to_string());
    }
    rows
}
