//! 画面への置き方と、文字の色・縁取り・影の決め方。

use nekomoji::{GlyphInstance, Outline, Page, Shadow, WaitKind, Writing};

/// 入力待ちのカーソルに使うアイコンの ID(入力待ちの種類ごと)。
///
/// 絵は [`Atlas::icons_mut`](crate::atlas::Atlas::icons_mut) に登録しておく(`[icon]` と同じ)。
/// `None` の種類は、アイコンのカーソルを出さない(猫モードでは肉球になる)。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CursorIcons {
    /// ページの途中の入力待ち(`[l]`)
    pub click: Option<String>,
    /// ページの終わりの入力待ち(`[p]`・自動改ページ)
    pub page: Option<String>,
    /// すべて表示し終えたとき
    pub end: Option<String>,
}

impl CursorIcons {
    /// 入力待ちの種類 `kind` のアイコンの ID。
    pub fn get(&self, kind: WaitKind) -> Option<&str> {
        match kind {
            WaitKind::Click => self.click.as_deref(),
            WaitKind::End => self.end.as_deref(),
            _ => self.page.as_deref(),
        }
    }
}

/// 配置の設定。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Placement {
    /// 本文の左上(px)。縦書きでは本文の**右上**(1行目の右端と上端)
    pub origin: [f32; 2],
    /// 行の間隔(倍率 1.0 の文字の高さに対する比)。ルビの分を含めて 1.8 前後。
    /// 縦書きでは行(列)の間隔で、各行の右側にルビの幅を取る。
    ///
    /// 大きな字(`[size]`)のある行は、行で最も大きい文字の倍率を掛けた分だけ送る
    /// ([`Placement::line_advance`])
    pub line_pitch: f32,
    /// ルビの大きさ([`LayoutOptions::ruby_scale`](nekomoji::LayoutOptions::ruby_scale) と同じ値)
    pub ruby_scale: f32,
    /// 色を指定しないときの色(猫モードの肉球のカーソルもこの色)
    pub default_color: [f32; 3],
    /// 縁取りの既定。`[outline]` で囲んでいない文字(とそのルビ・圏点)に付ける。`None` なら付けない
    pub outline: Option<Outline>,
    /// 影の既定。`[shadow]` で囲んでいない文字(とそのルビ・圏点)に付ける。`None` なら付けない
    pub shadow: Option<Shadow>,
    /// 入力待ちのカーソルに使うアイコン(種類ごと)。既定はどれも `None`
    /// (通常は何も描かず、猫モードでは肉球)。絵が登録されていなければ何も描かず、
    /// [`Built::missing_icons`](super::Built::missing_icons) に知らせる
    pub cursor_icons: CursorIcons,
    /// アイコンのカーソルの大きさ(倍率 1.0 の文字の高さに対する比。正方形の枠に絵を収める)
    pub cursor_size: f32,
}

impl Placement {
    /// 入力待ちのすべての種類のカーソルを、アイコン `id` にする([`Placement::cursor_icons`])。
    #[must_use]
    pub fn cursor_icon(mut self, id: impl Into<String>) -> Self {
        let id = id.into();
        self.cursor_icons = CursorIcons {
            click: Some(id.clone()),
            page: Some(id.clone()),
            end: Some(id),
        };
        self
    }

    /// 入力待ちの種類 `kind` のカーソルだけをアイコンにする(`None` で戻す)。
    /// 「ページの途中は ▼、最後は ■」のように種類ごとに変えられる。
    #[must_use]
    pub fn cursor_icon_for(mut self, kind: WaitKind, id: Option<String>) -> Self {
        match kind {
            WaitKind::Click => self.cursor_icons.click = id,
            WaitKind::End => self.cursor_icons.end = id,
            _ => self.cursor_icons.page = id,
        }
        self
    }

    /// アイコンのカーソルの大きさ([`Placement::cursor_size`])。
    #[must_use]
    pub fn cursor_size(mut self, value: f32) -> Self {
        self.cursor_size = value;
        self
    }

    /// 本文の左上(縦書きでは右上)の位置([`Placement::origin`])。
    #[must_use]
    pub fn origin(mut self, value: [f32; 2]) -> Self {
        self.origin = value;
        self
    }

    /// 行の間隔([`Placement::line_pitch`])。
    #[must_use]
    pub fn line_pitch(mut self, value: f32) -> Self {
        self.line_pitch = value;
        self
    }

    /// ルビの大きさ([`Placement::ruby_scale`])。
    #[must_use]
    pub fn ruby_scale(mut self, value: f32) -> Self {
        self.ruby_scale = value;
        self
    }

    /// 色を指定しないときの色([`Placement::default_color`])。
    #[must_use]
    pub fn default_color(mut self, value: [f32; 3]) -> Self {
        self.default_color = value;
        self
    }

    /// 縁取りの既定([`Placement::outline`])。
    #[must_use]
    pub fn outline(mut self, value: Option<Outline>) -> Self {
        self.outline = value;
        self
    }

    /// 影の既定([`Placement::shadow`])。
    #[must_use]
    pub fn shadow(mut self, value: Option<Shadow>) -> Self {
        self.shadow = value;
        self
    }
}

/// 行の並び方。大きな字のある行は、行で最も大きい文字の倍率([`nekomoji::Line::size`]。
/// 1 未満は 1)を行送りとルビの場所に掛ける。行の中では、大きさの違う文字を横書きはベースライン、
/// 縦書きは中心でそろえる。ページの行の外(行数の上限の目盛りなど)は倍率 1 で続ける。
impl Placement {
    /// 行 `line` の倍率(行送りとルビの場所に掛ける)。行で最も大きい文字の倍率で、1 未満なら 1。
    pub fn line_scale(page: &Page, line: usize) -> f32 {
        page.lines.get(line).map_or(1.0, |l| l.size.max(1.0))
    }

    /// 行 `line` の送り(px)。行の始まりの辺から次の行の始まりの辺まで。
    pub fn line_advance(&self, page: &Page, line: usize, base_px: f32) -> f32 {
        self.line_pitch * base_px * Self::line_scale(page, line)
    }

    /// 行 `line` の始まりの辺の、[`Placement::origin`] からの距離(px)。横書きは下へ、縦書きは
    /// 左へ測る。
    pub fn line_offset(&self, page: &Page, line: usize, base_px: f32) -> f32 {
        let n = line.min(page.lines.len());
        let known: f32 = (0..n).map(|j| self.line_advance(page, j, base_px)).sum();
        known + (line - n) as f32 * self.line_pitch * base_px
    }

    /// 行 `line` のルビの場所(行の始まりの辺から本文までの幅、px)。
    pub fn ruby_room(&self, page: &Page, line: usize, base_px: f32) -> f32 {
        self.ruby_scale * base_px * Self::line_scale(page, line)
    }

    /// 行 `line` に置く大きさ `size` の字の枠の始まり(横書きは上端、縦書きは右端)の、行の始まりの
    /// 辺からの距離(px)。行で最も大きい文字と、横書きはベースライン、縦書きは中心をそろえる。
    ///
    /// `ascent` は字の枠の上端からベースラインまでの、字の大きさに対する比
    /// (`FontSet::ascent(1.0)`)。縦書きでは使わない。上付き・下付きは周りの文字の大きさ
    /// ([`GlyphInstance::baseline_size`])を渡す。
    pub fn glyph_across(
        &self,
        writing: Writing,
        page: &Page,
        line: usize,
        size: f32,
        ascent: f32,
        base_px: f32,
    ) -> f32 {
        let top = page.lines.get(line).map_or(size, |l| l.size.max(size));
        let align = match writing {
            Writing::Vertical => (top - size) / 2.0,
            _ => ascent * (top - size),
        };
        self.ruby_room(page, line, base_px) + align * base_px
    }

    /// 行の始まりの辺からの距離 `d`(px)を、小数の行の番号にする(0.0〜1.0 が 0 行目の行送り)。
    pub(crate) fn line_at(&self, page: &Page, d: f32, base_px: f32) -> f32 {
        let pitch = self.line_pitch * base_px;
        if d < 0.0 {
            return d / pitch;
        }
        let mut start = 0.0;
        for j in 0..page.lines.len() {
            let a = self.line_advance(page, j, base_px);
            if d < start + a {
                return j as f32 + (d - start) / a;
            }
            start += a;
        }
        page.lines.len() as f32 + (d - start) / pitch
    }
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            origin: [32.0, 32.0],
            line_pitch: 1.8,
            ruby_scale: 0.5,
            default_color: [1.0, 1.0, 1.0],
            outline: None,
            shadow: None,
            cursor_icons: CursorIcons::default(),
            cursor_size: 0.8,
        }
    }
}

/// 色名(大文字小文字は区別しない)か `#rgb`・`#rrggbb` を RGB にする。知らない名前は `None`。
pub fn parse_color(s: &str) -> Option<[f32; 3]> {
    if let Some(hex) = s.strip_prefix('#') {
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        // `#rgb` は各桁を2回書いた `#rrggbb` と同じ
        let v = match hex.len() {
            6 => u32::from_str_radix(hex, 16).ok()?,
            3 => {
                let d = u32::from_str_radix(hex, 16).ok()?;
                let (r, g, b) = ((d >> 8) & 0xf, (d >> 4) & 0xf, d & 0xf);
                (r * 0x11) << 16 | (g * 0x11) << 8 | (b * 0x11)
            }
            _ => return None,
        };
        return Some([
            ((v >> 16) & 0xff) as f32 / 255.0,
            ((v >> 8) & 0xff) as f32 / 255.0,
            (v & 0xff) as f32 / 255.0,
        ]);
    }
    Some(match s.to_ascii_lowercase().as_str() {
        "white" => [1.0, 1.0, 1.0],
        "black" => [0.0, 0.0, 0.0],
        "red" => [1.0, 0.25, 0.25],
        "green" => [0.3, 0.9, 0.4],
        "blue" => [0.35, 0.55, 1.0],
        "yellow" => [1.0, 0.9, 0.3],
        "cyan" => [0.3, 0.9, 0.95],
        "magenta" | "pink" => [1.0, 0.5, 0.8],
        "orange" => [1.0, 0.65, 0.2],
        "gray" | "grey" => [0.6, 0.6, 0.6],
        "lime" => [0.0, 1.0, 0.0],
        "aqua" => [0.0, 1.0, 1.0],
        "maroon" => [0.5, 0.0, 0.0],
        "olive" => [0.5, 0.5, 0.0],
        "teal" => [0.0, 0.5, 0.5],
        "navy" => [0.0, 0.0, 0.5],
        "purple" => [0.5, 0.0, 0.5],
        "fuchsia" => [1.0, 0.0, 1.0],
        "silver" => [0.75, 0.75, 0.75],
        _ => return None,
    })
}

/// 文字の縁取りと影。タグで付けていなければ [`Placement`] の既定。
pub(super) fn deco_of<'a>(
    g: &'a GlyphInstance,
    place: &'a Placement,
) -> (Option<&'a Outline>, Option<&'a Shadow>) {
    (
        g.outline.as_ref().or(place.outline.as_ref()),
        g.shadow.as_ref().or(place.shadow.as_ref()),
    )
}

pub(super) fn color_of(g: &GlyphInstance, place: &Placement) -> [f32; 4] {
    let rgb = g
        .color
        .as_deref()
        .and_then(parse_color)
        .unwrap_or(place.default_color);
    [rgb[0], rgb[1], rgb[2], g.alpha]
}
