//! 時間割を作り、ページ・文字・合図の時刻を出す。

use super::blip::BlipCounter;
use super::options::RevealOptions;
use super::output::{Step, StepKind};
use super::{Click, Reveal, CLICK_EPS};
use crate::layout::{Cue, Layout};
use crate::markup::TextEvent;

/// 文字 `ch` の後の間。`next` は次の文字(`1,000` の `,` のような数の中の区切りでは間を置かない)。
/// 負・NaN の設定は 0 とみなす。
fn pause_after(ch: char, next: Option<char>, opts: &RevealOptions) -> f32 {
    let secs = match ch {
        ',' if next.is_some_and(|c| c.is_ascii_digit()) => 0.0,
        '、' | '，' | ',' | '…' | '‥' => opts.comma_pause,
        '。' | '．' | '！' | '？' | '!' | '?' => opts.period_pause,
        _ => 0.0,
    };
    finite_secs(secs)
}

/// 句読点の間を後ろへ送る文字(句読点・感嘆符・三点リーダー・終わり括弧・終わり引用符)。
fn trails_pause(ch: char) -> bool {
    use crate::text::line_class::{line_class, LineClass};
    matches!(ch, '…' | '‥' | '"' | '\'')
        || matches!(line_class(ch), LineClass::Close | LineClass::Exclam)
        || matches!(ch, '、' | '。' | '，' | '．')
}

/// 秒数として使える値にする。負・NaN・無限大は 0。
fn finite_secs(secs: f32) -> f32 {
    if secs.is_finite() {
        secs.max(0.0)
    } else {
        0.0
    }
}

impl Reveal {
    /// 時間割を作る。
    pub fn new(layout: &Layout, opts: RevealOptions) -> Reveal {
        let last = layout.pages.len().saturating_sub(1);
        let mut timelines = Vec::new();
        let mut appear = Vec::new();
        let mut all_clicks = Vec::new();
        for (pi, page) in layout.pages.iter().enumerate() {
            let mut steps = Vec::new();
            let mut times = Vec::with_capacity(page.glyphs.len());
            let mut t = 0.0;
            let mut blips = BlipCounter::default();
            let mut clicks: Vec<Click> = Vec::new();
            let mut cues = page.cues.iter().peekable();
            let glyph_count = page.glyphs.len();
            let mut run_cues = |n: usize, t: &mut f32, steps: &mut Vec<Step>| {
                while let Some((_, cue)) = cues.next_if(|(at, _)| *at == n) {
                    match cue {
                        Cue::Wait(s) => *t += s,
                        Cue::Event(e) => steps.push(Step {
                            at: *t,
                            kind: StepKind::Emit(e.clone()),
                        }),
                        // ページの途中の入力待ち。後ろに文字が無ければ(ページの終わりの入力待ちと重なるので)
                        // 待たない。同じ位置に続けて書いても1度だけ待つ
                        Cue::Click
                            if n < glyph_count && !clicks.last().is_some_and(|c| c.glyph == n) =>
                        {
                            clicks.push(Click {
                                step: steps.len(),
                                at: *t,
                                glyph: n,
                            });
                            steps.push(Step {
                                at: *t,
                                kind: StepKind::Emit(TextEvent::WaitClick),
                            });
                        }
                        _ => {}
                    }
                }
            };
            let mut held = 0.0_f32;
            for (gi, g) in page.glyphs.iter().enumerate() {
                run_cues(gi, &mut t, &mut steps);
                steps.push(Step {
                    at: t,
                    kind: StepKind::Show(gi),
                });
                times.push(t);
                let style = layout.styles.get(g.style);
                if let Some(id) = blips.next(g.ch, t, style, &opts) {
                    steps.push(Step {
                        at: t,
                        kind: StepKind::Emit(TextEvent::Blip(id)),
                    });
                }
                let speed = layout.styles.get(g.style).map_or(1.0, |s| s.speed);
                // 1秒あたりの文字数が 0 以下・NaN なら、待たずに表示する
                t += finite_secs(1.0 / (opts.chars_per_sec * speed));
                // 句読点の間は、続く句読点・閉じ括弧(`。」` `！？`)の最後の文字の後に1度だけ置く
                // 次の文字の前に合図(待機・クリック待ち・イベント)があるとき、次の文字が段落の頭のときは送らない
                let next = page.glyphs.get(gi + 1).map(|n| n.ch);
                held = held.max(pause_after(g.ch, next, &opts));
                let boundary = page.cues.iter().any(|(at, _)| *at == gi + 1)
                    || page
                        .lines
                        .iter()
                        .any(|l| l.paragraph_start && l.first_glyph == gi + 1);
                if boundary || !next.is_some_and(trails_pause) {
                    t += held;
                    held = 0.0;
                }
            }
            run_cues(page.glyphs.len(), &mut t, &mut steps);
            let end = if pi == last {
                TextEvent::Finished
            } else {
                TextEvent::WaitInput
            };
            steps.push(Step {
                at: t,
                kind: StepKind::Emit(end),
            });
            timelines.push(steps);
            appear.push(times);
            all_clicks.push(clicks);
        }
        if timelines.is_empty() {
            timelines.push(vec![Step {
                at: 0.0,
                kind: StepKind::Emit(TextEvent::Finished),
            }]);
            appear.push(Vec::new());
            all_clicks.push(Vec::new());
        }
        Reveal {
            timelines,
            appear,
            page: 0,
            time: 0.0,
            next: 0,
            shown: 0,
            waiting: false,
            finished: false,
            skipped: false,
            motion: opts.motion,
            chars: layout
                .pages
                .iter()
                .map(|p| {
                    let mut acc = vec![0];
                    for g in &p.glyphs {
                        let n = acc[acc.len() - 1] + usize::from(!g.ch.is_whitespace());
                        acc.push(n);
                    }
                    acc
                })
                .collect(),
            auto: opts.auto,
            clicks: all_clicks,
            resumed: Vec::new(),
        }
    }

    /// 今のページの `[l]` の一覧。
    pub(super) fn page_clicks(&self) -> &[Click] {
        self.clicks.get(self.page).map_or(&[], |v| v.as_slice())
    }

    /// `[l]` で待っているか。
    pub(super) fn waiting_click(&self) -> bool {
        self.waiting && self.resumed.len() < self.page_clicks().len()
    }

    /// 今のページで、`k` 個の `[l]` の後ろにあるもの(時間割・グリフ)の時刻に足すずれ(秒)。
    ///
    /// 進んだ `[l]` は待った長さ、待っている `[l]` は今までの長さ(と少し)だけ後ろへずらす。
    pub(super) fn shift_after(&self, k: usize) -> f32 {
        if k == 0 {
            return 0.0;
        }
        let clicks = self.page_clicks();
        let done = self.resumed.len().min(clicks.len());
        if k <= done {
            return (self.resumed[k - 1] - clicks[k - 1].at).max(0.0);
        }
        let prior = if done == 0 {
            0.0
        } else {
            (self.resumed[done - 1] - clicks[done - 1].at).max(0.0)
        };
        if self.waiting_click() {
            let c = clicks[done];
            prior.max(self.time - c.at) + CLICK_EPS
        } else {
            prior
        }
    }

    /// 今のページの時間割の `step` 番目の、実際の時刻。
    pub(super) fn step_time(&self, step: usize) -> f32 {
        let at = self.timelines[self.page].get(step).map_or(0.0, |s| s.at);
        let k = self.page_clicks().iter().filter(|c| c.step < step).count();
        at + self.shift_after(k)
    }

    /// 今のページの `glyph` 番目のグリフが出る、実際の時刻。
    pub(super) fn glyph_time(&self, glyph: usize) -> f32 {
        let at = self.appear[self.page].get(glyph).copied().unwrap_or(0.0);
        let k = self
            .page_clicks()
            .iter()
            .filter(|c| c.glyph <= glyph)
            .count();
        at + self.shift_after(k)
    }

    /// ページの最後(入力待ち・終了)の時刻。
    ///
    /// 今のページでは、`[l]` で待った長さ(待っている途中なら今までの長さ)だけ後ろへずれる。
    pub fn page_end(&self, page: usize) -> f32 {
        let Some(t) = self.timelines.get(page) else {
            return 0.0;
        };
        if page == self.page {
            self.step_time(t.len().saturating_sub(1))
        } else {
            t.last().map_or(0.0, |s| s.at)
        }
    }

    /// ページのグリフごとの表示時刻(ページの始めからの秒)。
    ///
    /// 今のページでは、`[l]` で待った長さ(待っている途中なら今までの長さ)だけ後ろへずれる。
    pub fn appear_times(&self, page: usize) -> Vec<f32> {
        if page == self.page {
            (0..self.appear[page].len())
                .map(|i| self.glyph_time(i))
                .collect()
        } else {
            self.appear.get(page).cloned().unwrap_or_default()
        }
    }

    /// ページの時間割(確認・ログ用)。
    pub fn timeline(&self, page: usize) -> &[Step] {
        self.timelines.get(page).map_or(&[], |v| v.as_slice())
    }
}
