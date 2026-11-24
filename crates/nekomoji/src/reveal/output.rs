//! 時間割の項目と、描画側に渡す描画命令の型。

use crate::layout::TcyPlace;
use crate::markup::{Outline, Shadow, TextEvent};
use crate::text::metrics::LineKind;

/// 時間割の1項目。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Step {
    /// ページの始めからの時刻(秒)
    pub at: f32,
    /// 何が起きるか
    pub kind: StepKind,
}

/// 時間割の項目の種類。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum StepKind {
    /// グリフ(ページ内の添字)を表示する
    Show(usize),
    /// 出来事を知らせる
    Emit(TextEvent),
}

/// 描画側に渡す1文字の描画命令。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GlyphInstance {
    /// 文字(書記素クラスタの先頭の文字)
    pub ch: char,
    /// 書記素クラスタの続きの文字([`crate::Glyph::tail`] と同じ。たいていは空)
    pub tail: String,
    /// 行頭からの位置(字形を描き始める位置。[`crate::Glyph::shift`] を足した値)
    pub x: f32,
    /// 送り幅([`crate::Glyph::width`] と同じ。縦書きでは行方向の送り)。
    /// アイコン(`[icon]`)の絵を置く枠の、行の中の向きの長さに使う
    pub width: f32,
    /// 行の番号
    pub line: usize,
    /// 大きさの倍率(上付き・下付きでは小さくした値)
    pub size: f32,
    /// ベースラインを揃える相手の大きさ(周りの文字の大きさ。上付き・下付きでなければ `size` と同じ)。
    /// 描画側はこの大きさの文字のベースラインを求め、そこから `rise` だけずらして置く
    pub baseline_size: f32,
    /// ベースラインのずらし(全角1文字を 1.0 とする単位、上向きが正。縦書きでは右向きが正)
    pub rise: f32,
    /// 色
    pub color: Option<String>,
    /// 太字
    pub bold: bool,
    /// 不透明度(0.0〜1.0)
    pub alpha: f32,
    /// エフェクトによるずれ(全角1文字を 1.0 とする単位。y は下向きが正)
    pub offset: (f32, f32),
    /// 回転(ラジアン、時計回りが正)。文字の中心を軸にする
    pub rotation: f32,
    /// 縦中横の置き方([`crate::Glyph::tcy`] と同じ。縦書きの `[tcy]` の文字だけ)
    pub tcy: Option<TcyPlace>,
    /// 発光の強さ(0.0 で光らない。`[glow]` の強さを掛けた値)。描画側は文字の色の光を後ろに描く
    pub glow: f32,
    /// 縁取り([`Style::outline`](crate::Style::outline) と同じ)。描画側は文字の後ろに字形を太らせた形を塗る。
    /// 不透明度は `alpha` に従い、動きの強さ([`Motion`](crate::Motion))の影響は受けない
    pub outline: Option<Outline>,
    /// 影([`Style::shadow`](crate::Style::shadow) と同じ)。描画側は文字の後ろに、ずらした字形を塗る。
    /// 不透明度は `alpha` に従い、動きの強さの影響は受けない
    pub shadow: Option<Shadow>,
    /// 文中のアイコン(`[icon id]`)なら、その ID([`crate::Glyph::icon`] と同じ)。描画側は字形の代わりに
    /// アイコンの絵を描く(位置・不透明度・ずれ・回転は文字と同じに扱う)
    pub icon: Option<String>,
}

/// 描画側に渡す線1本の描画命令。文字送り中は表示済みの文字の分だけ伸びる。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DecorationInstance {
    /// 線の種類
    pub kind: LineKind,
    /// 行の番号
    pub line: usize,
    /// 始まりの位置
    pub x0: f32,
    /// 終わりの位置(表示済みの最後の文字の端)
    pub x1: f32,
    /// 線の中心([`crate::Decoration::y`] と同じ向き)
    pub y: f32,
    /// 太さ
    pub thickness: f32,
    /// 色
    pub color: Option<String>,
    /// 不透明度(表示済みの最後の文字に合わせる)
    pub alpha: f32,
    /// 線を引く文字の大きさの倍率(上付き・下付きでも周りの文字の大きさ)。描画側が、行の中で
    /// 大きさの違う文字をそろえて置くときに、線の位置をその文字に合わせるのに使う
    pub size: f32,
}

/// 入力待ちのしるし(カーソル)を出す位置。何を描くかは描画側が決める。
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Cursor {
    /// 表示済みの最後の文字の右端(行頭からの位置。縦書きでは下端)
    pub x: f32,
    /// その行(ページ内)
    pub line: usize,
    /// 入力待ちになってからの秒数(点滅などに使う)
    pub since: f32,
    /// 何の入力待ちか(ページの途中・ページの終わり・終了)。カーソルの絵を変えるのに使う
    pub kind: WaitKind,
}

/// 入力待ちの種類([`Reveal::wait_kind`](crate::Reveal::wait_kind)・[`Cursor::kind`])。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WaitKind {
    /// ページの途中の入力待ち(`[l]`)。[`Reveal::proceed`](crate::Reveal::proceed) で同じページの続きを出す
    Click,
    /// ページの終わりの入力待ち(`[p]`・自動改ページ)。[`Reveal::proceed`](crate::Reveal::proceed) で次のページへ進む
    Page,
    /// すべて表示し終えた
    End,
}
