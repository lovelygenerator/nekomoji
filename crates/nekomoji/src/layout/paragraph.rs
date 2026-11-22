//! 本文を書記素クラスタの塊にし、ページ・段落に分ける。字下げのための段落の種類も決める。

use std::ops::Range;

use super::line_break::first_char;
use super::options::{Indent, Vars};
use super::page::{Cue, ICON_CHAR};
use crate::markup::{Item, Script};
use crate::text::grapheme::graphemes;
use crate::text::kinsoku::{is_closing_punct, is_opening_bracket};
use crate::text::line_class::{line_class, LineClass};

/// 書記素クラスタ(見た目の1文字)。`ch` は先頭の文字、`tail` は続く文字。
#[derive(Debug, Clone)]
pub(super) struct Cluster {
    pub(super) ch: char,
    pub(super) tail: String,
    /// 元の文章のバイト範囲
    pub(super) source: Range<usize>,
    /// 変数から来た文字なら、その変数の番号
    pub(super) var: Option<usize>,
}

/// 元の文章の位置。1文字ごとの範囲か、変数なら `[var name]` 全体と変数の番号。
#[derive(Clone, Copy)]
enum Origin<'a> {
    Chars(&'a [Range<usize>]),
    Var(&'a Range<usize>, usize),
}

/// 文字列を書記素クラスタの列にする。
fn clusters(s: &str, origin: Origin<'_>) -> Vec<Cluster> {
    let mut at = 0;
    graphemes(s)
        .into_iter()
        .map(|g| {
            let n = g.chars().count();
            let mut it = g.chars();
            let ch = it.next().expect("書記素クラスタは1文字以上");
            let (source, var) = match origin {
                Origin::Chars(src) => {
                    let first = src.get(at).map_or(0, |r| r.start);
                    let last = src.get(at + n - 1).map_or(first, |r| r.end);
                    (first..last, None)
                }
                Origin::Var(r, v) => (r.clone(), Some(v)),
            };
            at += n;
            Cluster {
                ch,
                tail: it.as_str().to_string(),
                source,
                var,
            }
        })
        .collect()
}

/// 分割できない塊(1文字、英単語、ルビ付きの親文字)。`chars` は書記素クラスタの列。
#[derive(Debug, Clone)]
pub(super) struct Unit {
    pub(super) chars: Vec<Cluster>,
    pub(super) style: usize,
    pub(super) ruby: Option<String>,
    /// 縦中横(`[tcy]`)の塊か
    pub(super) tcy: bool,
    /// 文中のアイコン(`[icon]`)の ID と長さ
    pub(super) icon: Option<(String, f32)>,
    /// この塊の前に起きる合図
    pub(super) cues: Vec<Cue>,
    /// 前の塊とつながった英単語・数の続きで、この塊の前では改行しない
    /// (`hel[b]lo[/b]` のようにスタイルの境目で分かれた語)
    pub(super) glue: bool,
}

/// 改行で区切られた段落(塊の列と、末尾の合図)。
#[derive(Debug, Default)]
pub(super) struct Paragraph {
    pub(super) units: Vec<Unit>,
    pub(super) tail_cues: Vec<Cue>,
    pub(super) kind: ParaKind,
    /// まだ塊に分けていない文字(スタイルの番号と、その文字の前に起きる合図)
    text: Vec<(Cluster, usize, Vec<Cue>)>,
}

/// 字下げのための段落の種類。
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(super) enum ParaKind {
    /// 地の文
    #[default]
    Plain,
    /// 全角の空白で始まる地の文(手で字下げしてある)
    ManualIndent,
    /// 始め括弧で始まる段落(会話)
    Bracket,
    /// 前の段落で開いた括弧が閉じていない段落(会話の続き)
    Continued,
}

/// 閉じ括弧か(句読点を除く)。
fn is_closing_bracket(ch: char) -> bool {
    is_closing_punct(ch) && !matches!(ch, '、' | '。' | '，' | '．')
}

/// 段落の種類を決める。開いたまま閉じていない括弧の数を、段落・ページをまたいで数える。
fn classify_paragraphs(pages: &mut [PageSrc]) {
    let mut depth = 0usize;
    for para in pages.iter_mut().flat_map(|p| p.paragraphs.iter_mut()) {
        let first = para.units.first().map(first_char);
        // 段落ごとに始め括弧を開き直す書き方(「…/「…」。閉じるのは最後の段落だけ)では、
        // 開き直した段落を新しい会話とし、閉じていない括弧の数を数え直す
        if depth > 0 && first.is_some_and(is_opening_bracket) {
            depth = 0;
        }
        para.kind = match first {
            _ if depth > 0 => ParaKind::Continued,
            Some(c) if is_opening_bracket(c) => ParaKind::Bracket,
            Some('\u{3000}') => ParaKind::ManualIndent,
            _ => ParaKind::Plain,
        };
        for c in para.units.iter().flat_map(|u| &u.chars) {
            if is_opening_bracket(c.ch) {
                depth += 1;
            } else if is_closing_bracket(c.ch) {
                depth = depth.saturating_sub(1);
            }
        }
    }
}

impl Indent {
    /// 段落の (1行目, 2行目以降) の字下げ。
    pub(super) fn of(&self, kind: ParaKind) -> (f32, f32) {
        let (a, b) = match kind {
            ParaKind::Plain => (self.first, self.rest),
            ParaKind::ManualIndent => (0.0, self.rest),
            ParaKind::Bracket => (self.bracket_first, self.bracket_rest),
            ParaKind::Continued => (self.bracket_rest, self.bracket_rest),
        };
        let ok = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        (ok(a), ok(b))
    }
}

#[derive(Default)]
pub(super) struct PageSrc {
    pub(super) paragraphs: Vec<Paragraph>,
}

pub(super) fn split_pages(
    script: &Script,
    vars: &Vars,
    missing: &mut Vec<String>,
    used: &mut Vec<String>,
) -> Vec<PageSrc> {
    let mut pages = vec![PageSrc::default()];
    let mut para = Paragraph::default();
    let mut pending: Vec<Cue> = Vec::new();

    /// 文字を、塊に分ける前の並びに足す(合図は次の文字の前に起きる)。
    fn push_text(
        text: &str,
        origin: Origin<'_>,
        style: usize,
        para: &mut Paragraph,
        pending: &mut Vec<Cue>,
    ) {
        for c in clusters(text, origin) {
            para.text.push((c, style, std::mem::take(pending)));
        }
    }

    /// 並べた文字を塊に分ける。英数字の語・数(`3.14` `1,000` `10:30` `$100` `100%`)は1語にし、
    /// `well-known` はハイフンの後ろでだけ改行できるようにする。語はスタイル・合図の境目を
    /// またいで判定し(`3[b].14[/b]`)、境目では塊を分けて `glue` でつなぐ。
    fn flush_text(para: &mut Paragraph) {
        let cs = std::mem::take(&mut para.text);
        let class = |k: usize| cs.get(k).map(|c| line_class(c.0.ch));
        let digit = |c: Option<LineClass>| c == Some(LineClass::Numeric);
        // 前の字と同じ語か(分類は crate::text::line_class の表)
        let mut join = vec![false; cs.len()];
        let mut last: Option<LineClass> = None;
        for k in 0..cs.len() {
            let (c, kind, next) = (cs[k].0.ch, line_class(cs[k].0.ch), class(k + 1));
            let in_word = matches!(kind, LineClass::Alpha | LineClass::Numeric)
                // 語の中のアポストロフィ(it's)
                || (matches!(c, '\'' | '’') && last == Some(LineClass::Alpha))
                // 数の中の区切り(3.14・1,000・10:30・2026/11/10・2026-11-10)
                || (matches!(c, '.' | ',' | ':' | '/' | '-') && digit(last) && digit(next))
                // 数の前の記号(US$100 も1語)と後ろの記号(100%・30℃)
                || (kind == LineClass::Prefix && digit(next))
                || (kind == LineClass::Postfix && digit(last));
            if in_word {
                join[k] = last.is_some();
                last = Some(kind);
            } else if kind == LineClass::Hyphen && last == Some(LineClass::Alpha) {
                // ハイフンは前の語に付け、その後ろで改行できるようにする
                join[k] = true;
                last = None;
            } else {
                last = None;
            }
        }
        for (k, (c, style, cues)) in cs.into_iter().enumerate() {
            if join[k] {
                if let Some(u) = para
                    .units
                    .last_mut()
                    .filter(|u| u.style == style && cues.is_empty())
                {
                    u.chars.push(c);
                    continue;
                }
            }
            para.units.push(Unit {
                chars: vec![c],
                style,
                ruby: None,
                tcy: false,
                icon: None,
                cues,
                glue: join[k],
            });
        }
    }

    for item in &script.items {
        // ルビ・縦中横・アイコン・改行の前に、それまでの文字を塊に分ける(待機・イベントは語を切らない)
        if !matches!(
            item,
            Item::Text { .. } | Item::Var { .. } | Item::Wait(_) | Item::Click | Item::Event(_)
        ) {
            flush_text(&mut para);
        }
        match item {
            Item::Text {
                text,
                style,
                source,
            } => push_text(text, Origin::Chars(source), *style, &mut para, &mut pending),
            Item::Var {
                name,
                style,
                source,
            } => {
                let id = used.iter().position(|v| v == name).unwrap_or_else(|| {
                    used.push(name.clone());
                    used.len() - 1
                });
                let value = match vars.get(name) {
                    Some(v) => v.clone(),
                    None => {
                        if !missing.contains(name) {
                            missing.push(name.clone());
                        }
                        format!("[var {name}]")
                    }
                };
                // 値の改行は改行(段落の区切り)にする。タブは空白にし、ほかの制御文字は捨てる
                let value: String = value
                    .replace("\r\n", "\n")
                    .chars()
                    .filter_map(|c| match c {
                        '\n' => Some('\n'),
                        '\t' => Some(' '),
                        c if c.is_control() => None,
                        c => Some(c),
                    })
                    .collect();
                for (k, part) in value.split('\n').enumerate() {
                    if k > 0 {
                        flush_text(&mut para);
                        para.tail_cues = std::mem::take(&mut pending);
                        pages
                            .last_mut()
                            .expect("ページは1つ以上")
                            .paragraphs
                            .push(std::mem::take(&mut para));
                    }
                    push_text(
                        part,
                        Origin::Var(source, id),
                        *style,
                        &mut para,
                        &mut pending,
                    );
                }
            }
            Item::Ruby {
                base,
                reading,
                style,
                source,
            } => para.units.push(Unit {
                chars: clusters(base, Origin::Chars(source)),
                style: *style,
                ruby: Some(reading.clone()),
                tcy: false,
                icon: None,
                cues: std::mem::take(&mut pending),
                glue: false,
            }),
            Item::Tcy {
                text,
                style,
                source,
            } => para.units.push(Unit {
                chars: clusters(text, Origin::Chars(source)),
                style: *style,
                ruby: None,
                tcy: true,
                icon: None,
                cues: std::mem::take(&mut pending),
                glue: false,
            }),
            Item::Icon {
                id,
                width,
                style,
                source,
            } => para.units.push(Unit {
                chars: vec![Cluster {
                    ch: ICON_CHAR,
                    tail: String::new(),
                    source: source.clone(),
                    var: None,
                }],
                style: *style,
                ruby: None,
                tcy: false,
                icon: Some((id.clone(), *width)),
                cues: std::mem::take(&mut pending),
                glue: false,
            }),
            Item::Wait(s) => pending.push(Cue::Wait(*s)),
            Item::Click => pending.push(Cue::Click),
            Item::Event(e) => pending.push(Cue::Event(e.clone())),
            Item::Break => {
                para.tail_cues = std::mem::take(&mut pending);
                pages
                    .last_mut()
                    .expect("ページは1つ以上")
                    .paragraphs
                    .push(std::mem::take(&mut para));
            }
            Item::Page => {
                para.tail_cues = std::mem::take(&mut pending);
                pages
                    .last_mut()
                    .expect("ページは1つ以上")
                    .paragraphs
                    .push(std::mem::take(&mut para));
                pages.push(PageSrc::default());
            }
        }
    }
    flush_text(&mut para);
    para.tail_cues = std::mem::take(&mut pending);
    let last = pages.last_mut().expect("ページは1つ以上");
    if !para.units.is_empty() || !para.tail_cues.is_empty() || last.paragraphs.is_empty() {
        last.paragraphs.push(para);
    }
    // 末尾の [p] の後に何もなければ、空のページは作らない
    if pages.len() > 1 && pages.last().is_some_and(|p| p.is_blank()) {
        pages.pop();
    }
    classify_paragraphs(&mut pages);
    pages
}

impl PageSrc {
    fn is_blank(&self) -> bool {
        self.paragraphs
            .iter()
            .all(|p| p.units.is_empty() && p.tail_cues.is_empty())
    }
}
