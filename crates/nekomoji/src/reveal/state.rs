//! 文字送りの保存と復元。

use super::Reveal;

/// 文字送りの保存用の状態。[`Reveal::state`] で取り出し、[`Reveal::restore`] で戻す。
///
/// 時間割は [`Layout`](crate::Layout) と [`RevealOptions`](crate::RevealOptions) から決まり、エフェクトと猫モードは時刻の関数なので、
/// 「何ページ目の何秒目か」と「`skip` したか」だけで見た目まで元に戻る。
/// 同じシナリオ・変数・設定で組んだ [`Layout`](crate::Layout) に戻すこと(シナリオを書き換えるとページがずれる)。
///
/// ページの途中の入力待ち(`[l]`)で待った長さは人によって違うので、そこから進んだ時刻も持つ(`clicks`)。
///
/// 文字列にもできる(`p<ページ> t<秒>`、`[l]` から進んでいれば ` l<秒>,<秒>…`、`skip` したときは
/// 末尾に ` skip`)。
///
/// ```
/// use nekomoji::RevealState;
/// let s = RevealState::new(2, 1.5, false);
/// assert_eq!(s.to_string(), "p2 t1.5");
/// assert_eq!("p2 t1.5".parse::<RevealState>(), Ok(s));
/// let s = RevealState::new(0, 4.0, false).clicks(vec![1.25, 3.0]);
/// assert_eq!(s.to_string(), "p0 t4 l1.25,3");
/// assert_eq!("p0 t4 l1.25,3".parse::<RevealState>(), Ok(s));
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct RevealState {
    /// ページの番号(0 始まり)
    pub page: usize,
    /// ページの始めからの経過時間(秒)
    pub time: f32,
    /// 今の区切り(ページの始めか、最後に進んだ `[l]` の後)で [`Reveal::skip`] を使ったか
    pub skipped: bool,
    /// そのページで、ページの途中の入力待ち(`[l]`)から進んだ時刻(ページの始めからの秒。順に)
    pub clicks: Vec<f32>,
}

impl RevealState {
    /// 状態を作る(保存した値から戻すとき)。
    pub fn new(page: usize, time: f32, skipped: bool) -> Self {
        Self {
            page,
            time,
            skipped,
            clicks: Vec::new(),
        }
    }

    /// `[l]` から進んだ時刻([`RevealState::clicks`])。
    #[must_use]
    pub fn clicks(mut self, value: Vec<f32>) -> Self {
        self.clicks = value;
        self
    }
}

impl std::fmt::Display for RevealState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "p{} t{}", self.page, self.time)?;
        if !self.clicks.is_empty() {
            let v: Vec<String> = self.clicks.iter().map(|c| c.to_string()).collect();
            write!(f, " l{}", v.join(","))?;
        }
        if self.skipped {
            f.write_str(" skip")?;
        }
        Ok(())
    }
}

/// [`RevealState`] の文字列が読めなかった。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParseStateError(pub String);

impl std::fmt::Display for ParseStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "文字送りの状態を読めません(`p<ページ> t<秒> [l<秒>,…] [skip]` の形): {}",
            self.0
        )
    }
}

impl std::error::Error for ParseStateError {}

impl std::str::FromStr for RevealState {
    type Err = ParseStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseStateError(s.to_string());
        let mut words = s.split_whitespace();
        let page = words
            .next()
            .and_then(|w| w.strip_prefix('p'))
            .and_then(|v| v.parse().ok())
            .ok_or_else(err)?;
        let time: f32 = words
            .next()
            .and_then(|w| w.strip_prefix('t'))
            .and_then(|v| v.parse().ok())
            .filter(|t: &f32| t.is_finite() && *t >= 0.0)
            .ok_or_else(err)?;
        let mut next = words.next();
        let mut clicks = Vec::new();
        if let Some(list) = next.and_then(|w| w.strip_prefix('l')) {
            for v in list.split(',') {
                let c: f32 = v
                    .parse()
                    .ok()
                    .filter(|c: &f32| c.is_finite() && *c >= 0.0)
                    .ok_or_else(err)?;
                clicks.push(c);
            }
            next = words.next();
        }
        let skipped = match next {
            None => false,
            Some("skip") => true,
            Some(_) => return Err(err()),
        };
        if words.next().is_some() {
            return Err(err());
        }
        Ok(RevealState {
            page,
            time,
            skipped,
            clicks,
        })
    }
}

impl Reveal {
    /// 保存用の状態を取り出す。
    pub fn state(&self) -> RevealState {
        RevealState {
            page: self.page,
            time: self.time,
            skipped: self.skipped,
            clicks: self.resumed.clone(),
        }
    }

    /// 保存した状態に戻す。表示数・入力待ち・終了は時間割から決め直す。
    ///
    /// 戻した後は `advance(0.0)` を呼んだ後と同じ状態になる(時刻 0 の文字も表示済み)。
    /// 戻すまでの間に起きるはずだった出来事(voice など)は返さない(鳴らし直すかは利用者が決める)。
    /// ページがこの文字送りに無いとき、または時刻が負・有限でないときは何もせず `false` を返す。
    pub fn restore(&mut self, state: RevealState) -> bool {
        if state.page >= self.timelines.len() || !state.time.is_finite() || state.time < 0.0 {
            return false;
        }
        if state.clicks.iter().any(|c| !c.is_finite() || *c < 0.0) {
            return false;
        }
        self.page = state.page;
        self.time = state.time;
        self.next = 0;
        self.shown = 0;
        self.waiting = false;
        self.finished = false;
        self.skipped = state.skipped;
        // 進んだ時刻は、その [l] で待ち始めた時刻より前にはできない
        let clicks = self.page_clicks().to_vec();
        self.resumed.clear();
        for (k, c) in state.clicks.iter().take(clicks.len()).enumerate() {
            let start = clicks[k].at + self.shift_after(k);
            self.resumed.push(c.max(start));
        }
        let until = if state.skipped {
            f32::INFINITY
        } else {
            state.time
        };
        self.run_until(until);
        true
    }
}
