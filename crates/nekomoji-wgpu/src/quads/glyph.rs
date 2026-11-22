//! 字形をラスタライズしてアトラスに置き、1つの四角形にする。

use super::Quad;
use crate::atlas::{Atlas, Shape};
use crate::font::{Ch, FontSet};

/// 字形をラスタライズする(縦書き用の字形が無ければ横書きの字形)。
pub(super) fn raster(font: &FontSet, ch: Ch, px: f32, vertical: bool) -> crate::font::Raster {
    if vertical {
        font.rasterize_vertical(ch, px)
            .unwrap_or_else(|| font.rasterize(ch, px))
    } else {
        font.rasterize(ch, px)
    }
}

/// 字形のアトラス上の位置を引き、無ければラスタライズして置く。
pub(super) fn entry(
    font: &FontSet,
    atlas: &mut Atlas,
    ch: Ch,
    px: f32,
    vertical: bool,
) -> Option<crate::atlas::Entry> {
    atlas.get_or_insert(
        (ch, px.round().max(1.0) as u32, vertical, Shape::Plain),
        || {
            let r = raster(font, ch, px, vertical);
            (r.width, r.height, r.xmin, r.ymin, r.bitmap)
        },
    )
}

pub(super) fn quad(
    e: &crate::atlas::Entry,
    pos: [f32; 2],
    tex: f32,
    color: [f32; 4],
    rotation: f32,
) -> Quad {
    Quad {
        pos,
        size: [e.w as f32, e.h as f32],
        uv_min: [e.x as f32 / tex, e.y as f32 / tex],
        uv_max: [(e.x + e.w) as f32 / tex, (e.y + e.h) as f32 / tex],
        color,
        extra: [rotation, 0.0],
    }
}

/// 送り幅いっぱいに引き伸ばして描くダッシュ類。続けて書いたとき(`――`)に途切れないように
/// (フォントの字形は左右に余白があることが多い)。
pub fn is_stretched_dash(ch: char) -> bool {
    matches!(ch, '―' | '—' | '─' | '━')
}

/// 引き伸ばすダッシュの両端の滑らかな縁(1 テクセル)を切り落とす。`axis` は 0 が横、1 が縦。
pub(super) fn trim_dash_uv(q: &mut Quad, axis: usize, tex: f32) {
    if q.uv_max[axis] - q.uv_min[axis] > 3.0 / tex {
        q.uv_min[axis] += 1.0 / tex;
        q.uv_max[axis] -= 1.0 / tex;
    }
}
