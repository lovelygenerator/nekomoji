//! 飛ばされた文字の動き(飛んで消え、少しして元の位置へ戻る)と、動作の長さの定数。

use std::ops::Range;

use super::cat::Facing;

/// 文字の飛ばされ方。
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Flight {
    /// 押されて横へ少し滑り、縁を越えて下へ落ちる
    Drop,
    /// 猫パンチで、叩かれた向きへ弧を描いて飛ぶ
    Punch(Facing),
    /// 爆散: 猫のいる所から外へ弾け飛ぶ(初速 x・y と回転の速さ。全角/秒・ラジアン/秒)
    Burst {
        /// 横の初速
        vx: f32,
        /// 縦の初速(下向きが正)
        vy: f32,
        /// 回転の速さ
        spin: f32,
    },
}

/// 飛ばされた文字の状態。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Knocked {
    /// 飛ぶ文字(ページ内の添字)。ルビ付きなら親文字すべて、縦中横なら塊ごと
    pub glyphs: Range<usize>,
    /// 一緒に飛ぶルビ([`Page::rubies`](crate::Page::rubies) の添字)
    pub ruby: Option<usize>,
    /// 飛ばされ方
    pub flight: Flight,
    /// 飛ばされてからの秒数
    pub since: f32,
    /// 今の動き
    pub fall: Fall,
}

/// 飛ぶ文字の動き(全角1文字を 1.0 とする単位)。
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Fall {
    /// ずれ(y は下向きが正)
    pub offset: (f32, f32),
    /// 回転(ラジアン)
    pub rotation: f32,
    /// 不透明度。飛んで消えると 0、戻るときに 0 から 1 へ
    pub alpha: f32,
    /// 飛んで見えなくなっているか(戻り始める前)
    pub gone: bool,
}

/// 入力待ちで猫が最後の文字のそばに着いてから、押す・爆散させるまで(秒)
pub(crate) const KNOCK_HIT: f32 = 0.3;
/// 押す・爆散させる動作の長さ(秒)
pub(crate) const KNOCK_SECS: f32 = 0.6;
/// 猫パンチの動作の長さ(秒)
pub(crate) const PUNCH_SECS: f32 = 0.4;
/// 猫パンチを始めてから文字に当たるまで(秒)
pub(crate) const PUNCH_HIT: f32 = 0.2;
/// 文字が飛んで消えるまで(秒)
pub(crate) const FALL_SECS: f32 = 1.0;
/// 消えてから戻り始めるまで(秒)
pub(crate) const RETURN_WAIT: f32 = 1.2;
/// 戻るフェードインの長さ(秒)
pub(crate) const RETURN_SECS: f32 = 0.6;
/// 飛ばされてから元に戻り終えるまで(秒)
pub(crate) const FLIGHT_TOTAL: f32 = FALL_SECS + RETURN_WAIT + RETURN_SECS;
/// 伸びをする動作の長さ(秒)
pub(crate) const STRETCH_SECS: f32 = 1.2;

impl Flight {
    /// 飛ばされてから `s` 秒後の動き。戻り終えたら `None`。
    pub fn at(self, s: f32) -> Option<Fall> {
        fall_at(self, s)
    }
}

/// 飛ばされてから `s` 秒後の動き。戻り終えたら `None`。
pub(crate) fn fall_at(flight: Flight, s: f32) -> Option<Fall> {
    let s = s.max(0.0);
    if s >= FLIGHT_TOTAL {
        return None;
    }
    if s >= FALL_SECS {
        // 消えている間は元の位置で透明。戻るときはその場でフェードイン
        let back = ((s - FALL_SECS - RETURN_WAIT) / RETURN_SECS).clamp(0.0, 1.0);
        return Some(Fall {
            offset: (0.0, 0.0),
            rotation: 0.0,
            alpha: back,
            gone: back <= 0.0,
        });
    }
    // 最後の 0.3 秒で薄くなる
    let alpha = ((FALL_SECS - s) / 0.3).clamp(0.0, 1.0);
    let (offset, rotation) = match flight {
        Flight::Drop => {
            const SLIDE: f32 = 0.2;
            let slide = s.min(SLIDE) / SLIDE;
            let drop = (s - SLIDE).max(0.0);
            ((0.5 * slide + 0.6 * drop, 9.0 * drop * drop), 2.5 * drop)
        }
        Flight::Punch(f) => {
            let d = f.sign();
            ((d * 5.0 * s, -3.0 * s + 7.0 * s * s), d * 7.0 * s)
        }
        Flight::Burst { vx, vy, spin } => ((vx * s, vy * s + 6.0 * s * s), spin * s),
        #[allow(unreachable_patterns)]
        _ => ((0.0, 0.0), 0.0),
    };
    Some(Fall {
        offset,
        rotation,
        alpha,
        gone: false,
    })
}
