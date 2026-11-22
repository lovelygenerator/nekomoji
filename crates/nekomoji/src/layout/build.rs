//! 1ページ分を組む: 行に並べ、ルビ・揃え・両端揃え・自動改ページをする。

use super::decoration::decorations;
use super::line_break::{break_lines, first_char, is_space, last_char, split_long_words, EPS};
use super::measure::{measure, squeeze_punct, Measured};
use super::options::{Kinsoku, LayoutOptions};
use super::page::{Glyph, Line, Page, RubyRun};
use super::paragraph::{PageSrc, Unit};
use crate::markup::{Align, Style};
use crate::text::kinsoku::{is_closing_punct, is_latin, is_opening_bracket, is_wa};
use crate::text::metrics::FontMetrics;
use crate::text::vertical::Writing;

/// `[p]` で区切られた1ページ分を組む。`max_lines` を超えたら自動で次のページへ分ける。
pub(super) fn build_pages(
    src: &PageSrc,
    styles: &[Style],
    metrics: &dyn FontMetrics,
    opts: &LayoutOptions,
) -> Vec<Page> {
    let mut done: Vec<Page> = Vec::new();
    let mut page = Page::default();
    // 自動改ページの直後か(次のページの先頭の空行を捨てるため)
    let mut auto_broken = false;
    for para in &src.paragraphs {
        // 段落の揃えは最初の文字の揃え。字下げは行頭揃えのときだけ
        let align = para
            .units
            .first()
            .and_then(|u| styles.get(u.style))
            .and_then(|s| s.align)
            .unwrap_or(opts.align);
        // 空の段落(空行)は字下げしない
        let indent = if para.units.is_empty() || align != Align::Start {
            (0.0, 0.0)
        } else {
            opts.indent.of(para.kind)
        };
        let units = &split_long_words(&para.units, indent, styles, metrics, opts);
        let mut measured: Vec<Measured> = units
            .iter()
            .map(|u| measure(u, styles, metrics, opts))
            .collect();
        let glyph_size = |u: &Unit| styles.get(u.style).map_or(1.0, |s| s.glyph_size());
        if opts.punct_squeeze {
            squeeze_punct(
                units,
                &mut measured,
                &glyph_size,
                opts.kinsoku == Kinsoku::Standard,
            );
        }
        let leads: Vec<f32> = (0..units.len())
            .map(|i| {
                if i == 0 || opts.latin_space <= 0.0 {
                    return 0.0;
                }
                if opts.writing == Writing::Vertical && (units[i - 1].tcy || units[i].tcy) {
                    return 0.0;
                }
                let (a, b) = (last_char(&units[i - 1]), first_char(&units[i]));
                if (is_wa(a) && is_latin(b)) || (is_latin(a) && is_wa(b)) {
                    opts.latin_space * glyph_size(&units[i - 1]).min(glyph_size(&units[i]))
                } else {
                    0.0
                }
            })
            .collect();
        let widths: Vec<f32> = measured.iter().map(|m| m.width).collect();
        let breaks = break_lines(units, &widths, &leads, indent, opts);
        let last_line = breaks.len() - 1;
        for (li, &(start, end)) in breaks.iter().enumerate() {
            if opts.max_lines.is_some_and(|n| page.lines.len() >= n.max(1)) {
                page.decorations = decorations(&page, styles, metrics, opts.writing);
                done.push(std::mem::take(&mut page));
                auto_broken = true;
            }
            // 自動改ページの直後の空行は捨てる(合図は残す)
            if auto_broken && page.lines.is_empty() && start == end {
                for unit in &units[start..end] {
                    for cue in &unit.cues {
                        page.cues.push((page.glyphs.len(), cue.clone()));
                    }
                }
                continue;
            }
            auto_broken = false;
            let line_no = page.lines.len();
            let first_glyph = page.glyphs.len();
            let line_indent = if li == 0 { indent.0 } else { indent.1 };
            let room = opts.max_width - line_indent;
            // 行末の半角空白は幅に数えない(揃え・両端揃え・行の幅)
            let mut content_end = end;
            while content_end > start && is_space(&units[content_end - 1]) {
                content_end -= 1;
            }
            // 両端揃え: 段落の最後の行を除き、幅に満たない行の塊の間を広げる
            let natural: f32 = (start..content_end)
                .map(|i| widths[i] + if i > start { leads[i] } else { 0.0 })
                .sum();
            let rubies_before = page.rubies.len();
            // 広げるのは塊の間だけで、スタイルの境目でつながった語の中(`glue`)は広げない
            let gaps = (start + 1..content_end).filter(|&i| !units[i].glue).count();
            let extra = if opts.justify
                && align == Align::Start
                && li < last_line
                && gaps >= 1
                && natural < room - EPS
            {
                (room - natural) / gaps as f32
            } else {
                0.0
            };
            let line_indent = if start == end { 0.0 } else { line_indent };
            let mut x = line_indent;
            let mut content_x = x;
            let mut size: f32 = 1.0;
            for (i, (unit, m)) in units[start..end]
                .iter()
                .zip(&measured[start..end])
                .enumerate()
            {
                let trailing = start + i >= content_end;
                if i > 0 {
                    x += leads[start + i] + if unit.glue || trailing { 0.0 } else { extra };
                }
                for cue in &unit.cues {
                    page.cues.push((page.glyphs.len(), cue.clone()));
                }
                let style_size = styles.get(unit.style).map_or(1.0, |s| s.size);
                size = size.max(style_size);
                let unit_first = page.glyphs.len();
                let mark = styles.get(unit.style).and_then(|s| s.emphasis);
                let tcy_unit = !m.tcy.is_empty();
                if let Some(mark) = mark.filter(|_| tcy_unit) {
                    // 縦中横には塊に1つだけ付ける
                    page.rubies.push(RubyRun {
                        text: mark.to_string(),
                        x,
                        width: m.width,
                        line: line_no,
                        first_glyph: unit_first,
                        glyph_count: unit.chars.len(),
                        emphasis: true,
                    });
                }
                for (k, (c, &(dx, adv))) in unit.chars.iter().zip(&m.places).enumerate() {
                    let ch = c.ch;
                    if let Some(mark) = mark.filter(|_| {
                        !tcy_unit
                            && unit.ruby.is_none()
                            && unit.icon.is_none()
                            && takes_emphasis(ch)
                    }) {
                        page.rubies.push(RubyRun {
                            text: mark.to_string(),
                            x: x + dx,
                            width: adv,
                            line: line_no,
                            first_glyph: page.glyphs.len(),
                            glyph_count: 1,
                            emphasis: true,
                        });
                    }
                    page.glyphs.push(Glyph {
                        ch,
                        tail: c.tail.clone(),
                        x: x + dx,
                        width: adv,
                        shift: if k == 0 { m.shift } else { 0.0 },
                        line: line_no,
                        style: unit.style,
                        tcy: m.tcy.get(k).copied(),
                        source: c.source.clone(),
                        var: c.var,
                        icon: unit.icon.as_ref().map(|(id, _)| id.clone()),
                    });
                }
                if let Some(r) = &unit.ruby {
                    page.rubies.push(RubyRun {
                        text: r.clone(),
                        x,
                        width: m.width,
                        line: line_no,
                        first_glyph: unit_first,
                        glyph_count: unit.chars.len(),
                        emphasis: false,
                    });
                }
                x += m.width;
                if !trailing {
                    content_x = x;
                }
            }
            let x = content_x;
            // 中央揃え・行末揃え: 組んだ行を残りの幅の半分・全部だけ行末側へずらす
            let shift = match align {
                _ if start == end => 0.0,
                Align::Center => ((opts.max_width - x) / 2.0).max(0.0),
                Align::End => (opts.max_width - x).max(0.0),
                _ => 0.0,
            };
            if shift > 0.0 {
                for g in &mut page.glyphs[first_glyph..] {
                    g.x += shift;
                }
                for r in &mut page.rubies[rubies_before..] {
                    r.x += shift;
                }
            }
            page.lines.push(Line {
                first_glyph,
                glyph_count: page.glyphs.len() - first_glyph,
                width: x + shift,
                indent: line_indent + shift,
                align,
                size,
                paragraph_start: li == 0,
            });
        }
        for cue in &para.tail_cues {
            page.cues.push((page.glyphs.len(), cue.clone()));
        }
    }
    page.decorations = decorations(&page, styles, metrics, opts.writing);
    // 自動改ページの後に行が残らなければ、空のページは作らず、合図は前のページの最後に付ける
    match done.last_mut() {
        Some(prev) if page.lines.is_empty() => {
            let n = prev.glyphs.len();
            prev.cues.extend(page.cues.into_iter().map(|(_, c)| (n, c)));
        }
        _ => done.push(page),
    }
    done
}

/// 圏点を付ける文字か。空白・句読点・括弧・記号には付けない。
fn takes_emphasis(ch: char) -> bool {
    !(ch.is_whitespace()
        || is_closing_punct(ch)
        || is_opening_bracket(ch)
        || matches!(
            ch,
            '・' | '！' | '？' | '!' | '?' | '…' | '‥' | '―' | '—' | ',' | '.' | '「' | '」'
        ))
}
