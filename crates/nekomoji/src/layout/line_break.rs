//! 禁則を守って段落を行に分ける。長い英単語の折り返し。

use super::measure::measure;
use super::options::{Kinsoku, LayoutOptions};
use super::paragraph::Unit;
use crate::markup::Style;
use crate::text::kinsoku::{self};
use crate::text::metrics::FontMetrics;

pub(super) fn first_char(u: &Unit) -> char {
    u.chars[0].ch
}

pub(super) fn last_char(u: &Unit) -> char {
    u.chars.last().expect("塊は1文字以上").ch
}

/// `b` の前で改行すると禁則に触れるか。
fn violates(units: &[Unit], b: usize) -> bool {
    let next = first_char(&units[b]);
    let prev = last_char(&units[b - 1]);
    kinsoku::is_line_start_forbidden(next)
        || kinsoku::is_line_end_forbidden(prev)
        || kinsoku::is_inseparable(prev, next)
}

pub(super) const EPS: f32 = 1e-4;

/// 半角空白だけの塊。行末に来たら幅に数えず、次の行の頭には置かない。
pub(super) fn is_space(u: &Unit) -> bool {
    u.ruby.is_none() && !u.tcy && u.icon.is_none() && u.chars.len() == 1 && u.chars[0].ch == ' '
}

/// 段落を行に分ける。各行の塊の範囲を返す。
///
/// `leads[i]` は塊 `i` の前に入れるアキ(和欧間)。行頭では入れない。
/// `indent` は (1行目, 2行目以降) の字下げで、その分だけ行に入る幅が減る。
pub(super) fn break_lines(
    units: &[Unit],
    widths: &[f32],
    leads: &[f32],
    indent: (f32, f32),
    opts: &LayoutOptions,
) -> Vec<(usize, usize)> {
    let n = units.len();
    let mut lines = Vec::new();
    let mut start = 0;
    while start < n {
        let room = opts.max_width - if lines.is_empty() { indent.0 } else { indent.1 };
        let mut w = 0.0;
        let mut b = start;
        while b < n && (b == start || w + leads[b] + widths[b] <= room + EPS) {
            if b != start {
                w += leads[b];
            }
            w += widths[b];
            b += 1;
        }
        if b < n {
            // 禁則に触れる位置と、スタイルの境目で分かれた語の途中(禁則なしでも)では改行しない
            let standard = opts.kinsoku == Kinsoku::Standard;
            let rule = |k: usize| units[k].glue || (standard && violates(units, k));
            // 空白の前で改行すると空白は行末に残るので、空白の後ろの文字で判定する
            let blocked = |k: usize| {
                let mut j = k;
                while j < n && is_space(&units[j]) {
                    j += 1;
                }
                if j == k {
                    rule(k)
                } else {
                    j < n && rule(j)
                }
            };
            let single = units[b].chars.len() == 1 && units[b].ruby.is_none();
            let hang = standard
                && opts.hanging
                && single
                && kinsoku::is_hangable(first_char(&units[b]))
                && !(b + 1 < n && blocked(b + 1));
            if hang {
                b += 1;
            } else {
                let mut nb = b;
                while nb > start + 1 && blocked(nb) {
                    nb -= 1;
                }
                // 追い出せる位置がなければ、幅を超えても禁則を優先する(追い込み)
                if blocked(nb) {
                    let mut fb = b;
                    while fb < n && blocked(fb) {
                        fb += 1;
                    }
                    b = fb;
                } else {
                    b = nb;
                }
            }
        }
        // 折り返した行の後ろの半角空白は行末に残す(次の行の頭に空白を出さない。幅には数えない)
        if b < n {
            while b < n && is_space(&units[b]) {
                b += 1;
            }
        }
        lines.push((start, b));
        start = b;
    }
    if lines.is_empty() {
        lines.push((0, 0));
    }
    lines
}

/// 行に収まらない長い英単語(英数字の連続)を、書記素クラスタごとの塊に分ける。
///
/// 英単語は途中で分けないが、どの行にも収まらない(字下げを引いた行の幅より長い)語だけは、
/// 幅からはみ出さないように文字の間で折り返す。ルビ・縦中横・アイコンの塊は分けない。
pub(super) fn split_long_words(
    units: &[Unit],
    indent: (f32, f32),
    styles: &[Style],
    metrics: &dyn FontMetrics,
    opts: &LayoutOptions,
) -> Vec<Unit> {
    let room = opts.max_width - indent.0.max(indent.1);
    let splittable = |u: &Unit| u.ruby.is_none() && !u.tcy && u.icon.is_none();
    let mut out = Vec::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        // スタイルの境目で分かれた語は、つながった塊をまとめて1語として測る
        let mut j = i + 1;
        while j < units.len() && units[j].glue {
            j += 1;
        }
        let word = &units[i..j];
        let width: f32 = word
            .iter()
            .map(|u| measure(u, styles, metrics, opts).width)
            .sum();
        let long = word.iter().map(|u| u.chars.len()).sum::<usize>() > 1
            && word.iter().all(splittable)
            && width > room + EPS;
        for u in word {
            if !long {
                out.push(u.clone());
                continue;
            }
            // 文字ごとに塊を複製すると語の長さの2乗になるので、文字と合図を空にした型を1度だけ作る
            let mut base = u.clone();
            base.chars = Vec::new();
            base.cues = Vec::new();
            base.glue = false;
            for (k, c) in u.chars.iter().enumerate() {
                let mut one = base.clone();
                one.chars = vec![c.clone()];
                if k == 0 {
                    one.cues = u.cues.clone();
                }
                out.push(one);
            }
        }
        i = j;
    }
    out
}
