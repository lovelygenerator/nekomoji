//! 文字を出す窓(テキストボックス)の位置と大きさから、配置の設定を作る補助(GPU 非依存)。
//!
//! 画面の矩形(px)を 1 つ決めれば、コアの [`LayoutOptions`](幅・行数・書字方向)と
//! [`Placement`](本文の起点・行の間隔)、はみ出しを切るシザー矩形がそろって得られる。
//! 「何文字ぶんの幅か」と「画面の何 px か」を別々に計算しなくてよい。
//!
//! ```
//! use nekomoji::LayoutOptions;
//! use nekomoji_wgpu::{Placement, TextWindow};
//!
//! // 1280x720 の画面の下に、高さ 200 のメッセージ窓。文字は 32px
//! let win = TextWindow::bottom([1280.0, 720.0], 200.0, 24.0, 32.0).padding(16.0);
//! let opts = win.layout_options(LayoutOptions::default()); // max_width・max_lines が入る
//! let place = win.placement(Placement::default());          // 本文の左上が入る
//! assert_eq!(opts.max_lines, Some(2));
//! assert_eq!(place.origin, [40.0, 512.0]);
//! ```

use crate::quads::Placement;
use nekomoji::{LayoutOptions, Writing};

/// 文字を出す窓。位置・大きさ・余白・文字の大きさ(px)・書字方向を持つ。
///
/// `px` は [`FontSet::base_px`](crate::FontSet::base_px) と同じ値にする(字の大きさ 1 が何 px か)。
/// 画面の座標は物理ピクセルで、左上が原点。高解像度の画面では [`TextWindow::scaled`] で
/// 倍率を掛ける。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct TextWindow {
    /// 窓の矩形 `[左, 上, 幅, 高さ]`(px)
    pub rect: [f32; 4],
    /// 余白 `[上, 右, 下, 左]`(px)
    pub padding: [f32; 4],
    /// 字の大きさ(px)。`FontSet::base_px` と同じ値
    pub px: f32,
    /// 行の間隔(字の高さに対する比)。[`Placement::line_pitch`] と同じ値
    pub line_pitch: f32,
    /// 書字方向
    pub writing: Writing,
}

impl TextWindow {
    /// 矩形 `[左, 上, 幅, 高さ]`(px)と字の大きさ(px)から作る。余白なし・横書き。
    pub fn new(rect: [f32; 4], px: f32) -> Self {
        Self {
            rect,
            padding: [0.0; 4],
            px,
            line_pitch: Placement::default().line_pitch,
            writing: Writing::Horizontal,
        }
    }

    /// 画面の下に、横幅いっぱい(左右に `margin`)の窓を置く。ノベルゲームのメッセージ窓の形。
    pub fn bottom(screen: [f32; 2], height: f32, margin: f32, px: f32) -> Self {
        let rect = [
            margin,
            screen[1] - margin - height,
            screen[0] - 2.0 * margin,
            height,
        ];
        Self::new(rect, px)
    }

    /// 画面の中央に、大きさ `size` の窓を置く。
    pub fn centered(screen: [f32; 2], size: [f32; 2], px: f32) -> Self {
        let rect = [
            (screen[0] - size[0]) / 2.0,
            (screen[1] - size[1]) / 2.0,
            size[0],
            size[1],
        ];
        Self::new(rect, px)
    }

    /// 四方に同じ余白(px)を取る。
    #[must_use]
    pub fn padding(mut self, value: f32) -> Self {
        self.padding = [value; 4];
        self
    }

    /// 余白を `[上, 右, 下, 左]`(px)で別々に決める。
    #[must_use]
    pub fn padding_each(mut self, value: [f32; 4]) -> Self {
        self.padding = value;
        self
    }

    /// 字の大きさ(px)。
    #[must_use]
    pub fn px(mut self, value: f32) -> Self {
        self.px = value;
        self
    }

    /// 行の間隔(字の高さに対する比)。
    #[must_use]
    pub fn line_pitch(mut self, value: f32) -> Self {
        self.line_pitch = value;
        self
    }

    /// 書字方向。
    #[must_use]
    pub fn writing(mut self, value: Writing) -> Self {
        self.writing = value;
        self
    }

    /// 縦書きにする。
    #[must_use]
    pub fn vertical(self) -> Self {
        self.writing(Writing::Vertical)
    }

    /// 矩形・余白・字の大きさに倍率を掛ける(高解像度の画面。`scale_factor` を渡す)。
    ///
    /// フォントは `base_px * scale` で作り直す。
    #[must_use]
    pub fn scaled(mut self, scale: f32) -> Self {
        for v in self.rect.iter_mut().chain(self.padding.iter_mut()) {
            *v *= scale;
        }
        self.px *= scale;
        self
    }

    /// 余白を除いた本文の矩形 `[左, 上, 幅, 高さ]`(px)。幅・高さは 0 未満にならない。
    pub fn content_rect(&self) -> [f32; 4] {
        let [x, y, w, h] = self.rect;
        let [top, right, bottom, left] = self.padding;
        [
            x + left,
            y + top,
            (w - left - right).max(0.0),
            (h - top - bottom).max(0.0),
        ]
    }

    /// 本文の起点(px)。横書きは左上、縦書きは右上(1 行目の右端と上端)。
    pub fn origin(&self) -> [f32; 2] {
        let [x, y, w, _] = self.content_rect();
        match self.writing {
            Writing::Vertical => [x + w, y],
            _ => [x, y],
        }
    }

    /// 1 行の最大の長さ(字の大きさ 1 を単位に)。横書きは本文の幅、縦書きは高さ。
    /// [`LayoutOptions::max_width`](nekomoji::LayoutOptions::max_width) に渡す。
    pub fn max_width(&self) -> f32 {
        let [_, _, w, h] = self.content_rect();
        let along = match self.writing {
            Writing::Vertical => h,
            _ => w,
        };
        along / self.px
    }

    /// 窓に入る行数(1 以上)。横書きは本文の高さ、縦書きは幅を、行の送りで割る。
    /// [`LayoutOptions::max_lines`](nekomoji::LayoutOptions::max_lines) に渡す。
    pub fn max_lines(&self) -> usize {
        let [_, _, w, h] = self.content_rect();
        let across = match self.writing {
            Writing::Vertical => w,
            _ => h,
        };
        ((across / (self.line_pitch * self.px)).floor() as usize).max(1)
    }

    /// `base` に、書字方向・1 行の幅・行数を入れて返す(ほかの設定は `base` のまま)。
    #[must_use]
    pub fn layout_options(&self, base: LayoutOptions) -> LayoutOptions {
        base.writing(self.writing)
            .max_width(self.max_width())
            .max_lines(self.max_lines())
    }

    /// `base` に、本文の起点と行の間隔を入れて返す(ほかの設定は `base` のまま)。
    #[must_use]
    pub fn placement(&self, base: Placement) -> Placement {
        base.origin(self.origin()).line_pitch(self.line_pitch)
    }

    /// 窓の矩形をシザー矩形 `[x, y, 幅, 高さ]` にする。画面 `surface`(幅, 高さ)の中に収め、
    /// 画面の外や大きさ 0 になるときは 1px にする(wgpu は大きさ 0 のシザーを許さない)。
    /// 余白の外まで切るので、はみ出した字を窓の中だけに見せられる。
    pub fn scissor(&self, surface: [u32; 2]) -> [u32; 4] {
        let [x, y, w, h] = self.rect;
        let x0 = x.max(0.0).min(surface[0].saturating_sub(1) as f32);
        let y0 = y.max(0.0).min(surface[1].saturating_sub(1) as f32);
        let x1 = (x + w).max(x0 + 1.0).min(surface[0] as f32);
        let y1 = (y + h).max(y0 + 1.0).min(surface[1] as f32);
        let (x0, y0) = (x0.floor() as u32, y0.floor() as u32);
        [
            x0,
            y0,
            ((x1.ceil() as u32).saturating_sub(x0)).max(1),
            ((y1.ceil() as u32).saturating_sub(y0)).max(1),
        ]
    }

    /// 描く前に呼ぶと、描画を窓の中だけに切る([`TextWindow::scissor`])。
    /// 窓の外にも描きたいときは、描いた後に画面全体へ戻す(`set_scissor_rect(0, 0, w, h)`)。
    pub fn apply_scissor(&self, pass: &mut wgpu::RenderPass<'_>, surface: [u32; 2]) {
        let [x, y, w, h] = self.scissor(surface);
        pass.set_scissor_rect(x, y, w, h);
    }

    /// 画面の点 `(x, y)`(px)が窓の中か。窓をクリックしたかの判定に使う。
    pub fn contains(&self, point: [f32; 2]) -> bool {
        let [x, y, w, h] = self.rect;
        point[0] >= x && point[0] < x + w && point[1] >= y && point[1] < y + h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win() -> TextWindow {
        TextWindow::new([100.0, 50.0, 640.0, 360.0], 32.0)
    }

    #[test]
    fn 余白を除いた本文の矩形を返す() {
        let w = win().padding_each([10.0, 20.0, 30.0, 40.0]);
        assert_eq!(w.content_rect(), [140.0, 60.0, 580.0, 320.0]);
    }

    #[test]
    fn 余白が窓より大きければ大きさは0になる() {
        let w = TextWindow::new([0.0, 0.0, 10.0, 10.0], 32.0).padding(50.0);
        let r = w.content_rect();
        assert_eq!((r[2], r[3]), (0.0, 0.0));
        assert_eq!(w.max_lines(), 1);
    }

    #[test]
    fn 横書きは幅が文字数で高さが行数になる() {
        let w = win();
        assert_eq!(w.max_width(), 20.0);
        // 360 / (1.8 * 32) = 6.25
        assert_eq!(w.max_lines(), 6);
        assert_eq!(w.origin(), [100.0, 50.0]);
    }

    #[test]
    fn 縦書きは高さが文字数で幅が行数になり起点は右上() {
        let w = win().vertical();
        assert_eq!(w.max_width(), 360.0 / 32.0);
        // 640 / (1.8 * 32) = 11.1
        assert_eq!(w.max_lines(), 11);
        assert_eq!(w.origin(), [740.0, 50.0]);
    }

    #[test]
    fn 設定の組み立ては他の項目を変えない() {
        let w = win();
        let o = w.layout_options(LayoutOptions::default().max_lines(None).ruby_scale(0.4));
        assert_eq!(o.max_width, 20.0);
        assert_eq!(o.max_lines, Some(6));
        assert_eq!(o.ruby_scale, 0.4);
        let p = w.placement(Placement::default().default_color([0.0, 1.0, 0.0]));
        assert_eq!(p.origin, [100.0, 50.0]);
        assert_eq!(p.default_color, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn 下に置く窓は画面の下端から余白だけ離れる() {
        let w = TextWindow::bottom([1280.0, 720.0], 200.0, 24.0, 32.0);
        assert_eq!(w.rect, [24.0, 496.0, 1232.0, 200.0]);
    }

    #[test]
    fn 中央に置く窓は画面の真ん中に来る() {
        let w = TextWindow::centered([1000.0, 600.0], [400.0, 200.0], 24.0);
        assert_eq!(w.rect, [300.0, 200.0, 400.0, 200.0]);
    }

    #[test]
    fn 倍率は矩形と余白と字の大きさに掛かる() {
        let w = win().padding(8.0).scaled(2.0);
        assert_eq!(w.rect, [200.0, 100.0, 1280.0, 720.0]);
        assert_eq!(w.padding, [16.0; 4]);
        assert_eq!(w.px, 64.0);
        // 倍率を掛けても文字数・行数は変わらない
        assert_eq!(w.max_width(), win().padding(8.0).max_width());
    }

    #[test]
    fn シザーは画面の中に収まり大きさ0にならない() {
        let w = TextWindow::new([-10.0, 700.0, 100.0, 100.0], 32.0);
        assert_eq!(w.scissor([1280, 720]), [0, 700, 90, 20]);
        let out = TextWindow::new([2000.0, 2000.0, 10.0, 10.0], 32.0);
        let [x, y, ww, hh] = out.scissor([1280, 720]);
        assert!(ww >= 1 && hh >= 1 && x < 1280 && y < 720);
        assert!(x + ww <= 1280 && y + hh <= 720);
    }

    #[test]
    fn 点が窓の中かを調べる() {
        let w = win();
        assert!(w.contains([100.0, 50.0]));
        assert!(!w.contains([740.0, 50.0]));
        assert!(!w.contains([99.0, 100.0]));
    }
}
