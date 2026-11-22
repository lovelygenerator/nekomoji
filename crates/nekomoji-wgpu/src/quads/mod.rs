//! [`Layout`] と [`Reveal`] の結果を、描画用の四角形(インスタンス)に変える。
//!
//! GPU に触れない純粋な処理。座標は画面のピクセル(左上が原点、下向き正)。
//!
//! 縦書き([`nekomoji::Writing::Vertical`])のレイアウトは、行を右から左へ、行の中を上から下へ置く。
//! フォントが縦書き用の字形(`vert`/`vrt2`)を持つ文字はそれを使い、持たない文字は
//! [`nekomoji::chars::vertical_form`] の分類で回転・ずらしをする。
//!
//! - `placement`: 画面への置き方([`Placement`])と色
//! - `build`: ページを四角形にする(横書き。縦書きは `vertical`)
//! - `vertical`: 縦書きの置き方(縦書き用の字形・回転・句読点のずらし・縦中横)
//! - `lines`: 取り消し線・下線
//! - `layers`: 発光・縁取り・影の形を作り、後ろから順に積む
//! - `paw`: 猫モードの肉球のカーソル
//! - `glyph`: 字形をアトラスに置いて1つの四角形にする

mod build;
mod glyph;
mod layers;
mod lines;
mod paw;
mod placement;
#[cfg(test)]
mod tests;
mod vertical;

pub use glyph::is_stretched_dash;
pub use placement::{parse_color, Placement};
pub use vertical::{VERTICAL_BASELINE, VERTICAL_PUNCT_SHIFT, VERTICAL_SMALL_KANA_SHIFT};

use bytemuck::{Pod, Zeroable};
use nekomoji::{DecorationInstance, GlyphInstance, Layout, Neko, Reveal};

use crate::atlas::Atlas;
use crate::font::FontSet;
use build::build_page;
use paw::push_paw;

/// 1文字ぶんの描画インスタンス。頂点バッファにそのまま詰める。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Quad {
    /// 左上(px)
    pub pos: [f32; 2],
    /// 大きさ(px)
    pub size: [f32; 2],
    /// アトラス内の左上 UV
    pub uv_min: [f32; 2],
    /// アトラス内の右下 UV
    pub uv_max: [f32; 2],
    /// RGBA(不透明度込み)
    pub color: [f32; 4],
    /// [回転(ラジアン、中心が軸), 予約(0)]
    pub extra: [f32; 2],
}

/// 出力。`overflow` が真のときはアトラスが満杯で、描けなかった文字がある。
///
/// 毎フレーム [`build_quads_into`]・[`build_neko_quads_into`] に同じ `Built` を渡すと、
/// 四角形の `Vec` と組み立て中に使う領域を使い回せる。
#[derive(Debug, Default)]
pub struct Built {
    /// 描く四角形(描く順)。後ろから 影・発光の光・縁取り・文字 の順に、層ごとにまとめて置く
    pub quads: Vec<Quad>,
    /// アトラスが満杯で描けなかった文字があるか
    pub overflow: bool,
    /// 組み立て中の影(最後に `quads` の先頭へ移す)
    shadows: Vec<Quad>,
    /// 組み立て中の発光の光(最後に `quads` の先頭へ移す)
    halos: Vec<Quad>,
    /// 組み立て中の縁取り(最後に `quads` の先頭へ移す)
    outlines: Vec<Quad>,
    /// 組み立て中の文字の描画命令(使い回す)
    glyphs: Vec<GlyphInstance>,
    /// 組み立て中の線の描画命令(使い回す)
    lines: Vec<DecorationInstance>,
}

impl Built {
    /// 前のフレームの中身を消す。確保済みの領域は残す。
    fn reset(&mut self) {
        self.quads.clear();
        self.overflow = false;
        self.shadows.clear();
        self.halos.clear();
        self.outlines.clear();
        self.glyphs.clear();
        self.lines.clear();
    }
}

/// 今のページの表示済みの文字(とそのルビ)を四角形にする。
///
/// 毎フレーム呼ぶなら、`Built` を使い回せる [`build_quads_into`] のほうが割り当てが少ない。
pub fn build_quads(
    layout: &Layout,
    reveal: &Reveal,
    font: &FontSet,
    atlas: &mut Atlas,
    place: &Placement,
) -> Built {
    let mut out = Built::default();
    build_quads_into(layout, reveal, font, atlas, place, &mut out);
    out
}

/// [`build_quads`] と同じ四角形を `out` に書く。`out` は最初に空にし(`overflow` も偽に戻す)、
/// 確保済みの領域は使い回す。
pub fn build_quads_into(
    layout: &Layout,
    reveal: &Reveal,
    font: &FontSet,
    atlas: &mut Atlas,
    place: &Placement,
    out: &mut Built,
) {
    build_page(layout, reveal, None, font, atlas, place, out);
}

/// [`build_quads`] に猫モードを足す。見えない猫に飛ばされた文字(とそのルビ)を動かし、
/// 入力待ちでは最後の文字の後ろに肉球のカーソルを描く(肉球の四角形は最後に積む)。
///
/// 毎フレーム呼ぶなら、`Built` を使い回せる [`build_neko_quads_into`] のほうが割り当てが少ない。
pub fn build_neko_quads(
    layout: &Layout,
    reveal: &Reveal,
    neko: &Neko,
    font: &FontSet,
    atlas: &mut Atlas,
    place: &Placement,
) -> Built {
    let mut out = Built::default();
    build_neko_quads_into(layout, reveal, neko, font, atlas, place, &mut out);
    out
}

/// [`build_neko_quads`] と同じ四角形を `out` に書く。`out` は最初に空にし(`overflow` も偽に戻す)、
/// 確保済みの領域は使い回す。
pub fn build_neko_quads_into(
    layout: &Layout,
    reveal: &Reveal,
    neko: &Neko,
    font: &FontSet,
    atlas: &mut Atlas,
    place: &Placement,
    out: &mut Built,
) {
    build_page(layout, reveal, Some(neko), font, atlas, place, out);
    if let (Some(c), Some(page)) = (reveal.cursor(layout), layout.pages.get(reveal.page())) {
        push_paw(
            layout.writing,
            page,
            &c,
            reveal.motion(),
            font,
            atlas,
            place,
            out,
        );
    }
}
