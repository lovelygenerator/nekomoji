//! ちょっかいの対象: 一緒に飛ぶ文字の塊・押して落とす文字・猫パンチの相手・爆散。

use std::ops::Range;

use super::flight::{Flight, FLIGHT_TOTAL};
use super::rng::XorShift;
use super::Hit;
use crate::layout::Page;

/// 爆散: ページの文字の塊ごとに、猫のいる所から外へ向かう初速を乱数で決める。
pub(super) fn burst(page: &Page, seed: u64, pi: usize, cx: f32, cl: usize, at: f32) -> Vec<Hit> {
    let mut rng = XorShift::new(seed ^ (pi as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let mut hits = Vec::new();
    let mut i = 0;
    while i < page.glyphs.len() {
        let (range, ruby) = unit_of(page, i);
        i = range.end.max(i + 1);
        let g = &page.glyphs[range.start];
        if g.ch.is_whitespace() {
            continue;
        }
        let last = &page.glyphs[range.end - 1];
        let dx = (g.x + last.x + last.width) / 2.0 - cx;
        let dy = (g.line as f32 - cl as f32) * 1.8;
        let len = (dx * dx + dy * dy).sqrt().max(0.5);
        let speed = rng.range(3.0, 7.0);
        let vx = dx / len * speed + rng.range(-1.5, 1.5);
        let vy = dy / len * speed - rng.range(3.0, 6.0);
        let spin = rng.range(-9.0, 9.0);
        // 猫のそばから順に弾ける
        let delay = (len * 0.02).min(0.2);
        hits.push((at + delay, range, ruby, Flight::Burst { vx, vy, spin }));
    }
    hits
}

/// `i` 文字目を含む「一緒に飛ぶ塊」: ルビ付きなら親文字すべてとルビ、縦中横なら塊ごと、ほかは1文字。
pub(super) fn unit_of(page: &Page, i: usize) -> (Range<usize>, Option<usize>) {
    for (ri, r) in page.rubies.iter().enumerate() {
        if r.emphasis {
            continue;
        }
        if (r.first_glyph..r.first_glyph + r.glyph_count).contains(&i) {
            return (r.first_glyph..r.first_glyph + r.glyph_count, Some(ri));
        }
    }
    let g = &page.glyphs[i];
    if g.tcy.is_some() {
        let same = |p: &crate::layout::Glyph| p.tcy.is_some() && p.line == g.line && p.x == g.x;
        let mut start = i;
        while start > 0 && same(&page.glyphs[start - 1]) {
            start -= 1;
        }
        let mut end = i + 1;
        while end < page.glyphs.len() && same(&page.glyphs[end]) {
            end += 1;
        }
        return (start..end, None);
    }
    (i..i + 1, None)
}

/// 最後に見えている文字(空白を除く)の塊。
pub(super) fn knock_target(page: &Page) -> Option<(Range<usize>, Option<usize>)> {
    let i = page.glyphs.iter().rposition(|g| !g.ch.is_whitespace())?;
    Some(unit_of(page, i))
}

/// 猫のそば(1.5 文字以内)で、表示済みで、今飛んでいない文字の塊。いちばん近いもの。
pub(super) fn punch_target(
    page: &Page,
    shown: usize,
    line: usize,
    x: f32,
    tau: f32,
    punches: &[Hit],
) -> Option<(Range<usize>, Option<usize>, f32)> {
    let l = page.lines.get(line)?;
    let flying = |i: usize| {
        punches
            .iter()
            .any(|(at, r, _, _)| r.contains(&i) && tau - at < FLIGHT_TOTAL)
    };
    let mut best: Option<(usize, f32)> = None;
    let end = (l.first_glyph + l.glyph_count).min(shown);
    for (i, g) in page.glyphs.iter().enumerate().take(end).skip(l.first_glyph) {
        if g.ch.is_whitespace() || flying(i) {
            continue;
        }
        let d = (g.x + g.width / 2.0 - x).abs();
        if d <= 1.5 && best.map_or(true, |(_, b)| d < b) {
            best = Some((i, d));
        }
    }
    let (i, _) = best?;
    let (range, ruby) = unit_of(page, i);
    let g = &page.glyphs[i];
    Some((range, ruby, g.x + g.width / 2.0))
}
