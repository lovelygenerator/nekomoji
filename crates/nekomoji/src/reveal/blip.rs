//! 文字送りの音を何文字ごとに鳴らすか数える。

use super::options::RevealOptions;
use crate::markup::Style;

/// 文字送りの音を何文字ごとに鳴らすか数える。ページごとに作り直す。
#[derive(Default)]
pub(super) struct BlipCounter {
    /// 今の音の ID と、その音で数えた字の数
    current: Option<(String, usize)>,
    /// 前に鳴らした時刻
    pub(super) last: Option<f32>,
}

impl BlipCounter {
    /// 時刻 `t` に出る文字 `ch` で鳴らす音の ID。
    pub(super) fn next(
        &mut self,
        ch: char,
        t: f32,
        style: Option<&Style>,
        opts: &RevealOptions,
    ) -> Option<String> {
        let base = opts.blip.clone().unwrap_or_default();
        let id = match style.and_then(|s| s.blip.as_deref()) {
            Some("off") => None,
            Some("") => Some(base.id.clone()),
            Some(id) => Some(id.to_string()),
            None => opts.blip.as_ref().map(|b| b.id.clone()),
        };
        let Some(id) = id else {
            // 鳴らさない範囲では数え直す
            self.current = None;
            return None;
        };
        if !ch.is_alphanumeric() {
            return None;
        }
        let count = match &mut self.current {
            Some((cur, n)) if *cur == id => {
                *n += 1;
                *n
            }
            _ => {
                self.current = Some((id.clone(), 1));
                1
            }
        };
        let every = base.every.max(1);
        if (count - 1) % every != 0 {
            return None;
        }
        if self.last.is_some_and(|l| t - l < base.min_interval) {
            return None;
        }
        self.last = Some(t);
        Some(id)
    }
}
