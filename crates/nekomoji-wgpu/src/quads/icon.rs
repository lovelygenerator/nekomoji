//! 文中のアイコン(`[icon]`)の絵を四角形にする。字形ではなく RGBA の絵を、字の枠に収めて置く。

use nekomoji::GlyphInstance;

use super::{Built, Quad};
use crate::atlas::Atlas;

/// `Quad::extra[1]`(種類)の値。0 は字形のカバレッジ、1 はアイコンの絵(RGBA)。
pub(super) const KIND_ICON: f32 = 1.0;

/// アイコン `g` の絵を、画面の枠 `frame`(左, 上, 幅, 高さ。px)に縦横比を保って収めて中央に置く。
///
/// 色は白(絵のまま)、不透明度は `alpha`。回転は文字と同じに枠の中心を軸にする。
/// 登録されていない ID は何も描かず、`Built::missing_icons` に覚える(一度だけ)。
/// 見えない(不透明度 0)・枠が潰れているときも何もしない。
pub(super) fn push_icon(
    atlas: &Atlas,
    out: &mut Built,
    g: &GlyphInstance,
    frame: [f32; 4],
    alpha: f32,
) {
    let Some(id) = g.icon.as_deref() else {
        return;
    };
    push_icon_id(atlas, out, id, frame, alpha, g.rotation);
}

/// 登録した絵 `id` を、画面の枠 `frame` に収めて置く([`push_icon`] の本体。カーソルも使う)。
pub(super) fn push_icon_id(
    atlas: &Atlas,
    out: &mut Built,
    id: &str,
    frame: [f32; 4],
    alpha: f32,
    rotation: f32,
) {
    if alpha <= 0.0 || frame[2] <= 0.0 || frame[3] <= 0.0 {
        return;
    }
    let icons = atlas.icons();
    let Some(e) = icons.get(id) else {
        if !out.missing_icons.iter().any(|m| m == id) {
            out.missing_icons.push(id.to_string());
        }
        return;
    };
    let scale = (frame[2] / e.w as f32).min(frame[3] / e.h as f32);
    let (w, h) = (e.w as f32 * scale, e.h as f32 * scale);
    let tex = icons.size() as f32;
    out.quads.push(Quad {
        pos: [
            frame[0] + (frame[2] - w) / 2.0,
            frame[1] + (frame[3] - h) / 2.0,
        ],
        size: [w, h],
        uv_min: [e.x as f32 / tex, e.y as f32 / tex],
        uv_max: [(e.x + e.w) as f32 / tex, (e.y + e.h) as f32 / tex],
        color: [1.0, 1.0, 1.0, alpha],
        extra: [rotation, KIND_ICON],
    });
}
