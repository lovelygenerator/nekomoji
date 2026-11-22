//! 取り消し線・下線を行ごとに作る。

use super::page::{Decoration, Page};
use crate::markup::Style;
use crate::text::metrics::{default_line_metrics, FontMetrics, LineKind};
use crate::text::vertical::Writing;

/// 取り消し線・下線を、行ごと・(種類, 色, 大きさ) が同じ文字の並びごとに作る。
pub(super) fn decorations(
    page: &Page,
    styles: &[Style],
    metrics: &dyn FontMetrics,
    writing: Writing,
) -> Vec<Decoration> {
    let mut out: Vec<Decoration> = Vec::new();
    for kind in [LineKind::Strike, LineKind::Underline] {
        let mut prev: Option<usize> = None;
        for (i, g) in page.glyphs.iter().enumerate() {
            let st = styles.get(g.style).cloned().unwrap_or_default();
            let on = match kind {
                LineKind::Strike => st.strike,
                LineKind::Underline => st.underline,
            };
            // 折り返した行の後ろに残る半角空白には引かない
            let trailing_space = g.ch == ' '
                && page.glyphs[i + 1..]
                    .iter()
                    .take_while(|h| h.line == g.line)
                    .all(|h| h.ch == ' ');
            if !on || trailing_space {
                prev = None;
                continue;
            }
            // 上付き・下付きでも、線は周りの文字(size)に合わせる
            let size = st.size;
            if let Some(d) = prev.and_then(|p| out.get_mut(p)) {
                let same = d.line == g.line
                    && d.first_glyph + d.glyph_count == i
                    && styles
                        .get(d.style)
                        .is_some_and(|s| s.color == st.color && s.size == size);
                if same {
                    d.glyph_count += 1;
                    d.x1 = g.x + g.width;
                    continue;
                }
            }
            let (y, thickness) = match writing {
                Writing::Horizontal => {
                    // 利用者の実装が有限でない値や負の太さを返したら既定の比率にする
                    let (y, t) = metrics.line_metrics(kind, size);
                    if y.is_finite() && t.is_finite() && t >= 0.0 {
                        (y, t)
                    } else {
                        let (y, t) = default_line_metrics(kind);
                        (y * size, t * size)
                    }
                }
                // 縦書き: 取り消し線は行の中央、下線は行の右側(傍線)
                Writing::Vertical => match kind {
                    LineKind::Strike => (0.5 * size, 0.05 * size),
                    LineKind::Underline => (-0.06 * size, 0.05 * size),
                },
            };
            out.push(Decoration {
                kind,
                line: g.line,
                x0: g.x,
                x1: g.x + g.width,
                y,
                thickness,
                first_glyph: i,
                glyph_count: 1,
                style: g.style,
            });
            prev = Some(out.len() - 1);
        }
    }
    out
}
