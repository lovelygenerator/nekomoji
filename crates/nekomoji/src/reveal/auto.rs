//! オートモードの待ち時間。

use super::output::StepKind;
use super::Reveal;
use crate::markup::TextEvent;

impl Reveal {
    /// 区切り `seg`(0 はページの始めから最初の `[l]` まで、最後はページの終わりまで)の
    /// オートモードの待ち時間。`start` はその区切りの始めの時間割の添字。
    fn auto_wait_seg(
        &self,
        page: usize,
        seg: usize,
        voice_len: impl Fn(&str) -> Option<f32>,
    ) -> f32 {
        let a = &self.auto;
        let ok = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        let clicks = self.clicks.get(page).map_or(&[][..], |v| v.as_slice());
        let steps = self.timeline(page);
        let (g0, s0) = match seg.checked_sub(1).and_then(|k| clicks.get(k)) {
            Some(c) => (c.glyph, c.step + 1),
            None => (0, 0),
        };
        let prefix = self.chars.get(page).map_or(&[0][..], |v| v.as_slice());
        let (g1, s1) = match clicks.get(seg) {
            Some(c) => (c.glyph, c.step),
            None => (prefix.len() - 1, steps.len().saturating_sub(1)),
        };
        let count = |g: usize| prefix.get(g).copied().unwrap_or(0);
        let chars = count(g1).saturating_sub(count(g0)) as f32;
        let read = (ok(a.base) + ok(a.per_char) * chars).min(ok(a.max).max(ok(a.base)));
        let end = steps.get(s1).map_or(0.0, |s| s.at);
        let voice_end = steps
            .get(s0..s1)
            .unwrap_or(&[])
            .iter()
            .filter_map(|s| match &s.kind {
                StepKind::Emit(TextEvent::Voice(id)) => voice_len(id).map(|len| s.at + ok(len)),
                _ => None,
            })
            .reduce(f32::max);
        match voice_end {
            Some(v) => read.max((v - end).max(0.0) + ok(a.after_voice)),
            None => read,
        }
    }

    /// ページ `page` の終わりの入力待ちから、オートモードで次へ進むまでの推奨の秒数。
    ///
    /// `voice_len` はボイスの ID から長さ(秒)を返す(分からなければ `None`)。計算は [`AutoWait`](crate::AutoWait)。
    /// ページの途中に `[l]` があれば、最後の `[l]` より後ろの文字とボイスだけで数える。
    pub fn auto_wait(&self, page: usize, voice_len: impl Fn(&str) -> Option<f32>) -> f32 {
        let seg = self.clicks.get(page).map_or(0, Vec::len);
        self.auto_wait_seg(page, seg, voice_len)
    }

    /// ページ `page` の `k` 番目(0 始まり)の `[l]` から、オートモードで続きへ進むまでの推奨の
    /// 秒数。前の `[l]`(無ければページの始め)からその `[l]` までの文字とボイスで数える。
    /// その `[l]` が無ければ `None`。
    pub fn auto_wait_click(
        &self,
        page: usize,
        k: usize,
        voice_len: impl Fn(&str) -> Option<f32>,
    ) -> Option<f32> {
        let n = self.clicks.get(page).map_or(0, Vec::len);
        (k < n).then(|| self.auto_wait_seg(page, k, voice_len))
    }

    /// ページ `page` の `[l]` の、時間割の上の時刻(待たなかったときの、ページの始めからの秒)。
    pub fn click_times(&self, page: usize) -> Vec<f32> {
        self.clicks
            .get(page)
            .map_or(Vec::new(), |v| v.iter().map(|c| c.at).collect())
    }

    /// オートモードで次へ進めてよいか。入力待ち(`[l]`・ページの終わり)で、待ち始めから
    /// オートモードの待ち時間([`Reveal::auto_wait`]・[`Reveal::auto_wait_click`])がたっていれば
    /// `true`。終了(最後のページ)では `false`。
    pub fn auto_due(&self, voice_len: impl Fn(&str) -> Option<f32>) -> bool {
        if !self.waiting {
            return false;
        }
        let k = self.resumed.len();
        let (start, wait) = match self.page_clicks().get(k).filter(|_| self.waiting_click()) {
            Some(c) => (
                c.at + self.shift_after(k),
                self.auto_wait_seg(self.page, k, voice_len),
            ),
            None => (
                self.page_end(self.page),
                self.auto_wait(self.page, voice_len),
            ),
        };
        self.time - start >= wait
    }
}
