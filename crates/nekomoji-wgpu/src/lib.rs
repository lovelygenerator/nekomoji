#![doc = include_str!("../README.md")]
//!
//! ## モジュール
//!
//! - [`font`]: fontdue によるフォントの連鎖(本命 → 代わりのフォント)。`nekomoji::FontMetrics` を実装。縦書き用の字形(`vert`/`vrt2`)は ttf-parser で読む
//! - [`atlas`]: 出てきた文字だけを置く動的アトラス(GPU 非依存)
//! - [`quads`]: [`Layout`](nekomoji::Layout) + [`Reveal`](nekomoji::Reveal) → 四角形(GPU 非依存)。縦書きにも対応。猫モードでは飛ばされた文字と肉球のカーソル
//! - [`icon`]: 文中のアイコン(`[icon]`)の絵の置き場(RGBA のアトラス。GPU 非依存)
//! - [`overlay`]: 論理座標の重ね描き(行・字の枠・ルビ・上限・カーソル・猫)。確認用(GPU 非依存)
//! - [`renderer`]: wgpu のパイプラインと描画
//! - [`window`]: 窓(テキストボックス)の矩形から、幅・行数・起点・シザーを作る補助(GPU 非依存)
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod atlas;
pub mod font;
pub mod icon;
pub mod overlay;
pub mod quads;
pub mod renderer;
pub mod window;

pub use font::FontSet;
pub use icon::{IconEntry, IconError, IconSet};
pub use overlay::{build_overlay, build_rects, Overlay};
pub use quads::{
    build_neko_quads, build_neko_quads_into, build_quads, build_quads_into, Built, CursorIcons,
    Placement, Quad,
};
pub use renderer::{surface_config, surface_format, Renderer, DEFAULT_ATLAS_SIZE};
pub use window::TextWindow;
