//! 見えない猫の歩き回り。(シード, ページ番号) の乱数で動作を決め、時刻 `t` まで進める。

use super::cat::{CatAction, Facing};
use super::flight::{Flight, PUNCH_HIT, PUNCH_SECS, STRETCH_SECS};
use super::hit::punch_target;
use super::rng::XorShift;
use super::Hit;
use crate::layout::Page;

fn line_start(page: &Page, line: usize) -> f32 {
    let l = &page.lines[line];
    if l.glyph_count == 0 {
        0.0
    } else {
        page.glyphs[l.first_glyph].x
    }
}

/// ページを出し切ったときの、最後の文字の右端。
fn final_position(page: &Page) -> (f32, usize) {
    match page.glyphs.last() {
        Some(g) => (g.x + g.width, g.line),
        None => (0.0, page.lines.len().saturating_sub(1)),
    }
}

/// 見えない猫の、ある時刻の様子。
pub(super) struct Roam {
    pub(super) x: f32,
    pub(super) line: usize,
    pub(super) facing: Facing,
    pub(super) action: CatAction,
    /// 今の動作を始めた時刻
    pub(super) since: f32,
    /// 入力待ちで最後の文字のそばに着いた時刻(まだなら `None`)
    pub(super) settled_at: Option<f32>,
    /// `t` までに当たった猫パンチ
    pub(super) punches: Vec<Hit>,
}

pub(super) struct RoamParams {
    pub(super) speed: f32,
    pub(super) punch_chance: f32,
    pub(super) seed: u64,
    pub(super) page: usize,
    pub(super) end: f32,
}

const EPS: f32 = 1e-4;
/// 1ページで積む動作の数の上限(1つ 0.5〜3 秒なので、数時間ぶん)。超えたら猫は落ち着く。
const MAX_STEPS: u32 = 10_000;

/// 歩き出しと止まる前をゆっくりにする(0〜1 → 0〜1)。平均の速さの 1.5 倍が最も速い。
fn ease(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// 距離 `d` を速さの目安 `v` で歩くのにかける秒数(緩急をつけるので平均は `v` の 2/3)。
fn walk_secs(d: f32, v: f32) -> f32 {
    1.5 * d.abs() / v.max(EPS)
}

/// 猫が歩ける範囲: その行で表示済みの文字の上。その行にまだ文字が出ていなければ `None`。
pub(super) fn walk_range(page: &Page, shown: usize, line: usize) -> Option<(f32, f32)> {
    let l = page.lines.get(line)?;
    if l.glyph_count == 0 || shown <= l.first_glyph {
        return None;
    }
    let last = (l.first_glyph + l.glyph_count).min(shown) - 1;
    let g = &page.glyphs[last];
    Some((page.glyphs[l.first_glyph].x, g.x + g.width))
}

fn facing_of(d: f32, now: Facing) -> Facing {
    if d > EPS {
        Facing::Right
    } else if d < -EPS {
        Facing::Left
    } else {
        now
    }
}

/// 時刻 `t` の見えない猫の様子。(シード, ページ番号) の乱数で次々に動作を決め、`t` まで進める。
///
/// - 動作は、歩く(ぶらぶら・小走り・駆け足。歩き出しと止まる前はゆっくり)・立ち止まる・
///   毛づくろい・伸び・猫パンチ・新しい文字が出た行へ跳び移る
/// - 歩く先は、猫のいる行で表示済みの文字の上(よく新しい文字のそばへ行く)
/// - 猫パンチは、そばの文字を歩いている向きへ飛ばす(当たった時刻を `punches` に積む)
/// - 入力待ち(`end`)になったら今の動作をやめ(伸びは最後まで)、最後の文字の右端へ寄って落ち着く
/// - `skip` したら、その時刻に最後の文字の右端へ移って落ち着く
pub(super) fn roam(
    page: &Page,
    appear: &[f32],
    p: &RoamParams,
    t: f32,
    skip_at: Option<f32>,
) -> Roam {
    let (fx, fl) = final_position(page);
    let mut punches: Vec<Hit> = Vec::new();
    let Some(first_line) = page.lines.iter().position(|l| l.glyph_count > 0) else {
        return Roam {
            x: fx,
            line: fl,
            facing: Facing::Right,
            action: CatAction::Idle,
            since: 0.0,
            settled_at: Some(0.0),
            punches,
        };
    };
    let n = page.glyphs.len().min(appear.len());
    let appear = &appear[..n];
    let shown_at = |tau: f32| appear.partition_point(|a| *a <= tau);
    let mut rng = XorShift::new(p.seed ^ (p.page as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let mut line = first_line;
    let mut x = line_start(page, line);
    let mut facing = Facing::Right;
    let mut tau = 0.0f32;
    // 入力待ち(または skip)で、ちょっかいを切り上げる時刻
    let stop_at = skip_at.map_or(p.end, |s| s.min(p.end));
    macro_rules! at_t {
        ($x:expr, $action:expr, $since:expr) => {
            return Roam {
                x: $x,
                line,
                facing,
                action: $action,
                since: $since,
                settled_at: None,
                punches,
            }
        };
    }
    let mut steps = 0u32;
    loop {
        // 動作を1つずつ時刻 0 から積むので、極端に長いページ(とても長い待機・遅い文字送り)では
        // 回数が増え、時刻が大きすぎると `tau` が進まなくなる。決めた回数を超えたら落ち着かせる
        steps += 1;
        if steps > MAX_STEPS || !tau.is_finite() {
            let s = if tau.is_finite() { tau } else { 0.0 };
            return Roam {
                x: fx,
                line: fl,
                facing: Facing::Right,
                action: CatAction::Idle,
                since: s,
                settled_at: Some(s),
                punches,
            };
        }
        if let Some(s) = skip_at.filter(|s| tau >= *s) {
            return Roam {
                x: fx,
                line: fl,
                facing: Facing::Right,
                action: CatAction::Idle,
                since: s,
                settled_at: Some(s),
                punches,
            };
        }
        if tau >= p.end {
            // 入力待ち: 最後の文字の行へ跳び移り、右端へ寄る
            if line != fl {
                line = fl;
                if let Some((lo, hi)) = walk_range(page, n, fl) {
                    x = x.clamp(lo, hi);
                }
            }
            let d = fx - x;
            if d.abs() > EPS {
                facing = facing_of(d, facing);
                let secs = walk_secs(d, p.speed * 0.8);
                if t < tau + secs {
                    at_t!(x + d * ease((t - tau) / secs), CatAction::Walk, tau);
                }
                tau += secs;
                x = fx;
            }
            return Roam {
                x,
                line,
                facing: Facing::Right,
                action: CatAction::Idle,
                since: tau,
                settled_at: Some(tau),
                punches,
            };
        }
        let shown = shown_at(tau);
        if shown == 0 {
            // まだ文字が出ていない: 最初の文字が出るまで待つ
            let next = appear
                .first()
                .copied()
                .unwrap_or(stop_at)
                .min(stop_at)
                .max(tau + EPS);
            if t < next {
                at_t!(x, CatAction::Idle, tau);
            }
            tau = next;
            continue;
        }
        let newest = page.glyphs[shown - 1].line;
        // 新しい行に文字が出ていれば、ときどきそちらへ跳び移る
        if newest > line && rng.unit() < 0.35 {
            if let Some(l) = (line + 1..=newest).find(|&l| walk_range(page, shown, l).is_some()) {
                line = l;
            }
        }
        let Some((lo, hi)) = walk_range(page, shown, line) else {
            // 猫の行に文字がない(空行など): 文字のある行へ移る
            line = (0..=newest)
                .rev()
                .find(|&l| walk_range(page, shown, l).is_some())
                .unwrap_or(first_line);
            continue;
        };
        x = x.clamp(lo, hi);
        // 猫パンチ
        if rng.unit() < p.punch_chance {
            if let Some((range, ruby, cx)) = punch_target(page, shown, line, x, tau, &punches) {
                facing = facing_of(cx - x, facing);
                let hit = tau + PUNCH_HIT;
                if hit <= t && hit < stop_at {
                    punches.push((hit, range, ruby, Flight::Punch(facing)));
                }
                let stop = (tau + PUNCH_SECS).min(stop_at.max(tau + PUNCH_HIT));
                if t < stop {
                    at_t!(x, CatAction::Punch, tau);
                }
                tau = stop;
                continue;
            }
        }
        let kind = rng.unit();
        if kind < 0.5 {
            // 歩く。よく新しい文字のそばへ行く
            let target = if rng.unit() < 0.6 {
                hi - rng.range(0.0, 2.0)
            } else {
                rng.range(lo, hi)
            }
            .clamp(lo, hi);
            let d = target - x;
            if d.abs() >= 0.8 {
                let gait = rng.unit();
                let v = p.speed
                    * if gait < 0.5 {
                        0.4
                    } else if gait < 0.85 {
                        1.0
                    } else {
                        1.8
                    };
                let secs = walk_secs(d, v);
                facing = facing_of(d, facing);
                // 入力待ちになったら歩くのをやめて最後の文字へ向かう
                let stop = (tau + secs).min(stop_at);
                if t < stop {
                    at_t!(x + d * ease((t - tau) / secs), CatAction::Walk, tau);
                }
                x += d * ease((stop - tau) / secs);
                tau = stop;
                continue;
            }
        }
        let (action, secs) = if kind < 0.75 {
            (CatAction::Idle, rng.range(0.5, 2.0))
        } else if kind < 0.88 {
            (CatAction::Groom, rng.range(1.6, 3.0))
        } else {
            (CatAction::Stretch, STRETCH_SECS)
        };
        // 伸びは最後まで。ほかは入力待ちになったらやめる
        let stop = if action == CatAction::Stretch {
            tau + secs
        } else {
            (tau + secs).min(stop_at)
        };
        if t < stop {
            at_t!(x, action, tau);
        }
        tau = stop;
    }
}
