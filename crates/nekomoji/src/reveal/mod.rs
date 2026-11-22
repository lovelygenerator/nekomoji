//! 文字送り。時間を進めて、表示する文字数と出来事を返す。
//!
//! [`Reveal`] の処理はドメインごとに分けてある:
//! - `timeline`: [`Layout`](crate::Layout) から時間割([`Step`])を作り、ページ・文字・合図の時刻を出す
//! - `playback`: 時間を進める・skip・入力待ちから進める・今の状態を問い合わせる
//! - `auto`: オートモードの待ち時間
//! - `state`: 保存と復元([`RevealState`])
//! - `blip`: 文字送りの音を何文字ごとに鳴らすか
//! - `render`: 描画命令([`GlyphInstance`]・[`DecorationInstance`]・[`Cursor`])
//! - `effect`: 揺れ・波・発光などのエフェクトを時刻の関数で計算する
//!
//! 設定は `options`、描画側へ渡す型は `output`。

mod auto;
mod blip;
mod effect;
mod options;
mod output;
mod playback;
mod render;
mod state;
#[cfg(test)]
mod tests;
mod timeline;

pub use options::{AutoWait, Blip, Motion, RevealOptions};
pub use output::{Cursor, DecorationInstance, GlyphInstance, Step, StepKind, WaitKind};
pub use state::{ParseStateError, RevealState};

/// 文字送りの進行。[`Layout`](crate::Layout) から時間割を作り、`advance` で時間を進める。
#[derive(Debug, Clone)]
pub struct Reveal {
    timelines: Vec<Vec<Step>>,
    /// ページごと・グリフごとの表示時刻
    appear: Vec<Vec<f32>>,
    page: usize,
    time: f32,
    next: usize,
    shown: usize,
    waiting: bool,
    finished: bool,
    skipped: bool,
    motion: Motion,
    auto: AutoWait,
    /// ページごと・グリフごとの、そこまでの文字数(空白を除く。先頭に 0。オートモードの読む時間に使う)
    chars: Vec<Vec<usize>>,
    /// ページごとの、ページの途中の入力待ち(`[l]`)
    clicks: Vec<Vec<Click>>,
    /// 今のページで `[l]` から進んだ時刻(順に)
    resumed: Vec<f32>,
}

/// ページの途中の入力待ち(`[l]`)の位置。
#[derive(Debug, Clone, Copy)]
struct Click {
    /// 時間割の添字
    step: usize,
    /// 時間割の上の時刻(待たなかったときの時刻)
    at: f32,
    /// 待った後に出る最初のグリフ
    glyph: usize,
}

/// `[l]` で待っている間、続きの時刻を「今」より少しだけ後ろに置く(待っている間に続きが出たことにしない)
const CLICK_EPS: f32 = 1e-3;
