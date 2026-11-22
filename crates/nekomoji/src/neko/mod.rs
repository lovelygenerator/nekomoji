//! 猫モード: 見えない猫が文字にちょっかいを出す。
//!
//! 猫の絵は出さない。画面には見えない猫が1匹いて、表示済みの文字の上を気ままに歩き回る
//! (緩急のある歩き・立ち止まる・伸び・毛づくろい・別の行へ跳び移る)。ちょっかいは3つ:
//!
//! - **猫パンチ**: 文字送り中、通りすがりにそばの文字を叩き、歩いている向きへ飛ばす
//! - **落とす**: 入力待ちになると最後の文字へ寄り、前脚で押して落とす
//! - **爆散**: 入力待ちで、ときどきページの文字をまとめて弾き飛ばす
//!
//! 飛んだ文字は消えたあと、少しして元の位置へフェードインで戻る(読めなくならないように)。
//! テキスト(バックログ・`log` 出力)は変えない。何をいつするかは (シード, ページ番号) の乱数で
//! 決め、すべて [`Layout`] と [`Reveal`] の状態(時刻)の関数になる。フレームごとの積分はしない。
//!
//! 猫の出し方(`[neko hop]`)はエフェクトなので [`crate::Reveal::glyphs`] が扱う。
//! 入力待ちのしるし(カーソル)の位置は [`crate::Reveal::cursor`] が出す。

// - `mode`: 設定(`NekoMode`)
// - `cat`: 見えない猫の状態(`CatState`)
// - `roaming`: 見えない猫の歩き回り(時刻の関数)
// - `hit`: ちょっかいの対象(猫パンチ・押して落とす・爆散で飛ぶ文字の塊)
// - `flight`: 飛ばされた文字の動き(`Knocked`・`Fall`)
// - `rng`: シード付きの決定的な乱数

mod cat;
mod flight;
mod hit;
mod mode;
mod rng;
mod roaming;
#[cfg(test)]
mod tests;

pub use cat::{CatAction, CatState, Facing};
pub use flight::{Fall, Flight, Knocked};
pub use mode::NekoMode;

use std::ops::Range;

use crate::layout::{Layout, Page};
use crate::reveal::{GlyphInstance, Motion, Reveal};
use flight::{fall_at, FLIGHT_TOTAL, KNOCK_HIT, KNOCK_SECS};
use hit::{burst, knock_target, unit_of};
use rng::page_random;
use roaming::{roam, Roam, RoamParams};

/// 入力待ちでのちょっかい。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum WaitPlan {
    /// 最後の文字(ページ内の添字。ルビ付きなら親文字すべて)を押して落とす
    Drop(Range<usize>),
    /// ページの文字をまとめて爆散させる
    Burst,
}

/// 猫モードの計算。[`Layout`] ごとに作る(ページごとの入力待ちのちょっかいを先に決める)。
#[derive(Debug, Clone)]
pub struct Neko {
    mode: NekoMode,
    plans: Vec<Option<WaitPlan>>,
}

/// ちょっかい1つ: (当たる時刻, 文字の範囲, ルビ, 飛ばされ方)
type Hit = (f32, Range<usize>, Option<usize>, Flight);

impl Neko {
    /// 入力待ちのちょっかいを (シード, ページ番号) の乱数で決める。経過時間や `skip` には依存しない。
    pub fn new(layout: &Layout, mode: NekoMode) -> Neko {
        let plans = layout
            .pages
            .iter()
            .enumerate()
            .map(|(pi, page)| {
                if !mode.walk {
                    return None;
                }
                let r = page_random(mode.seed, pi);
                if r < mode.burst_chance {
                    page.glyphs
                        .iter()
                        .any(|g| !g.ch.is_whitespace())
                        .then_some(WaitPlan::Burst)
                } else if r < mode.burst_chance + mode.knock_chance {
                    knock_target(page).map(|(r, _)| WaitPlan::Drop(r))
                } else {
                    None
                }
            })
            .collect();
        Neko { mode, plans }
    }

    /// 設定。
    pub fn mode(&self) -> &NekoMode {
        &self.mode
    }

    /// そのページの入力待ちでのちょっかい。何もしないなら `None`。
    ///
    /// 動きの強さ([`Motion`])で抑える: 控えめでは爆散しない、なしでは何もしない。
    pub fn wait_plan(&self, page: usize, motion: Motion) -> Option<&WaitPlan> {
        let plan = self.plans.get(page)?.as_ref()?;
        match (motion, plan) {
            (Motion::Full, _) | (Motion::Reduced, WaitPlan::Drop(_)) => Some(plan),
            _ => None,
        }
    }

    /// 猫が入力待ちで最後の文字のそばに着く時刻(ページの始めからの秒。`skip` なしのとき)。
    /// 押す・爆散させるのは [`Neko::knock_time`]。
    pub fn settle_time(&self, layout: &Layout, reveal: &Reveal, page: usize) -> f32 {
        let end = reveal.page_end(page);
        let Some(p) = layout.pages.get(page) else {
            return end;
        };
        if !self.mode.walk {
            return end;
        }
        let w = self.roam(
            p,
            page,
            &reveal.appear_times(page),
            end,
            f32::INFINITY,
            None,
            reveal.motion(),
        );
        w.settled_at.unwrap_or(end).max(end)
    }

    /// 入力待ちで猫が最後の文字を押す・爆散させる時刻(ページの始めからの秒。`skip` なしのとき)。
    /// [`Neko::settle_time`] の 0.3 秒後。ちょっかいをするかどうかは [`Neko::wait_plan`] で決まる。
    pub fn knock_time(&self, layout: &Layout, reveal: &Reveal, page: usize) -> f32 {
        self.settle_time(layout, reveal, page) + KNOCK_HIT
    }

    /// そのページで飛ばされた文字がすべて戻り終える時刻(`skip` なしのとき)。
    /// 入力待ちのちょっかいがなければ、猫が最後の文字のそばに着く時刻。
    pub fn quiet_time(&self, layout: &Layout, reveal: &Reveal, page: usize) -> f32 {
        let settle = self.settle_time(layout, reveal, page);
        match self.wait_plan(page, reveal.motion()) {
            Some(_) => settle + KNOCK_HIT + FLIGHT_TOTAL,
            None => settle,
        }
    }

    /// そのページで文字送り中に出る猫パンチの一覧(当たる時刻・文字の範囲・飛ぶ向き)。
    /// `skip` なしのとき。確認・ログ用。
    pub fn punches(
        &self,
        layout: &Layout,
        reveal: &Reveal,
        page: usize,
    ) -> Vec<(f32, Range<usize>, Facing)> {
        let Some(p) = layout.pages.get(page) else {
            return Vec::new();
        };
        if !self.mode.walk {
            return Vec::new();
        }
        let end = reveal.page_end(page);
        self.roam(
            p,
            page,
            &reveal.appear_times(page),
            end,
            f32::INFINITY,
            None,
            reveal.motion(),
        )
        .punches
        .into_iter()
        .map(|(at, r, _, f)| match f {
            Flight::Punch(d) => (at, r, d),
            _ => (at, r, Facing::Right),
        })
        .collect()
    }

    /// 今の見えない猫の状態。ちょっかいを出さない設定なら `None`。
    pub fn cat(&self, layout: &Layout, reveal: &Reveal) -> Option<CatState> {
        if !self.mode.walk {
            return None;
        }
        let pi = reveal.page();
        let page = layout.pages.get(pi)?;
        let t = reveal.time();
        let w = self.roam_now(layout, reveal)?;
        let mut st = CatState {
            x: w.x,
            line: w.line,
            over_ruby: page.rubies.iter().any(|r| r.line == w.line),
            facing: w.facing,
            action: w.action,
            action_time: (t - w.since).max(0.0),
        };
        if let Some(done) = w.settled_at {
            let u = t - done;
            let plan = self.wait_plan(pi, reveal.motion());
            if plan.is_some() && u < KNOCK_SECS {
                st.action = CatAction::Knock;
                st.action_time = u;
            } else {
                let start = if plan.is_some() {
                    done + KNOCK_SECS
                } else {
                    done
                };
                st.action = CatAction::Idle;
                st.action_time = (t - start).max(0.0);
            }
        }
        Some(st)
    }

    /// 今飛んでいる・消えている・戻りかけの文字。当たった順。
    pub fn knocked(&self, layout: &Layout, reveal: &Reveal) -> Vec<Knocked> {
        let t = reveal.time();
        self.hits(layout, reveal)
            .into_iter()
            .filter_map(|(at, glyphs, ruby, flight)| {
                let since = t - at;
                if since < 0.0 {
                    return None;
                }
                let fall = fall_at(flight, since)?;
                Some(Knocked {
                    glyphs,
                    ruby,
                    flight,
                    since,
                    fall,
                })
            })
            .collect()
    }

    /// [`Reveal::glyphs`] に、飛ばされた文字の動きを足したもの。テキストそのものは変えない。
    /// 同じ文字が二度飛ばされていたら、後のほうを使う。
    /// 毎フレーム呼ぶなら、バッファを使い回せる [`Neko::glyphs_into`] のほうが割り当てが少ない。
    pub fn glyphs(&self, layout: &Layout, reveal: &Reveal) -> Vec<GlyphInstance> {
        let mut out = Vec::new();
        self.glyphs_into(layout, reveal, &mut out);
        out
    }

    /// [`Neko::glyphs`] と同じ描画命令を `out` に書く。`out` は最初に空にし、確保済みの領域は使い回す。
    pub fn glyphs_into(&self, layout: &Layout, reveal: &Reveal, out: &mut Vec<GlyphInstance>) {
        reveal.glyphs_into(layout, out);
        let knocked = self.knocked(layout, reveal);
        if knocked.is_empty() {
            return;
        }
        let gs = out;
        let mut done = vec![false; gs.len()];
        for k in knocked.iter().rev() {
            for i in k.glyphs.clone() {
                let (Some(g), Some(d)) = (gs.get_mut(i), done.get_mut(i)) else {
                    continue;
                };
                if *d {
                    continue;
                }
                *d = true;
                g.offset.0 += k.fall.offset.0;
                g.offset.1 += k.fall.offset.1;
                g.rotation += k.fall.rotation;
                g.alpha *= k.fall.alpha;
            }
        }
    }

    /// 今のページで起きた(起きる)ちょっかいの一覧。猫パンチと、入力待ちのちょっかい。
    fn hits(&self, layout: &Layout, reveal: &Reveal) -> Vec<Hit> {
        let Some(w) = self.roam_now(layout, reveal) else {
            return Vec::new();
        };
        let pi = reveal.page();
        let mut hits = w.punches;
        if let (Some(done), Some(plan), Some(page)) = (
            w.settled_at,
            self.wait_plan(pi, reveal.motion()),
            layout.pages.get(pi),
        ) {
            let at = done + KNOCK_HIT;
            match plan {
                WaitPlan::Drop(range) => {
                    let ruby = unit_of(page, range.start).1;
                    hits.push((at, range.clone(), ruby, Flight::Drop));
                }
                WaitPlan::Burst => {
                    let seed = self.mode.seed ^ 0x6275_7273_7421;
                    hits.extend(burst(page, seed, pi, w.x, w.line, at));
                }
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }
        hits
    }

    fn roam_now(&self, layout: &Layout, reveal: &Reveal) -> Option<Roam> {
        if !self.mode.walk {
            return None;
        }
        let pi = reveal.page();
        let page = layout.pages.get(pi)?;
        let end = reveal.page_end(pi);
        // skip したら猫は最後の位置へ即座に移る
        let skip_at = reveal.skipped().then_some(end);
        Some(self.roam(
            page,
            pi,
            &reveal.appear_times(pi),
            end,
            reveal.time(),
            skip_at,
            reveal.motion(),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn roam(
        &self,
        page: &Page,
        pi: usize,
        appear: &[f32],
        end: f32,
        t: f32,
        skip_at: Option<f32>,
        motion: Motion,
    ) -> Roam {
        // 入力待ちのちょっかいとは別の乱数の列を使う
        let seed = self.mode.seed ^ 0x6e65_6b6f_7761_6c6b;
        // 猫パンチは標準のときだけ
        let punch_chance = if motion == Motion::Full {
            self.mode.punch_chance
        } else {
            0.0
        };
        roam(
            page,
            appear,
            &RoamParams {
                speed: self.mode.walk_speed,
                punch_chance,
                seed,
                page: pi,
                end,
            },
            t,
            skip_at,
        )
    }
}
