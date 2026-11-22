//! 文字の見た目と速さ。範囲タグの入れ子で決まる。

/// 文字の見た目と速さ。範囲タグの入れ子で決まる。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Style {
    /// 色の名前か `#rrggbb`。`None` は既定色
    pub color: Option<String>,
    /// 文字の大きさの倍率(1.0 が標準)
    pub size: f32,
    /// 太字
    pub bold: bool,
    /// 文字送りの速さの倍率(1.0 が標準。大きいほど速い)
    pub speed: f32,
    /// 文字エフェクト(外側のタグから順)
    pub effects: Vec<Effect>,
    /// 上付き・下付き(`[sup]` `[sub]`)
    pub baseline: Baseline,
    /// 取り消し線(`[s]`)
    pub strike: bool,
    /// 下線(`[u]`)
    pub underline: bool,
    /// 圏点(`[em]`)の記号。`None` なら付けない
    pub emphasis: Option<char>,
    /// 名前付きの範囲(`[mark id]`)の ID。外側のタグから順
    pub marks: Vec<String>,
    /// 縁取り(`[outline]`)。`None` なら付けない(描画側の既定に任せる)
    pub outline: Option<Outline>,
    /// 影(`[shadow]`)。`None` なら付けない(描画側の既定に任せる)
    pub shadow: Option<Shadow>,
    /// 揃え(`[align]`)。`None` なら [`crate::LayoutOptions::align`] に従う
    pub align: Option<Align>,
    /// 文字送りの音(`[blip]`)。`None` はタグの外([`crate::RevealOptions::blip`] に従う)、
    /// 空の文字列は `[blip]`(設定の音の ID)、`"off"` は `[blip off]`(鳴らさない)、ほかは音の ID
    pub blip: Option<String>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            color: None,
            size: 1.0,
            bold: false,
            speed: 1.0,
            effects: Vec::new(),
            baseline: Baseline::Normal,
            strike: false,
            underline: false,
            emphasis: None,
            marks: Vec::new(),
            outline: None,
            shadow: None,
            align: None,
            blip: None,
        }
    }
}

/// 縁取り(`[outline 色? 太さ?]`)。字形の周りを太さの分だけ広げた形を、文字の後ろに塗る。
///
/// [`Outline::default`](黒・0.08)から builder のメソッドで組み立てる。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Outline {
    /// 色の名前か `#rrggbb`([`Style::color`] と同じ書き方。既定 `black`)
    pub color: String,
    /// 太さ(字の大きさに対する比。既定 0.08)
    pub width: f32,
}

/// 縁取りの太さの既定(字の大きさに対する比)
pub(crate) const OUTLINE_WIDTH: f32 = 0.08;

impl Default for Outline {
    fn default() -> Self {
        Self {
            color: "black".to_string(),
            width: OUTLINE_WIDTH,
        }
    }
}

impl Outline {
    /// 色の名前か `#rrggbb`([`Outline::color`])。
    #[must_use]
    pub fn color(mut self, value: impl Into<String>) -> Self {
        self.color = value.into();
        self
    }

    setters! {
        /// 太さ([`Outline::width`])。
        width: f32;
    }
}

/// 影(`[shadow 色? x? y?]`)。字形と同じ形を、ずらして文字の後ろに塗る。
///
/// [`Shadow::default`](黒・右下へ 0.06)から builder のメソッドで組み立てる。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Shadow {
    /// 色の名前か `#rrggbb`([`Style::color`] と同じ書き方。既定 `black`)
    pub color: String,
    /// 横のずれ(字の大きさに対する比、画面の右が正。縦書きでも画面の向き。既定 0.06)
    pub x: f32,
    /// 縦のずれ(字の大きさに対する比、画面の下が正。縦書きでも画面の向き。既定 0.06)
    pub y: f32,
}

/// 影のずれの既定(字の大きさに対する比。右へも下へもこの値)
pub(crate) const SHADOW_OFFSET: f32 = 0.06;

impl Default for Shadow {
    fn default() -> Self {
        Self {
            color: "black".to_string(),
            x: SHADOW_OFFSET,
            y: SHADOW_OFFSET,
        }
    }
}

impl Shadow {
    /// 色の名前か `#rrggbb`([`Shadow::color`])。
    #[must_use]
    pub fn color(mut self, value: impl Into<String>) -> Self {
        self.color = value.into();
        self
    }

    setters! {
        /// 横のずれ([`Shadow::x`])。
        x: f32;
        /// 縦のずれ([`Shadow::y`])。
        y: f32;
    }
}

/// 圏点の既定の記号(ゴマ点 U+FE45)
pub const EMPHASIS_MARK: char = '﹅';

/// 上付き・下付きの大きさ(周りの文字に対する倍率)
/// `[color]` などで書ける色の名前(小文字。大文字小文字は区別しない)。
pub const COLOR_NAMES: &[&str] = &[
    "white", "black", "red", "green", "blue", "yellow", "cyan", "magenta", "pink", "orange",
    "gray", "grey", "lime", "aqua", "maroon", "olive", "teal", "navy", "purple", "fuchsia",
    "silver",
];

/// 色の書き方として正しいか。色名([`COLOR_NAMES`]。大文字小文字は区別しない)か、
/// `#rgb`・`#rrggbb`(16進。大文字小文字は区別しない)。
pub fn is_color(s: &str) -> bool {
    match s.strip_prefix('#') {
        Some(hex) => matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit()),
        None => COLOR_NAMES.iter().any(|n| n.eq_ignore_ascii_case(s)),
    }
}

pub(crate) const SHIFT_SCALE: f32 = 0.6;
/// 上付きのベースラインのずらし(周りの文字の大きさに対する比、上向き)
pub(crate) const SUP_RISE: f32 = 0.4;
/// 下付きのベースラインのずらし(周りの文字の大きさに対する比、下向き)
pub(crate) const SUB_DROP: f32 = 0.15;

/// 行の揃え。段落ごとに決まる。
///
/// 縦書きでは `Start` が上、`End` が下になる(行の中の向き)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Align {
    /// 行頭に揃える(横書きでは左、縦書きでは上)。字下げ・両端揃えはこの揃えのときだけ効く
    #[default]
    Start,
    /// 中央に揃える
    Center,
    /// 行末に揃える(横書きでは右、縦書きでは下)
    End,
}

impl Align {
    /// `[align]` の引数の名前から読む(`start`・`left`・`center`・`end`・`right`)。
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "start" | "left" => Some(Self::Start),
            "center" => Some(Self::Center),
            "end" | "right" => Some(Self::End),
            _ => None,
        }
    }
}

/// 上付き・下付き。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Baseline {
    /// 通常
    #[default]
    Normal,
    /// 上付き(`[sup]`)
    Sup,
    /// 下付き(`[sub]`)
    Sub,
}

impl Style {
    /// 字の大きさ(上付き・下付きなら 0.6 を掛けた値)。
    ///
    /// [`Style::size`] は周りの文字と揃える大きさ(行の高さ・ベースラインの基準)として残る。
    pub fn glyph_size(&self) -> f32 {
        match self.baseline {
            Baseline::Normal => self.size,
            _ => self.size * SHIFT_SCALE,
        }
    }

    /// ベースラインのずらし(全角1文字を 1.0 とする単位、上向きが正)。
    pub fn rise(&self) -> f32 {
        match self.baseline {
            Baseline::Normal => 0.0,
            Baseline::Sup => SUP_RISE * self.size,
            Baseline::Sub => -SUB_DROP * self.size,
        }
    }
}

/// 文字エフェクト。計算は [`crate::Reveal::glyphs`] が時刻の関数として行う。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Effect {
    /// 揺れ。`[shake 強さ? 秒?]`
    Shake {
        /// 強さ(既定 1)
        strength: f32,
        /// 揺れる秒数。文字が出てからこの秒数かけて振れ幅が 0 へ弱まる。`None` なら揺れ続ける
        decay: Option<f32>,
    },
    /// 波。`[wave 強さ? 秒?]`
    Wave {
        /// 強さ(既定 1)
        strength: f32,
        /// 波打つ秒数。文字が出てからこの秒数かけて振れ幅が 0 へ弱まる。`None` なら続ける
        decay: Option<f32>,
    },
    /// フェードイン(秒)
    Fade(f32),
    /// 猫の出し方 `hop`: 猫が跳ねるように現れる(強さ)。`[neko hop 強さ?]`
    NekoHop(f32),
    /// 発光(ネオン)。`[glow 種類? 強さ? 秒?]`。光の色は文字の色
    Glow {
        /// 光り方
        kind: GlowKind,
        /// 強さ(既定 1)
        strength: f32,
        /// 秒数。`in` は光ってから元へ戻るまで(既定 1.5)、`breathe` は明滅の1周期(既定 2)、
        /// `flicker` は瞬きの平均の間隔(既定 3)
        secs: f32,
    },
}

/// 発光の光り方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GlowKind {
    /// 文字が出たときにふわっと光り、元の明るさへ戻る(`in`、既定)
    In,
    /// 呼吸するようにゆっくり明滅する(`breathe`)
    Breathe,
    /// ネオン管のように、ときどき不規則に瞬く(`flicker`)
    Flicker,
}

impl GlowKind {
    /// 秒数を書かなかったときの秒数。
    pub fn default_secs(self) -> f32 {
        match self {
            GlowKind::In => 1.5,
            GlowKind::Breathe => 2.0,
            GlowKind::Flicker => 3.0,
        }
    }
}
