//! CLI・REPL の設定と、オプションの値(動きの強さ・字下げ・揃え・文字送りの音)の読み書き。

use std::fmt::Write as _;

use nekomoji::{
    parse_all, Align, Blip, CellMetrics, Indent, Kinsoku, Layout, LayoutOptions, Motion, Neko,
    NekoMode, ParseError, RevealOptions, Vars, Writing,
};

/// CLI・REPL の設定。
///
/// 端末では半角未満の幅を表せないので、既定では約物の詰めと和欧間のアキを切る
/// (`--tsume`・`--wao` で有効にできる)。
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// レイアウトの設定(幅の単位は全角1文字)
    pub layout: LayoutOptions,
    /// `log` にページごとのオートモードの待ち時間を出すか
    pub auto: bool,
    /// `log` にページごとの読み上げ用のテキストを出すか
    pub speech: bool,
    /// ボイスの長さ(秒)。オートモードの待ち時間に使う
    pub voice_lens: std::collections::BTreeMap<String, f32>,
    /// 文字送りの設定
    pub reveal: RevealOptions,
    /// 変数
    pub vars: Vars,
    /// 猫モード(`None` は無効)
    pub neko: Option<NekoMode>,
    /// 猫を絵文字でなく ASCII(`=^.^=`)で表示する
    pub ascii: bool,
}

/// 動きの強さを `full` / `reduced` / `off` から読む。
pub fn parse_motion(s: &str) -> Option<Motion> {
    match s {
        "full" => Some(Motion::Full),
        "reduced" => Some(Motion::Reduced),
        "off" => Some(Motion::Off),
        _ => None,
    }
}

/// 字下げの「1行目\[,2行目以降\]」を読む。2行目以降を省くと `rest` のまま。
pub fn parse_indent_pair(s: &str, rest: f32) -> Option<(f32, f32)> {
    let num = |v: &str| {
        v.trim()
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite() && *v >= 0.0)
    };
    match s.split_once(',') {
        Some((a, b)) => Some((num(a)?, num(b)?)),
        None => Some((num(s)?, rest)),
    }
}

/// 地の文の字下げを読む: `novel`・`dialogue`・`off` か「1行目\[,2行目以降\]」。
/// `novel`・`dialogue`・`off` は会話の字下げも含めて置き換える。
pub fn apply_indent(indent: &mut Indent, s: &str) -> bool {
    match s {
        "novel" => *indent = Indent::novel(),
        "dialogue" => *indent = Indent::dialogue(),
        "off" => *indent = Indent::default(),
        _ => match parse_indent_pair(s, indent.rest) {
            Some((a, b)) => {
                indent.first = a;
                indent.rest = b;
            }
            None => return false,
        },
    }
    true
}

/// 会話(括弧で始まる段落)の字下げ「1行目\[,2行目以降\]」を読む。
pub fn apply_bracket_indent(indent: &mut Indent, s: &str) -> bool {
    match parse_indent_pair(s, indent.bracket_rest) {
        Some((a, b)) => {
            indent.bracket_first = a;
            indent.bracket_rest = b;
            true
        }
        None => false,
    }
}

/// 揃えを `start`・`center`・`end`(`left`・`right` も可)から読む。
pub fn parse_align(s: &str) -> Option<Align> {
    Align::from_name(s)
}

/// 揃えの表示名。
pub fn align_name(a: Align) -> &'static str {
    match a {
        Align::Center => "中央",
        Align::End => "行末",
        _ => "行頭",
    }
}

/// 文字送りの音の指定 `ID[,文字数]` を読む。`off` は `None`(本文全体では鳴らさない)。
pub fn parse_blip(s: &str) -> Option<Option<Blip>> {
    if s == "off" {
        return Some(None);
    }
    let (id, every) = match s.split_once(',') {
        Some((id, n)) => (id, n.trim().parse::<usize>().ok().filter(|n| *n > 0)?),
        None => (s, Blip::default().every),
    };
    let id = id.trim();
    if id.is_empty() || id.contains(char::is_whitespace) {
        return None;
    }
    Some(Some(Blip::default().id(id).every(every)))
}

/// 動きの強さの表示名。
pub fn motion_name(m: Motion) -> &'static str {
    match m {
        Motion::Full => "標準",
        Motion::Reduced => "控えめ",
        Motion::Off => "なし",
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            layout: LayoutOptions::default()
                .punct_squeeze(false)
                .latin_space(0.0),
            auto: false,
            speech: false,
            voice_lens: Default::default(),
            reveal: RevealOptions::default(),
            vars: Vars::default(),
            neko: None,
            ascii: false,
        }
    }
}

impl Settings {
    /// 設定の1行要約。
    pub fn summary(&self) -> String {
        let neko = match &self.neko {
            Some(m) => format!(
                " / 猫 seed {} 落とす {} 爆散 {} パンチ {}{}",
                m.seed,
                trim_num(m.knock_chance),
                trim_num(m.burst_chance),
                trim_num(m.punch_chance),
                if self.ascii { " ascii" } else { "" }
            ),
            None => String::new(),
        };
        let tate = match self.layout.writing {
            Writing::Horizontal => "",
            Writing::Vertical => " / 縦書き",
        };
        let mut lines = match self.layout.max_lines {
            Some(n) => format!(" / {n} 行で改ページ"),
            None => String::new(),
        };
        if self.layout.punct_squeeze {
            lines.push_str(" / 約物の詰め");
        }
        if self.layout.latin_space > 0.0 {
            let _ = write!(lines, " / 和欧間 {}", trim_num(self.layout.latin_space));
        }
        if self.layout.justify {
            lines.push_str(" / 両端揃え");
        }
        if self.layout.align != Align::Start {
            let _ = write!(lines, " / {}揃え", align_name(self.layout.align));
        }
        let i = self.layout.indent;
        if i != Indent::default() {
            let _ = write!(
                lines,
                " / 字下げ {},{} 会話 {},{}",
                trim_num(i.first),
                trim_num(i.rest),
                trim_num(i.bracket_first),
                trim_num(i.bracket_rest)
            );
        }
        let neko = if self.auto {
            format!("{neko} / オート")
        } else {
            neko
        };
        let neko = match &self.reveal.blip {
            Some(b) => format!("{neko} / 音 {} {}文字ごと", b.id, b.every.max(1)),
            None => neko,
        };
        let neko = match self.reveal.motion {
            Motion::Full => neko,
            m => format!("{neko} / 動き {}", motion_name(m)),
        };
        format!(
            "幅 {}{lines} / 禁則 {} / ぶら下げ {}{tate} / {} 文字/秒{neko}",
            trim_num(self.layout.max_width),
            match self.layout.kinsoku {
                Kinsoku::Standard => "standard",
                Kinsoku::Off => "off",
                _ => "?",
            },
            if self.layout.hanging { "on" } else { "off" },
            trim_num(self.reveal.chars_per_sec),
        )
    }

    /// 猫モードが有効なら、そのレイアウトの [`Neko`] を作る。
    pub fn neko(&self, layout: &Layout) -> Option<Neko> {
        self.neko.clone().map(|m| Neko::new(layout, m))
    }

    /// マークアップからレイアウトを組む。解析エラーはすべて位置の順に返す([`parse_all`])。
    pub fn build(&self, src: &str) -> Result<Layout, Vec<ParseError>> {
        let script = parse_all(src)?;
        Ok(Layout::build(
            &script,
            &CellMetrics,
            &self.layout,
            &self.vars,
        ))
    }
}

fn trim_num(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
