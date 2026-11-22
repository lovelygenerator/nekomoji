//! fontdue によるフォント。[`FontMetrics`] の実装とラスタライズを受け持つ。
//!
//! 縦書き用の字形の置き換え(GSUB の `vert`/`vrt2`)と、仮名のプロポーショナル詰め
//! (GPOS の `palt`/`vpal`)は `ttf-parser` で読む。

use std::collections::HashMap;

use fontdue::{Font, FontSettings};
use nekomoji::{FontMetrics, LineKind};

/// 描く1文字: 文字と、付いている異体字セレクタ。
///
/// 書記素クラスタ([`nekomoji::Glyph::tail`])の最初が異体字セレクタ(`FE00`〜`FE0F`・IVS の
/// `E0100`〜`E01EF`)なら `vs` に入れる。フォントの cmap の異体字の表(format 14)に字形があれば
/// それを描き、無ければ `ch` の字形を描く。`char` から作れる(`'辻'.into()`)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ch {
    /// 文字
    pub ch: char,
    /// 異体字セレクタ
    pub vs: Option<char>,
}

impl Ch {
    /// 文字と、書記素クラスタの続き(`tail`)から作る。続きの最初が異体字セレクタなら使う。
    pub fn new(ch: char, tail: &str) -> Self {
        Self {
            ch,
            vs: tail.chars().next().filter(|&c| is_variation_selector(c)),
        }
    }
}

impl From<char> for Ch {
    fn from(ch: char) -> Self {
        Self { ch, vs: None }
    }
}

/// 異体字セレクタか(`FE00`〜`FE0F`・`E0100`〜`E01EF`)。
pub fn is_variation_selector(c: char) -> bool {
    matches!(c as u32, 0xFE00..=0xFE0F | 0xE0100..=0xE01EF)
}

/// フォントの連鎖と基準のピクセル数。
///
/// 最初のフォント(本命)に字形が無い文字は、[`FontSet::with_fallback`] で足した代わりのフォントを
/// 足した順に探す(本命 → 大きいフォント)。名前入力など、シナリオから決まらない文字
/// (`[var 変数]` の値)をサブセットのフォントで描くときに、豆腐にならないようにする。
///
/// 探す順([`FontSet::glyph`]):
/// 1. 文字そのものを、連鎖の順に探す。最初に字形を持つフォントで、異体字セレクタ付き([`Ch::vs`])なら
///    そのフォントの異体字の字形、無ければ元の字形
/// 2. どのフォントにも無ければ、[`nekomoji::chars::fallback_chars`] の代わりの文字を、代わりの文字の順・
///    連鎖の順に探す
/// 3. それでも無ければ本命の .notdef
///
/// ベースライン(`ascent`)と線の位置は本命のフォントで決め、代わりのフォントの字形も同じ
/// ベースラインに置く。`ascent` は OS/2 の typo ascender(和文フォントでは仮想ボディの上端。
/// hhea の ascender は行の上下の余白を含み、字が枠より下へずれる)。幅の単位は「全角1文字 = 1.0」(`size` 倍率 1.0 のとき)。
///
/// [`FontSet::with_proportional`] で仮名をプロポーショナルに詰める(既定は詰めない)。
pub struct FontSet {
    /// 連鎖。0 番が本命
    faces: Vec<Face>,
    /// 倍率 1.0 のときの文字の大きさ(px)
    pub base_px: f32,
    /// 仮名をプロポーショナルに詰めるか(`palt`・`vpal`)
    proportional: bool,
}

/// プロポーショナル詰めの調整(全角比)。横書きは右向き、縦書きは下向きを正とする。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Adjust {
    /// 字形を置く位置のずれ
    place: f32,
    /// 送りの増減
    advance: f32,
}

/// 連鎖の中の1つのフォント。
struct Face {
    font: Font,
    /// 縦書き用の字形の置き換え(横書きの字形番号 → 縦書きの字形番号)
    vert: HashMap<u16, u16>,
    /// 異体字の字形(文字, 異体字セレクタ)→ 字形番号(cmap format 14 の non-default UVS)
    variants: HashMap<(char, char), u16>,
    /// 取り消し線・下線の (上端のベースラインからの高さ, 太さ)(全角比、上向き正)
    strike: Option<(f32, f32)>,
    underline: Option<(f32, f32)>,
    /// OS/2 の typo ascender(全角比)。無いか 0 以下なら `None`
    ascent: Option<f32>,
    /// 仮名の横書きの詰め(GPOS の `palt`)。字形番号 → 調整
    palt: HashMap<u16, Adjust>,
    /// 仮名の縦書きの詰め(GPOS の `vpal`)。字形番号(縦書き用の字形があればそれ)→ 調整
    vpal: HashMap<u16, Adjust>,
}

impl Face {
    fn from_bytes(data: &[u8], collection_index: u32) -> Result<Self, String> {
        let settings = FontSettings {
            collection_index,
            ..FontSettings::default()
        };
        let font = Font::from_bytes(data, settings).map_err(|e| e.to_string())?;
        let (strike, underline) = read_line_metrics(data, collection_index);
        let ascent = read_typo_ascender(data, collection_index);
        let vert = read_vert(data, collection_index);
        let (palt, vpal) = read_proportional(data, collection_index, &vert);
        Ok(Self {
            font,
            vert,
            variants: read_variants(data, collection_index),
            strike,
            underline,
            ascent,
            palt,
            vpal,
        })
    }

    /// 文字の字形番号(無ければ 0)。異体字の字形があればそれ。
    fn lookup(&self, ch: char, vs: Option<char>) -> u16 {
        let id = self.font.lookup_glyph_index(ch);
        if id == 0 {
            return 0;
        }
        vs.and_then(|v| self.variants.get(&(ch, v)).copied())
            .unwrap_or(id)
    }
}

/// 連鎖の中の字形: 何番目のフォントの、どの字形番号か。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct FontGlyph {
    /// 連鎖の中のフォントの番号(0 が本命)
    pub font: usize,
    /// そのフォントの字形番号
    pub id: u16,
}

/// ラスタライズ結果(カバレッジ 8bit)。
#[derive(Debug, Clone)]
pub struct Raster {
    /// 幅(px)
    pub width: u32,
    /// 高さ(px)
    pub height: u32,
    /// 原点(ベースライン左)からビットマップ左端までの距離(px)
    pub xmin: i32,
    /// ベースラインからビットマップ下端までの距離(px、上向き正)
    pub ymin: i32,
    /// 横書きの送り幅(px)
    pub advance: f32,
    /// カバレッジ(1 画素 1 バイト、上の行から)
    pub bitmap: Vec<u8>,
}

/// 字形が無い文字に使う、本命の .notdef。
const NOTDEF: FontGlyph = FontGlyph { font: 0, id: 0 };

impl FontSet {
    /// ttf / otf / ttc のバイト列から作る(本命のフォント)。`collection_index` は ttc のときだけ効く。
    pub fn from_bytes(data: &[u8], collection_index: u32, base_px: f32) -> Result<Self, String> {
        Ok(Self {
            faces: vec![Face::from_bytes(data, collection_index)?],
            base_px,
            proportional: false,
        })
    }

    /// 代わりのフォントを連鎖の最後に足す。本命と、それまでに足したフォントに字形が無い文字を
    /// このフォントで描く。
    ///
    /// ```no_run
    /// # use nekomoji_wgpu::FontSet;
    /// # let (subset, large) = (vec![], vec![]);
    /// let font = FontSet::from_bytes(&subset, 0, 32.0)?.with_fallback(&large, 0)?;
    /// # Ok::<(), String>(())
    /// ```
    pub fn with_fallback(mut self, data: &[u8], collection_index: u32) -> Result<Self, String> {
        self.faces.push(Face::from_bytes(data, collection_index)?);
        Ok(self)
    }

    /// 連鎖のフォントの数(本命を含む)。
    pub fn font_count(&self) -> usize {
        self.faces.len()
    }

    /// 文字 `ch` を描く字形(探す順は [`FontSet`] の説明)。どのフォントにも、代わりの文字も
    /// 無ければ `None`(描くときは本命の .notdef になる)。
    pub fn glyph(&self, ch: impl Into<Ch>) -> Option<FontGlyph> {
        let Ch { ch, vs } = ch.into();
        self.find(ch, vs).or_else(|| {
            nekomoji::chars::fallback_chars(ch)
                .iter()
                .find_map(|&c| self.find(c, None))
        })
    }

    /// 文字そのものを連鎖の順に探す。
    fn find(&self, ch: char, vs: Option<char>) -> Option<FontGlyph> {
        self.faces.iter().enumerate().find_map(|(font, face)| {
            let id = face.lookup(ch, vs);
            (id != 0).then_some(FontGlyph { font, id })
        })
    }

    /// 連鎖のどれかのフォントに `ch` の字形があるか(代わりの文字を含めない)。
    pub fn has_glyph(&self, ch: char) -> bool {
        self.find(ch, None).is_some()
    }

    /// 文字 `ch` を `px` ピクセルで描いたビットマップ。
    pub fn rasterize(&self, ch: impl Into<Ch>, px: f32) -> Raster {
        self.rasterize_glyph(self.glyph(ch).unwrap_or(NOTDEF), px)
    }

    /// 縦書き用の字形があれば、それを `px` ピクセルで描いたビットマップ。
    pub fn rasterize_vertical(&self, ch: impl Into<Ch>, px: f32) -> Option<Raster> {
        let g = self.vertical_glyph(ch)?;
        Some(self.rasterize_glyph(g, px))
    }

    /// 文字を描くフォントが縦書き用の字形(`vert`/`vrt2`)を持っていれば、その字形。
    pub fn vertical_glyph(&self, ch: impl Into<Ch>) -> Option<FontGlyph> {
        let g = self.glyph(ch)?;
        let id = *self.faces[g.font].vert.get(&g.id)?;
        Some(FontGlyph { font: g.font, id })
    }

    /// 連鎖のどれかのフォントが異体字の字形(cmap format 14)を持っているか。
    pub fn has_variants(&self) -> bool {
        self.faces.iter().any(|f| !f.variants.is_empty())
    }

    /// 縦書き用の字形の置き換えを使わないようにする(対応表での補正だけになる)。
    ///
    /// `vert` を持たないフォントの見た目を確かめるときに使う。連鎖のすべてのフォントに効く。
    pub fn without_vertical_forms(mut self) -> Self {
        for f in &mut self.faces {
            f.vert.clear();
        }
        self
    }

    /// 連鎖のどれかのフォントが縦書き用の字形の置き換え(`vert`/`vrt2` の単純置換)を持っているか。
    pub fn has_vertical_forms(&self) -> bool {
        self.faces.iter().any(|f| !f.vert.is_empty())
    }

    /// 仮名をプロポーショナルに詰める(GPOS の `palt`、縦書きでは `vpal`)。
    ///
    /// 詰めるのは仮名(ひらがな・カタカナ・長音・`々` など)だけで、句読点・括弧などの約物は
    /// 詰めない(約物は [`nekomoji::LayoutOptions::punct_squeeze`] に任せる)。フォントが
    /// `palt`・`vpal` を持たない文字は全角のまま。送り幅([`FontMetrics::advance`]・
    /// [`FontMetrics::vertical_advance`])と、描くときの字形の位置の両方が変わるので、
    /// このフォントで組んだ [`nekomoji::Layout`] と同じ設定で描く。連鎖のすべてのフォントに効く。
    pub fn with_proportional(mut self) -> Self {
        self.proportional = true;
        self
    }

    /// 仮名をプロポーショナルに詰めるか([`FontSet::with_proportional`])。
    pub fn is_proportional(&self) -> bool {
        self.proportional
    }

    /// 連鎖のどれかのフォントが仮名の詰め(`palt` か `vpal`)を持っているか。
    pub fn has_proportional(&self) -> bool {
        self.faces
            .iter()
            .any(|f| !f.palt.is_empty() || !f.vpal.is_empty())
    }

    /// 横書きの詰めの調整(詰めないときや表に無い文字は 0)。
    fn palt(&self, ch: impl Into<Ch>) -> Adjust {
        let ch = ch.into();
        if !self.proportional || !is_kana(ch.ch) {
            return Adjust::default();
        }
        self.glyph(ch)
            .and_then(|g| self.faces[g.font].palt.get(&g.id).copied())
            .unwrap_or_default()
    }

    /// 縦書きの詰めの調整(縦書き用の字形があればその字形で引く)。
    fn vpal(&self, ch: impl Into<Ch>) -> Adjust {
        let ch = ch.into();
        if !self.proportional || !is_kana(ch.ch) {
            return Adjust::default();
        }
        self.vertical_glyph(ch)
            .or_else(|| self.glyph(ch))
            .and_then(|g| self.faces[g.font].vpal.get(&g.id).copied())
            .unwrap_or_default()
    }

    /// 横書きで字形を置く位置の右へのずれ(全角比)。詰めないときは 0。
    pub(crate) fn palt_shift(&self, ch: Ch) -> f32 {
        self.palt(ch).place
    }

    /// 縦書きで字形を置く位置の下へのずれ(全角比)。詰めないときは 0。
    pub(crate) fn vpal_shift(&self, ch: Ch) -> f32 {
        self.vpal(ch).place
    }

    /// 詰めない横書きの送り幅(全角比)。
    pub(crate) fn full_advance(&self, ch: char) -> f32 {
        let px = self.base_px;
        let g = self.glyph(ch).unwrap_or(NOTDEF);
        self.faces[g.font]
            .font
            .metrics_indexed(g.id, px)
            .advance_width
            / px
    }

    fn rasterize_glyph(&self, g: FontGlyph, px: f32) -> Raster {
        let (m, bitmap) = self.faces[g.font].font.rasterize_indexed(g.id, px);
        Raster {
            width: m.width as u32,
            height: m.height as u32,
            xmin: m.xmin,
            ymin: m.ymin,
            advance: m.advance_width,
            bitmap,
        }
    }

    /// 字の枠の上端からベースラインまでの距離(px)。本命のフォントで決める。
    ///
    /// OS/2 の typo ascender を使い、無ければ hhea の ascender、それも無ければ 0.88 倍。
    /// hhea の値は行の上下の余白を含むことが多く(Zen Maru Gothic は 1.16)、そのまま使うと
    /// 字が論理座標の字の枠より下へずれる。
    pub fn ascent(&self, px: f32) -> f32 {
        let face = &self.faces[0];
        if let Some(a) = face.ascent {
            return a * px;
        }
        face.font
            .horizontal_line_metrics(px)
            .map(|l| l.ascent)
            .unwrap_or(px * 0.88)
    }
}

impl FontMetrics for FontSet {
    /// 文字を描くフォント(連鎖の中で字形を持つもの)の送り幅。
    /// 詰めるとき([`FontSet::with_proportional`])は仮名に `palt` を効かせる。
    fn advance(&self, ch: char, size: f32) -> f32 {
        (self.full_advance(ch) + self.palt(ch).advance).max(0.0) * size
    }

    /// 寝かせる文字は詰めない横の送り幅、それ以外は全角1文字。
    /// 詰めるとき([`FontSet::with_proportional`])は仮名に `vpal` を効かせる。
    fn vertical_advance(&self, ch: char, size: f32) -> f32 {
        match nekomoji::chars::vertical_form(ch) {
            nekomoji::chars::VerticalForm::Sideways => self.full_advance(ch) * size,
            _ => (1.0 + self.vpal(ch).advance).max(0.0) * size,
        }
    }

    /// 本命のフォントの strikeout(OS/2)・underline(post)の値を使う。無ければ既定の比率。
    ///
    /// 文字はベースラインを「字の枠の上端 + ascent」に置くので、線もそれに合わせる。
    fn line_metrics(&self, kind: LineKind, size: f32) -> (f32, f32) {
        let m = match kind {
            LineKind::Strike => self.faces[0].strike,
            LineKind::Underline => self.faces[0].underline,
            _ => None,
        };
        match m {
            Some((top, thickness)) if thickness > 0.0 => {
                let ascent = self.ascent(self.base_px) / self.base_px;
                ((ascent - top + thickness / 2.0) * size, thickness * size)
            }
            _ => {
                let (y, t) = nekomoji::default_line_metrics(kind);
                (y * size, t * size)
            }
        }
    }
}

/// OS/2 の strikeout と post の underline を全角比で読む。
type LineMetric = Option<(f32, f32)>;
fn read_line_metrics(data: &[u8], index: u32) -> (LineMetric, LineMetric) {
    let Ok(face) = ttf_parser::Face::parse(data, index) else {
        return (None, None);
    };
    let em = face.units_per_em() as f32;
    let conv = |m: ttf_parser::LineMetrics| (m.position as f32 / em, m.thickness as f32 / em);
    (
        face.strikeout_metrics().map(conv),
        face.underline_metrics().map(conv),
    )
}

/// OS/2 の typo ascender を全角比で読む。無いか 0 以下なら `None`。
fn read_typo_ascender(data: &[u8], index: u32) -> Option<f32> {
    let face = ttf_parser::Face::parse(data, index).ok()?;
    let a = face.typographic_ascender()?;
    (a > 0).then(|| a as f32 / face.units_per_em() as f32)
}

/// cmap の format 14(異体字の表)のうち、既定と違う字形(non-default UVS)を読む。読めなければ空。
///
/// ttf-parser は format 14 の中身を列挙できないので、cmap の表を直接読む。
fn read_variants(data: &[u8], index: u32) -> HashMap<(char, char), u16> {
    let mut map = HashMap::new();
    let Ok(face) = ttf_parser::Face::parse(data, index) else {
        return map;
    };
    let Some(cmap) = face.raw_face().table(ttf_parser::Tag::from_bytes(b"cmap")) else {
        return map;
    };
    let u16_at = |b: &[u8], o: usize| b.get(o..o + 2).map(|x| u16::from_be_bytes([x[0], x[1]]));
    let u24_at = |b: &[u8], o: usize| {
        b.get(o..o + 3)
            .map(|x| u32::from_be_bytes([0, x[0], x[1], x[2]]))
    };
    let u32_at = |b: &[u8], o: usize| {
        b.get(o..o + 4)
            .map(|x| u32::from_be_bytes([x[0], x[1], x[2], x[3]]))
    };
    let tables = u16_at(cmap, 2).unwrap_or(0) as usize;
    for t in 0..tables {
        let rec = 4 + t * 8;
        let (Some(platform), Some(encoding), Some(offset)) = (
            u16_at(cmap, rec),
            u16_at(cmap, rec + 2),
            u32_at(cmap, rec + 4),
        ) else {
            break;
        };
        if platform != 0 || encoding != 5 {
            continue;
        }
        let Some(sub) = cmap.get(offset as usize..) else {
            continue;
        };
        if u16_at(sub, 0) != Some(14) {
            continue;
        }
        let records = u32_at(sub, 6).unwrap_or(0) as usize;
        for r in 0..records {
            let at = 10 + r * 11;
            let (Some(vs), Some(non_default)) = (u24_at(sub, at), u32_at(sub, at + 7)) else {
                break;
            };
            let Some(vs) = char::from_u32(vs) else {
                continue;
            };
            if non_default == 0 {
                continue;
            }
            let base = non_default as usize;
            let count = u32_at(sub, base).unwrap_or(0) as usize;
            for m in 0..count {
                let at = base + 4 + m * 5;
                let (Some(cp), Some(gid)) = (u24_at(sub, at), u16_at(sub, at + 3)) else {
                    break;
                };
                if let Some(c) = char::from_u32(cp) {
                    map.insert((c, vs), gid);
                }
            }
        }
    }
    map
}

/// プロポーショナル詰めをする仮名: ひらがな・カタカナ(小書き・長音・踊り字を含む)と `々` `〆`。
fn is_kana(c: char) -> bool {
    matches!(c as u32,
        0x3041..=0x3096 | 0x309D..=0x309F | 0x30A1..=0x30FA | 0x30FC..=0x30FF
        | 0x31F0..=0x31FF | 0x3005 | 0x3006)
}

/// 仮名の字形の、GPOS の `palt`(横書き)と `vpal`(縦書き)の単一調整を読む。読めなければ空。
///
/// 調べるのは仮名の字形(cmap で引いたものと、その縦書き用の字形)だけ。`vpal` の位置は
/// 上向きが正なので、下向きの正に直す。
fn read_proportional(
    data: &[u8],
    index: u32,
    vert: &HashMap<u16, u16>,
) -> (HashMap<u16, Adjust>, HashMap<u16, Adjust>) {
    use ttf_parser::gpos::{PositioningSubtable, SingleAdjustment};
    use ttf_parser::{GlyphId, Tag};

    let (mut palt, mut vpal) = (HashMap::new(), HashMap::new());
    let Ok(face) = ttf_parser::Face::parse(data, index) else {
        return (palt, vpal);
    };
    let Some(gpos) = face.tables().gpos else {
        return (palt, vpal);
    };
    let em = face.units_per_em() as f32;
    let mut glyphs: Vec<u16> = (0x3005..=0x31FF)
        .filter_map(char::from_u32)
        .filter(|&c| is_kana(c))
        .filter_map(|c| face.glyph_index(c))
        .map(|g| g.0)
        .collect();
    let verticals: Vec<u16> = glyphs.iter().filter_map(|g| vert.get(g).copied()).collect();
    glyphs.extend(verticals);
    glyphs.sort_unstable();
    glyphs.dedup();
    for (tag, map, vertical) in [
        (Tag::from_bytes(b"palt"), &mut palt, false),
        (Tag::from_bytes(b"vpal"), &mut vpal, true),
    ] {
        let mut lookups: Vec<u16> = gpos
            .features
            .into_iter()
            .filter(|f| f.tag == tag)
            .flat_map(|f| f.lookup_indices)
            .collect();
        lookups.sort_unstable();
        lookups.dedup();
        for li in lookups {
            let Some(lookup) = gpos.lookups.get(li) else {
                continue;
            };
            for sub in lookup.subtables.into_iter::<PositioningSubtable>() {
                let PositioningSubtable::Single(single) = sub else {
                    continue;
                };
                for &g in &glyphs {
                    if map.contains_key(&g) {
                        continue;
                    }
                    let v = match &single {
                        SingleAdjustment::Format1 { coverage, value } => {
                            coverage.get(GlyphId(g)).map(|_| *value)
                        }
                        SingleAdjustment::Format2 { coverage, values } => {
                            coverage.get(GlyphId(g)).and_then(|i| values.get(i))
                        }
                    };
                    let Some(v) = v else {
                        continue;
                    };
                    let a = if vertical {
                        Adjust {
                            place: -(v.y_placement as f32) / em,
                            advance: v.y_advance as f32 / em,
                        }
                    } else {
                        Adjust {
                            place: v.x_placement as f32 / em,
                            advance: v.x_advance as f32 / em,
                        }
                    };
                    if a != Adjust::default() {
                        map.insert(g, a);
                    }
                }
            }
        }
    }
    (palt, vpal)
}

/// GSUB の `vert`・`vrt2` の単純置換を読む。読めなければ空。
fn read_vert(data: &[u8], index: u32) -> HashMap<u16, u16> {
    use ttf_parser::gsub::{SingleSubstitution, SubstitutionSubtable};
    use ttf_parser::{GlyphId, Tag};

    let mut map = HashMap::new();
    let Ok(face) = ttf_parser::Face::parse(data, index) else {
        return map;
    };
    let Some(gsub) = face.tables().gsub else {
        return map;
    };
    let tags = [Tag::from_bytes(b"vert"), Tag::from_bytes(b"vrt2")];
    let mut lookups: Vec<u16> = gsub
        .features
        .into_iter()
        .filter(|f| tags.contains(&f.tag))
        .flat_map(|f| f.lookup_indices)
        .collect();
    lookups.sort_unstable();
    lookups.dedup();
    let glyphs = face.number_of_glyphs();
    for li in lookups {
        let Some(lookup) = gsub.lookups.get(li) else {
            continue;
        };
        for sub in lookup.subtables.into_iter::<SubstitutionSubtable>() {
            let SubstitutionSubtable::Single(single) = sub else {
                continue;
            };
            for g in 1..glyphs {
                if map.contains_key(&g) {
                    continue;
                }
                let to = match &single {
                    SingleSubstitution::Format1 { coverage, delta } => coverage
                        .get(GlyphId(g))
                        .map(|_| (g as i32 + *delta as i32) as u16),
                    SingleSubstitution::Format2 {
                        coverage,
                        substitutes,
                    } => coverage
                        .get(GlyphId(g))
                        .and_then(|i| substitutes.get(i))
                        .map(|id| id.0),
                };
                if let Some(to) = to.filter(|&t| t != g) {
                    map.insert(g, to);
                }
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 読めないデータからは置き換えを作らない() {
        assert!(read_vert(&[0, 1, 2, 3], 0).is_empty());
    }

    #[test]
    fn 読めないデータからは詰めの表を作らない() {
        let (palt, vpal) = read_proportional(&[0, 1, 2, 3], 0, &HashMap::new());
        assert!(palt.is_empty() && vpal.is_empty());
    }

    #[test]
    fn 詰めるのは仮名だけ() {
        for c in ['あ', 'ん', 'ゝ', 'ア', 'ヶ', 'ー', 'ヽ', 'ㇰ', '々', '〆'] {
            assert!(is_kana(c), "{c}");
        }
        for c in ['、', '。', '「', '』', '・', '漢', 'A', 'ｱ', '〜'] {
            assert!(!is_kana(c), "{c}");
        }
    }

    /// palt・vpal を持つフォント(Noto Sans CJK)がある環境でだけ確かめる。
    #[test]
    fn 詰めると仮名の送りが狭くなり約物と漢字は変わらない() {
        let path = std::env::var("NEKOMOJI_IVS_FONT")
            .unwrap_or_else(|_| "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into());
        let Ok(data) = std::fs::read(&path) else {
            eprintln!("{path} が無いので飛ばす");
            return;
        };
        let full = FontSet::from_bytes(&data, 0, 32.0).unwrap();
        assert!(full.has_proportional());
        assert!(!full.is_proportional());
        let font = FontSet::from_bytes(&data, 0, 32.0)
            .unwrap()
            .with_proportional();
        for c in ['く', 'ト', 'ッ'] {
            assert!(font.advance(c, 1.0) < full.advance(c, 1.0) - 0.01, "{c}");
            assert!(
                font.vertical_advance(c, 1.0) < full.vertical_advance(c, 1.0) - 0.01,
                "{c}"
            );
        }
        for c in ['、', '。', '「', '漢'] {
            assert_eq!(font.advance(c, 1.0), full.advance(c, 1.0), "{c}");
            assert_eq!(
                font.vertical_advance(c, 1.0),
                full.vertical_advance(c, 1.0),
                "{c}"
            );
            assert_eq!(font.palt_shift(c.into()), 0.0);
        }
        // 大きさに比例する
        assert!((font.advance('く', 2.0) - font.advance('く', 1.0) * 2.0).abs() < 1e-5);
        // 詰めないときはずれも 0
        assert_eq!(full.palt_shift('く'.into()), 0.0);
        assert_eq!(full.vpal_shift('く'.into()), 0.0);
    }

    #[test]
    fn 読めないデータからは異体字の表を作らない() {
        assert!(read_variants(&[0, 1, 2, 3], 0).is_empty());
    }

    #[test]
    fn 読めないデータからは_typo_ascender_を読まない() {
        assert_eq!(read_typo_ascender(&[0, 1, 2, 3], 0), None);
    }

    /// 同梱の Zen Maru Gothic(hhea 1160・typo 880)がある環境でだけ確かめる。
    #[test]
    fn ベースラインは_hhea_ではなく_typo_ascender_で決める() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/fonts/ZenMaruGothic-Regular.ttf"
        );
        let Ok(data) = std::fs::read(path) else {
            eprintln!("{path} が無いので飛ばす");
            return;
        };
        let font = FontSet::from_bytes(&data, 0, 100.0).unwrap();
        assert!((font.ascent(100.0) - 88.0).abs() < 0.01);
        assert!((font.ascent(50.0) - 44.0).abs() < 0.01);
    }

    #[test]
    fn 書記素クラスタの続きの最初が異体字セレクタなら使う() {
        assert_eq!(Ch::new('辻', "\u{E0100}").vs, Some('\u{E0100}'));
        assert_eq!(Ch::new('葛', "\u{FE00}x").vs, Some('\u{FE00}'));
        assert_eq!(Ch::new('か', "\u{3099}").vs, None);
        assert_eq!(Ch::new('あ', "").vs, None);
    }

    /// IVS を持つフォント(Noto Sans CJK)がある環境でだけ確かめる。
    #[test]
    fn 異体字セレクタ付きの文字は異体字の字形を引く() {
        let path = std::env::var("NEKOMOJI_IVS_FONT")
            .unwrap_or_else(|_| "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into());
        let Ok(data) = std::fs::read(&path) else {
            eprintln!("{path} が無いので飛ばす");
            return;
        };
        let font = FontSet::from_bytes(&data, 0, 32.0).unwrap();
        assert!(font.has_variants());
        let plain = font.glyph('辻').unwrap();
        let ivs = font.glyph(Ch::new('辻', "\u{E0100}")).unwrap();
        assert_ne!(plain, ivs, "辻󠄀(一点しんにょう)は別の字形");
        // 表に無い組み合わせは元の字形
        assert_eq!(font.glyph(Ch::new('あ', "\u{E0100}")), font.glyph('あ'));
    }
}
