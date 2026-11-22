//! 時間を進める・skip・入力待ちから進める、と今の状態の問い合わせ。

use super::options::Motion;
use super::output::{StepKind, WaitKind};
use super::Reveal;
use crate::markup::TextEvent;

impl Reveal {
    /// 入力待ちなら、その種類。文字送りの途中なら `None`。
    pub fn wait_kind(&self) -> Option<WaitKind> {
        if self.finished {
            Some(WaitKind::End)
        } else if !self.waiting {
            None
        } else if self.waiting_click() {
            Some(WaitKind::Click)
        } else {
            Some(WaitKind::Page)
        }
    }

    /// 動きの強さ([`RevealOptions::motion`](crate::RevealOptions::motion))。
    pub fn motion(&self) -> Motion {
        self.motion
    }

    /// 動きの強さを変える。時間割は変わらないので、文字送りの途中でもそのまま切り替わる。
    pub fn set_motion(&mut self, motion: Motion) {
        self.motion = motion;
    }

    /// `dt` 秒進め、その間に起きた出来事を返す。
    ///
    /// 入力待ち・終了中も時刻は進む(エフェクトや猫の動きが止まらないように)が、
    /// 文字と出来事は増えない。`dt` が負・NaN・無限大なら時刻は進めない。
    pub fn advance(&mut self, dt: f32) -> Vec<TextEvent> {
        // 負・NaN・無限大の経過時間では時刻を動かさない(巻き戻したり、NaN のまま戻らなくなったりしない)
        if dt.is_finite() && dt > 0.0 {
            self.time += dt;
        }
        if self.waiting || self.finished {
            return Vec::new();
        }
        self.run_until(self.time)
    }

    /// 次の入力待ち(`[l]`・ページの終わり)の手前まで一度に表示する。途中の出来事も返す。
    pub fn skip(&mut self) -> Vec<TextEvent> {
        if self.waiting || self.finished {
            return Vec::new();
        }
        let next_wait = self
            .page_clicks()
            .get(self.resumed.len())
            .map(|c| c.step)
            .unwrap_or_else(|| self.timelines[self.page].len().saturating_sub(1));
        let end = self.step_time(next_wait);
        self.time = self.time.max(end);
        self.skipped = true;
        let mut out = self.run_until(f32::INFINITY);
        out.retain(|e| !matches!(e, TextEvent::Blip(_)));
        out
    }

    /// 時刻 `until` までの出来事を返す。文字送りの音は最後の1つだけ残す。
    pub(super) fn run_until(&mut self, until: f32) -> Vec<TextEvent> {
        let mut out = self.run_steps(until);
        if let Some(last) = out.iter().rposition(|e| matches!(e, TextEvent::Blip(_))) {
            let mut i = 0;
            out.retain(|e| {
                let keep = !matches!(e, TextEvent::Blip(_)) || i == last;
                i += 1;
                keep
            });
        }
        out
    }

    fn run_steps(&mut self, until: f32) -> Vec<TextEvent> {
        let mut out = Vec::new();
        while let Some(step) = self.timelines[self.page].get(self.next) {
            if self.step_time(self.next) > until {
                break;
            }
            let kind = step.kind.clone();
            let index = self.next;
            self.next += 1;
            match kind {
                StepKind::Show(i) => self.shown = i + 1,
                StepKind::Emit(e) => {
                    match e {
                        TextEvent::WaitInput => self.waiting = true,
                        TextEvent::Finished => self.finished = true,
                        TextEvent::WaitClick => {
                            // 復元のときなど、もう進んだ [l] は待たない
                            let k = self
                                .page_clicks()
                                .iter()
                                .position(|c| c.step == index)
                                .unwrap_or(0);
                            if k < self.resumed.len() {
                                continue;
                            }
                            self.waiting = true;
                            out.push(e);
                            break;
                        }
                        _ => {}
                    }
                    out.push(e);
                }
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }
        out
    }

    /// 入力待ちなら先へ進む。進んだら `true`。
    ///
    /// ページの途中の入力待ち(`[l]`)では同じページの続きを、ページの終わりでは次のページを出す。
    /// 続きの文字は、待った長さだけ後ろへずれる(`[l]` を待った長さの `[wait]` として扱う)。
    pub fn proceed(&mut self) -> bool {
        if !self.waiting {
            return false;
        }
        if self.waiting_click() {
            self.resumed.push(self.time);
            self.waiting = false;
            self.skipped = false;
            return true;
        }
        self.waiting = false;
        self.page += 1;
        self.time = 0.0;
        self.next = 0;
        self.shown = 0;
        self.skipped = false;
        self.resumed.clear();
        true
    }

    /// 今の区切り(ページの始めか、最後に進んだ `[l]` の後)で [`Reveal::skip`] を使ったか。
    pub fn skipped(&self) -> bool {
        self.skipped
    }

    /// ページの数(空の文でも 1)。
    pub fn page_count(&self) -> usize {
        self.timelines.len()
    }

    /// 今のページの番号(0 始まり)。
    pub fn page(&self) -> usize {
        self.page
    }

    /// 今のページで表示済みの文字数。
    pub fn visible_count(&self) -> usize {
        self.shown
    }

    /// ページの始めからの経過時間(秒)。
    pub fn time(&self) -> f32 {
        self.time
    }

    /// 入力待ち(`[l]`・`[p]`)か。種類は [`Reveal::wait_kind`]。
    pub fn is_waiting(&self) -> bool {
        self.waiting
    }

    /// すべて表示し終えたか。
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}
