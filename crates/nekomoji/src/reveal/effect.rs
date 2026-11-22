//! エフェクト(揺れ・波・猫の出し方・発光)を時刻の関数で計算する。乱数は使わない。

use super::options::{Motion, REDUCED_DECAY_SECS, STEADY_GLOW};
use crate::markup::GlowKind;

/// `[neko hop]` で文字が跳ねて現れるまでの秒数
pub(super) const HOP_SECS: f32 = 0.35;

/// 揺れ・波の振れ幅の倍率。`decay` 秒かけて 1 から 0 へ、終わりほどゆっくり弱まる。
/// `None` なら弱まらない。
/// 控えめのときは、揺れ・波が 1 秒以内に止まるようにする。
pub(super) fn motion_decay(decay: Option<f32>, motion: Motion) -> Option<f32> {
    match motion {
        Motion::Reduced => Some(decay.map_or(REDUCED_DECAY_SECS, |d| d.min(REDUCED_DECAY_SECS))),
        _ => decay,
    }
}

pub(super) fn fading(age: f32, decay: Option<f32>) -> f32 {
    match decay {
        Some(secs) if secs > 0.0 => {
            let rest = (1.0 - age / secs).clamp(0.0, 1.0);
            rest * rest
        }
        _ => 1.0,
    }
}

/// 光り始めにかける秒数
pub(super) const GLOW_RISE: f32 = 0.15;
/// 発光 `flicker` の瞬きの最短の間隔(標準)。1 秒に 3 度を超えないように
const FLICKER_WINDOW: f32 = 0.35;
/// 発光 `flicker` の瞬きの最短の間隔(控えめ)。1 秒に 1 度を超えないように
const FLICKER_WINDOW_REDUCED: f32 = 1.05;

/// 発光の強さ(0〜1)と、文字の明るさの倍率。`age` は文字が出てからの秒数、
/// `t` はページの時刻、`key` は同じ範囲の文字で揃えて瞬かせるための値(スタイルの番号)。
pub(super) fn glow_at(
    kind: GlowKind,
    age: f32,
    secs: f32,
    t: f32,
    key: u32,
    motion: Motion,
) -> (f32, f32) {
    let secs = secs.max(0.01);
    let rise = (age / GLOW_RISE).clamp(0.0, 1.0);
    let rise = rise * rise * (3.0 - 2.0 * rise);
    if motion == Motion::Off {
        // 明滅させず、一定の明るさで光る(出たときだけ 0.15 秒でなめらかに)
        return (STEADY_GLOW * rise, 1.0);
    }
    match kind {
        GlowKind::In => {
            if age < GLOW_RISE {
                (rise, 1.0)
            } else {
                let rest = (1.0 - (age - GLOW_RISE) / secs).clamp(0.0, 1.0);
                (rest * rest, 1.0)
            }
        }
        GlowKind::Breathe => {
            let phase = (age / secs) * std::f32::consts::TAU;
            let wave = 0.5 - 0.5 * phase.cos();
            let low = if motion == Motion::Reduced { 0.6 } else { 0.25 };
            (rise * (low + (1.0 - low) * wave), 1.0)
        }
        GlowKind::Flicker => {
            // 時間を窓(標準 0.35 秒・控えめ 1.05 秒)に区切り、瞬くのは窓の始めの 0.05 秒だけ。
            // 瞬きの間は窓の長さより縮まないので、どの 1 秒を取っても標準で 3 度、控えめで 1 度を
            // 超えて明滅しない(光過敏への配慮)。窓ごとに平均 secs 秒に1度の割合で瞬く
            // (範囲ごとに揃えて瞬く)
            let window_secs = if motion == Motion::Reduced {
                FLICKER_WINDOW_REDUCED
            } else {
                FLICKER_WINDOW
            };
            let window = (t / window_secs) as u32;
            let r = (noise(key ^ 0x6e65_6f6e, window) + 1.0) / 2.0;
            let flash = r < window_secs / secs && t - window as f32 * window_secs < 0.05;
            // 瞬きの間のゆらぎは、なめらかに変わるだけにする(段差のある明滅を増やさない)。
            // 標準は 1 秒に 3 度・振れ幅 ±15%、控えめは 1 秒に 1 度・±5% の速さで値が移る
            let (rate, amp) = if motion == Motion::Reduced {
                (1.0, 0.05)
            } else {
                (3.0, 0.15)
            };
            let shimmer = smooth_noise(key, t * rate);
            match (flash, motion) {
                (true, Motion::Reduced) => (0.5 * rise, 1.0),
                (true, _) => (0.05 * rise, 0.35),
                _ => (rise * (1.0 - amp + amp * shimmer), 1.0),
            }
        }
        #[allow(unreachable_patterns)]
        _ => (0.0, 1.0),
    }
}

/// 整数の点ごとの [`noise`] を、点の間でなめらかにつないだ -1.0〜1.0 の値。
fn smooth_noise(key: u32, x: f32) -> f32 {
    let x = x.max(0.0);
    let i = x as u32;
    let f = x - i as f32;
    let f = f * f * (3.0 - 2.0 * f);
    let a = noise(key, i);
    let b = noise(key, i.wrapping_add(1));
    a + (b - a) * f
}

/// (a, b) から決まる -1.0〜1.0 の値(簡易ハッシュ)。
pub(super) fn noise(a: u32, b: u32) -> f32 {
    let mut x = a.wrapping_mul(0x85eb_ca6b) ^ b.wrapping_mul(0xc2b2_ae35);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    (x % 2001) as f32 / 1000.0 - 1.0
}
