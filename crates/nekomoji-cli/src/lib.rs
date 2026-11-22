//! nekomoji の CLI・REPL の本体(端末への表示と、REPL のコマンド処理)。
//!
//! 端末への出力はすべて文字列を返す関数にしてあり、テストで比較できる。
#![forbid(unsafe_code)]

pub mod render;
pub mod repl;
#[cfg(feature = "subset")]
pub mod subset;

pub use render::{
    align_name, apply_bracket_indent, apply_indent, check_json, check_text, format_error,
    format_errors, log, motion_name, page_rows, page_rows_with, parse_align, parse_blip,
    parse_indent_pair, parse_motion, CatView, Checked, Settings,
};
pub use repl::{Action, Repl};

/// 1行の幅(全角の文字数)の上限。端末の罫線などを幅の分だけ作るので、大きすぎる値は受け付けない。
pub const MAX_WIDTH: f32 = 1000.0;
