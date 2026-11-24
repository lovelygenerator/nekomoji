//! 描画命令を作る: 文字・線・入力待ちのカーソル。

use super::effect::{fading, glow_at, motion_decay, noise, HOP_SECS};
use super::options::Motion;
use super::output::{Cursor, DecorationInstance, GlyphInstance, WaitKind};
use super::Reveal;
use crate::layout::Layout;
use crate::markup::{Effect, Style};

impl Reveal {
    /// 入力待ち(`[l]`・`[p]`・終了)のときの、カーソルを出す位置。文字送りの途中なら `None`。
    pub fn cursor(&self, layout: &Layout) -> Option<Cursor> {
        let kind = self.wait_kind()?;
        let page = layout.pages.get(self.page)?;
        let (x, line) = match page.glyphs[..self.shown.min(page.glyphs.len())].last() {
            Some(g) => (g.x + g.width, g.line),
            None => (0.0, 0),
        };
        let start = match kind {
            WaitKind::Click => {
                let k = self.resumed.len();
                self.page_clicks()
                    .get(k)
                    .map_or(0.0, |c| c.at + self.shift_after(k))
            }
            _ => self.page_end(self.page),
        };
        Some(Cursor {
            x,
            line,
            since: (self.time - start).max(0.0),
            kind,
        })
    }

    /// 今のページの表示済みの文字を、描画命令として返す。
    ///
    /// エフェクトは時刻の関数で、同じ状態なら同じ結果になる(乱数を使わない)。
    /// 毎フレーム呼ぶなら、バッファを使い回せる [`Reveal::glyphs_into`] のほうが割り当てが少ない。
    pub fn glyphs(&self, layout: &Layout) -> Vec<GlyphInstance> {
        let mut out = Vec::new();
        self.glyphs_into(layout, &mut out);
        out
    }

    /// [`Reveal::glyphs`] と同じ描画命令を `out` に書く。`out` は最初に空にし、確保済みの領域は使い回す。
    ///
    /// ```
    /// # use nekomoji::{parse, CellMetrics, Layout, LayoutOptions, Reveal, RevealOptions, Vars};
    /// let script = parse("こんにちは").unwrap();
    /// let layout = Layout::build(&script, &CellMetrics, &LayoutOptions::default(), &Vars::new());
    /// let mut reveal = Reveal::new(&layout, RevealOptions::default());
    /// let mut buf = Vec::new();
    /// for _ in 0..3 {
    ///     reveal.advance(0.1);
    ///     reveal.glyphs_into(&layout, &mut buf); // 毎フレーム同じ Vec を使う
    ///     assert_eq!(buf, reveal.glyphs(&layout));
    /// }
    /// ```
    pub fn glyphs_into(&self, layout: &Layout, out: &mut Vec<GlyphInstance>) {
        out.clear();
        let Some(page) = layout.pages.get(self.page) else {
            return;
        };
        let shown = &page.glyphs[..self.shown.min(page.glyphs.len())];
        if shown.is_empty() {
            return;
        }
        let flicker_keys = flicker_keys(&layout.styles);
        let fallback = Style::default();
        out.extend(shown.iter().enumerate().map(|(i, g)| {
            let style = layout.styles.get(g.style).unwrap_or(&fallback);
            let key = flicker_keys.get(g.style).copied().unwrap_or(0);
            let m = self.motion_at(style, i, key);
            GlyphInstance {
                ch: g.ch,
                tail: g.tail.clone(),
                x: g.x + g.shift,
                width: g.width,
                line: g.line,
                size: style.glyph_size(),
                baseline_size: style.size,
                rise: style.rise(),
                color: style.color.clone(),
                bold: style.bold,
                alpha: m.alpha,
                offset: m.offset,
                rotation: m.rotation,
                tcy: g.tcy,
                glow: m.glow,
                outline: style.outline.clone(),
                shadow: style.shadow.clone(),
                icon: g.icon.clone(),
            }
        }));
    }

    /// 今のページの `i` 番目の文字の、エフェクトによる不透明度・ずれ・回転・発光。割り当てをしない。
    fn motion_at(&self, style: &Style, i: usize, flicker_key: u32) -> GlyphMotion {
        let age = self.time - self.glyph_time(i);
        let mut m = GlyphMotion {
            alpha: 1.0,
            offset: (0.0, 0.0),
            rotation: 0.0,
            glow: 0.0,
        };
        for e in &style.effects {
            match *e {
                Effect::Fade(secs) if secs > 0.0 => m.alpha *= (age / secs).clamp(0.0, 1.0),
                Effect::Fade(_) => {}
                Effect::Shake { strength, decay } => {
                    let amp = strength
                        * self.motion.amplitude()
                        * fading(age, motion_decay(decay, self.motion));
                    let frame = (self.time * 30.0) as u32;
                    m.offset.0 += amp * 0.05 * noise(i as u32, frame);
                    m.offset.1 += amp * 0.05 * noise(i as u32 ^ 0x9e37, frame);
                }
                Effect::Wave { strength, decay } => {
                    let amp = strength
                        * self.motion.amplitude()
                        * fading(age, motion_decay(decay, self.motion));
                    m.offset.1 += amp * 0.1 * (self.time * 6.0 + i as f32 * 0.6).sin();
                }
                Effect::NekoHop(amp) => {
                    // 左下から放物線で跳び上がり、少し傾いた姿勢から着地する
                    let u = (age / HOP_SECS).clamp(0.0, 1.0);
                    let amp = amp * self.motion.amplitude();
                    m.offset.0 -= amp * 0.35 * (1.0 - u);
                    m.offset.1 -= amp * 0.6 * 4.0 * u * (1.0 - u);
                    if self.motion == Motion::Full {
                        m.rotation -= amp * 0.35 * (1.0 - u);
                    }
                    m.alpha *= (u * 4.0).min(1.0);
                }
                Effect::Glow {
                    kind,
                    strength,
                    secs,
                } => {
                    let (g, dim) = glow_at(kind, age, secs, self.time, flicker_key, self.motion);
                    m.glow = m.glow.max(strength * g);
                    m.alpha *= dim;
                }
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }
        m
    }

    /// 今のページの、表示済みの文字に掛かる取り消し線・下線。
    ///
    /// 毎フレーム呼ぶなら、バッファを使い回せる [`Reveal::decorations_into`] のほうが割り当てが少ない。
    pub fn decorations(&self, layout: &Layout) -> Vec<DecorationInstance> {
        let mut out = Vec::new();
        self.decorations_into(layout, &mut out);
        out
    }

    /// [`Reveal::decorations`] と同じ描画命令を `out` に書く。`out` は最初に空にし、確保済みの領域は使い回す。
    ///
    /// ```
    /// # use nekomoji::{parse, CellMetrics, Layout, LayoutOptions, Reveal, RevealOptions, Vars};
    /// let script = parse("[s]取り消し[/s]").unwrap();
    /// let layout = Layout::build(&script, &CellMetrics, &LayoutOptions::default(), &Vars::new());
    /// let mut reveal = Reveal::new(&layout, RevealOptions::default());
    /// let mut buf = Vec::new();
    /// for _ in 0..3 {
    ///     reveal.advance(0.1);
    ///     reveal.decorations_into(&layout, &mut buf); // 毎フレーム同じ Vec を使う
    ///     assert_eq!(buf, reveal.decorations(&layout));
    /// }
    /// ```
    pub fn decorations_into(&self, layout: &Layout, out: &mut Vec<DecorationInstance>) {
        out.clear();
        let Some(page) = layout.pages.get(self.page) else {
            return;
        };
        if page.decorations.is_empty() {
            return;
        }
        let shown = self.shown.min(page.glyphs.len());
        let fallback = Style::default();
        out.extend(
            page.decorations
                .iter()
                .filter(|d| d.first_glyph < shown)
                .map(|d| {
                    let last = (d.first_glyph + d.glyph_count).min(shown) - 1;
                    let g = &page.glyphs[last];
                    // 不透明度は表示済みの最後の文字に合わせる(その文字だけを計算する)
                    let gs = layout.styles.get(g.style).unwrap_or(&fallback);
                    let key = flicker_key(&layout.styles, g.style);
                    let alpha = self.motion_at(gs, last, key).alpha;
                    let st = layout.styles.get(d.style);
                    let color = st.and_then(|st| st.color.clone());
                    let size = st.map_or(1.0, |st| st.size);
                    DecorationInstance {
                        kind: d.kind,
                        line: d.line,
                        x0: d.x0,
                        x1: g.x + g.width,
                        y: d.y,
                        thickness: d.thickness,
                        color,
                        alpha,
                        size,
                    }
                }),
        );
    }
}

/// 1文字のエフェクトの結果。
struct GlyphMotion {
    alpha: f32,
    offset: (f32, f32),
    rotation: f32,
    glow: f32,
}

/// 瞬き(flicker)の乱数の鍵。名前付きの範囲(`[mark]`)・揃え(`[align]`)・文字送りの音
/// (`[blip]`)は字の見た目を変えないので、それだけが違うスタイルは同じ鍵にして揃って瞬かせる。
fn same_look(a: &Style, b: &Style) -> bool {
    let plain = |st: &Style| Style {
        marks: Vec::new(),
        align: None,
        blip: None,
        ..st.clone()
    };
    plain(a) == plain(b)
}

fn has_glow(st: &Style) -> bool {
    st.effects.iter().any(|e| matches!(e, Effect::Glow { .. }))
}

/// スタイル `k` の瞬きの鍵(見た目が同じ最初のスタイルの添字)。発光を使わないスタイルは 0。
fn flicker_key(styles: &[Style], k: usize) -> u32 {
    match styles.get(k) {
        Some(st) if has_glow(st) => {
            styles.iter().position(|o| same_look(o, st)).unwrap_or(k) as u32
        }
        _ => 0,
    }
}

/// すべてのスタイルの瞬きの鍵。発光を使うスタイルが無ければ空(スタイルの数の2乗の比較を毎フレームしない)。
fn flicker_keys(styles: &[Style]) -> Vec<u32> {
    if !styles.iter().any(has_glow) {
        return Vec::new();
    }
    (0..styles.len()).map(|k| flicker_key(styles, k)).collect()
}
