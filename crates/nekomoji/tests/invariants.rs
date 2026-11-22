//! 不変条件のテスト: 組版と文字送りの「どんな入力でも成り立つ性質」を、生成した入力で確かめる。
//!
//! `robustness.rs` がパニックしないことと出力の構造を見るのに対し、こちらは見た目の正しさを見る。
//! 入力は禁則・英単語・数・空白・句読点の間に関わる断片を多めに混ぜる。仕様(docs/markup.md の
//! 「組版の規則」)を、実装とは別に書いた判定(この中の `oracle`)と突き合わせる。
//!
//! 失敗したときはシード・入力・設定・行が表示されるので、そのまま単体テストにできる。
//! 回数は `NEKOMOJI_INVARIANT_CASES` で増やせる(既定 3000)。

use nekomoji::chars::{
    is_hangable, is_inseparable, is_latin, is_line_end_forbidden, is_line_start_forbidden,
};
use nekomoji::{
    parse, Align, CellMetrics, Kinsoku, Layout, LayoutOptions, Page, Reveal, RevealOptions,
    RevealState, StepKind, Vars, Writing,
};

const EPS: f32 = 1e-3;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn unit(&mut self) -> f32 {
        (self.next() % 10_000) as f32 / 10_000.0
    }
}

/// 組版に関わる断片。
const ATOMS: &[&str] = &[
    "あ",
    "いう",
    "漢字",
    "カタカナ",
    "ー",
    "っ",
    "ゃ",
    "、",
    "。",
    "！",
    "？",
    "！？",
    "…",
    "……",
    "――",
    "「",
    "」",
    "『",
    "』",
    "（",
    "）",
    "「こんにちは。」",
    "（笑）",
    "hello",
    "world",
    "it's",
    "café",
    "well-known",
    " ",
    "  ",
    "　",
    "3.14",
    "1,000",
    "10:30",
    "2026/11/10",
    "2026-11-10",
    "$100",
    "US$5",
    "¥500",
    "100%",
    "abcdefghijklmnopqrstu",
    ",",
    ".",
    "!",
    "?",
    "(abc)",
    "\"q\"",
    "〜",
    "ｧ",
    "｡",
    "30℃",
    "₩1000",
    "￥500",
    "50％",
    "a:b",
    "and/or",
    "«a»",
    "〝あ〟",
    "——",
    "it’s",
    "\n",
    "[l]",
    "[wait 0.5]",
    "[voice v]",
    "[ruby よみ]読[/ruby]",
    "[ruby ちょうちょう]蝶[/ruby]",
    "[tcy]12[/tcy]",
    "[icon a]",
];

/// 語を割りうる範囲タグ。
const RANGES: &[(&str, &str)] = &[
    ("[b]", "[/b]"),
    ("[color red]", "[/color]"),
    ("[size 1.5]", "[/size]"),
    ("[speed 2]", "[/speed]"),
    ("[u]", "[/u]"),
    ("[em]", "[/em]"),
    ("[mark m]", "[/mark]"),
];

/// 横書きでは縦中横の字が普通の字と見分けられない(`Glyph::tcy` が無い)ので、縦書きのときだけ使う。
fn random_markup(rng: &mut Rng, depth: usize, vertical: bool, out: &mut String) {
    for _ in 0..rng.below(10) + 1 {
        match rng.below(12) {
            0 | 1 if depth < 2 => {
                let (open, close) = RANGES[rng.below(RANGES.len())];
                out.push_str(open);
                random_markup(rng, depth + 1, vertical, out);
                out.push_str(close);
            }
            _ => {
                let atom = ATOMS[rng.below(ATOMS.len())];
                if vertical || !atom.starts_with("[tcy]") {
                    out.push_str(atom);
                }
            }
        }
    }
}

fn random_options(rng: &mut Rng) -> LayoutOptions {
    LayoutOptions::default()
        .writing(if rng.below(3) == 0 {
            Writing::Vertical
        } else {
            Writing::Horizontal
        })
        .max_width([4.0, 5.0, 6.5, 8.0, 12.0, 20.0][rng.below(6)])
        .kinsoku(if rng.below(5) == 0 {
            Kinsoku::Off
        } else {
            Kinsoku::Standard
        })
        .hanging(rng.below(2) == 0)
        .punct_squeeze(rng.below(2) == 0)
        .latin_space([0.0, 0.25][rng.below(2)])
        .justify(rng.below(2) == 0)
        .align([Align::Start, Align::Start, Align::Center, Align::End][rng.below(4)])
}

/// 読点・句点の間(秒)。区別しやすいように大きく取る。
const COMMA: f32 = 1.0;
const PERIOD: f32 = 2.0;
const CPS: f32 = 10.0;

/// 仕様を実装とは別に書いた判定。
mod oracle {
    use super::*;

    /// 数の前に付く記号・後ろに付く記号(docs/markup.md の分類の表)。
    const PREFIX: &[char] = &[
        '$', '¥', '£', '€', '₩', '₹', '₽', '₺', '₴', '₱', '₫', '₪', '฿', '＄', '￥', '￡', '￦',
        '№',
    ];
    const POSTFIX: &[char] = &['%', '‰', '‱', '°', '′', '″', '℃', '℉', '¢', '％', '￠'];

    /// 数の中の区切り・数の前後の記号。
    pub fn number_joint(prev: Option<char>, c: char, next: Option<char>) -> bool {
        let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());
        (matches!(c, '.' | ',' | ':' | '/' | '-') && digit(prev) && digit(next))
            || (PREFIX.contains(&c) && digit(next))
            || (POSTFIX.contains(&c) && digit(prev))
    }

    /// 英単語・数の中の文字か(前後の文字とつながっているか)。
    pub fn wordish(prev: Option<char>, c: char, next: Option<char>) -> bool {
        is_latin(c)
            || number_joint(prev, c, next)
            || (matches!(c, '\'' | '’') && prev.is_some_and(|p| is_latin(p) && !p.is_ascii_digit()))
    }

    /// グリフ k が語の中の字か。前後の字は、同じ段落でルビ・縦中横・アイコンでないものだけ見る。
    pub fn word_at(page: &Page, k: usize) -> bool {
        let g = &page.glyphs;
        let para_at = |k: usize| {
            page.lines
                .iter()
                .any(|l| l.paragraph_start && l.first_glyph == k)
        };
        let plain = |k: usize| g.get(k).filter(|_| !in_ruby_or_tcy(page, k)).map(|g| g.ch);
        let prev = if para_at(k) {
            None
        } else {
            k.checked_sub(1).and_then(plain)
        };
        let next = if para_at(k + 1) { None } else { plain(k + 1) };
        !in_ruby_or_tcy(page, k) && wordish(prev, g[k].ch, next)
    }

    /// グリフ i と i+1 の間で改行してよいか(禁則・英単語・数・語の後ろのハイフン・ルビ・縦中横・分離禁止)。
    /// 語の途中は、語がどの行にも収まらないときだけ許す。
    pub fn can_break(page: &Page, i: usize, kinsoku: Kinsoku, room: f32) -> bool {
        let g = &page.glyphs;
        let (a, b) = (&g[i], &g[i + 1]);
        // ルビ・縦中横・アイコンの字は語の前後の字として数えない
        let ch = |k: usize| g.get(k).filter(|_| !in_ruby_or_tcy(page, k)).map(|g| g.ch);
        // 空白の前で改行すると空白は行末に残るので、空白の後ろの文字が行頭に来てよいかで決まる
        if b.ch == ' ' {
            let mut j = i + 1;
            while j < g.len() && g[j].ch == ' ' {
                j += 1;
            }
            return j >= g.len() || kinsoku == Kinsoku::Off || !is_line_start_forbidden(g[j].ch);
        }
        // 同じルビ・同じ縦中横の塊の中
        if page
            .rubies
            .iter()
            .any(|r| !r.emphasis && r.first_glyph <= i && i + 1 < r.first_glyph + r.glyph_count)
        {
            return false;
        }
        if same_tcy(a, b) {
            return false;
        }
        let a_word = word_at(page, i);
        let b_word = word_at(page, i + 1);
        let hyphen = b.ch == '-' && is_latin(a.ch);
        if (a_word && b_word || hyphen) && !in_ruby_or_tcy(page, i) && !in_ruby_or_tcy(page, i + 1)
        {
            // 語の長さ(つながった字の幅の合計)がどの行にも収まらなければ割ってよい
            // (ルビ・縦中横・アイコンは語の外)
            // 語の後ろのハイフン(数の中でないもの)で語は終わる
            let ends_word = |k: usize| {
                g[k].ch == '-' && !number_joint(k.checked_sub(1).and_then(ch), '-', ch(k + 1))
            };
            let para_at = |k: usize| {
                page.lines
                    .iter()
                    .any(|l| l.paragraph_start && l.first_glyph == k)
            };
            let mut s = i;
            while s > 0
                && !para_at(s)
                && !ends_word(s - 1)
                && !in_ruby_or_tcy(page, s - 1)
                && word_at(page, s - 1)
            {
                s -= 1;
            }
            let mut e = i + 1;
            while e + 1 < g.len()
                && !para_at(e + 1)
                && !ends_word(e)
                && !in_ruby_or_tcy(page, e + 1)
                && word_at(page, e + 1)
            {
                e += 1;
            }
            // 語の後ろのハイフンも語に付く
            if !ends_word(e) && e + 1 < g.len() && g[e + 1].ch == '-' && is_latin(g[e].ch) {
                e += 1;
            }
            let width: f32 = g[s..=e].iter().map(|g| g.width).sum();
            if width <= room + EPS {
                return false;
            }
        }
        !(kinsoku == Kinsoku::Standard
            && (is_line_start_forbidden(b.ch)
                || is_line_end_forbidden(a.ch)
                || is_inseparable(a.ch, b.ch)))
    }

    /// 同じ縦中横の塊の中の続く2文字か(隣り合う別の塊は、塊の中の位置が左へ戻る)。
    pub fn same_tcy(a: &nekomoji::Glyph, b: &nekomoji::Glyph) -> bool {
        match (a.tcy, b.tcy) {
            (Some(x), Some(y)) => y.offset > x.offset,
            _ => false,
        }
    }

    fn in_ruby_or_tcy(page: &Page, i: usize) -> bool {
        page.glyphs[i].tcy.is_some()
            || page.glyphs[i].icon.is_some()
            || page
                .rubies
                .iter()
                .any(|r| !r.emphasis && r.first_glyph <= i && i < r.first_glyph + r.glyph_count)
    }

    /// 句読点の間を置く文字と、その間。
    pub fn pause(c: char, next: Option<char>) -> f32 {
        match c {
            ',' if next.is_some_and(|c| c.is_ascii_digit()) => 0.0,
            '、' | '，' | ',' | '…' | '‥' => COMMA,
            '。' | '．' | '！' | '？' | '!' | '?' => PERIOD,
            _ => 0.0,
        }
    }

    /// 句読点の後ろに続くとき、句読点の間をその後ろへ送る文字(句読点・閉じ括弧など)。
    pub fn trails(c: char) -> bool {
        nekomoji::chars::is_closing_punct(c)
            || matches!(
                c,
                '！' | '？'
                    | '!'
                    | '?'
                    | '…'
                    | '‥'
                    | ')'
                    | ']'
                    | '}'
                    | '"'
                    | '\''
                    | '’'
                    | '”'
                    | '〟'
                    | '»'
            )
    }
}

/// 行の中身(後ろの半角空白を除く)のグリフの範囲。
fn content(page: &Page, ln: usize) -> std::ops::Range<usize> {
    let l = &page.lines[ln];
    let mut e = l.first_glyph + l.glyph_count;
    while e > l.first_glyph && page.glyphs[e - 1].ch == ' ' {
        e -= 1;
    }
    l.first_glyph..e
}

fn check_layout(layout: &Layout, opts: &LayoutOptions) -> Result<(), String> {
    let room_min = opts.max_width;
    for (pi, page) in layout.pages.iter().enumerate() {
        for (ln, line) in page.lines.iter().enumerate() {
            let here = format!("ページ {pi} 行 {ln} {:?}", line_text(page, ln));
            let c = content(page, ln);
            // 1. 折り返した行の頭に半角空白を置かない
            if !line.paragraph_start
                && line.glyph_count > 0
                && page.glyphs[line.first_glyph].ch == ' '
            {
                return Err(format!("{here}: 折り返した行の頭が空白"));
            }
            // 2. 改行の位置は、改行してよい位置(どこにも無ければ追い込み)
            if !line.paragraph_start && ln > 0 && line.glyph_count > 0 {
                let i = line.first_glyph - 1;
                let prev = &page.lines[ln - 1];
                if prev.glyph_count > 0
                    && !oracle::can_break(page, i, opts.kinsoku, room_min)
                    && line_has_break(page, ln - 1, opts.kinsoku, room_min)
                {
                    return Err(format!(
                        "{here}: {:?} と {:?} の間で改行した(前の行の中に改行できる位置がある)",
                        page.glyphs[i].ch,
                        page.glyphs[i + 1].ch
                    ));
                }
            }
            // 3. 幅を超えるのは、ぶら下げか、行の中に改行できる位置が無いときだけ
            if c.end > c.start {
                let last = &page.glyphs[c.end - 1];
                // 行の中身の終わり(ルビで広げた塊は字より広いので、字の枠ではなく行の幅で見る)
                let end = line.width;
                let hang =
                    opts.hanging && opts.kinsoku == Kinsoku::Standard && is_hangable(last.ch);
                let content_end = if hang { last.x } else { end };
                if content_end > opts.max_width + EPS
                    && line_has_break(page, ln, opts.kinsoku, room_min)
                    && line.indent < EPS
                {
                    let c2 = content(page, ln);
                    let at = (c2.start..c2.end.saturating_sub(1))
                        .find(|&i| oracle::can_break(page, i, opts.kinsoku, room_min))
                        .map(|i| (page.glyphs[i].ch, page.glyphs[i + 1].ch));
                    return Err(format!(
                        "{here}: 幅 {} を超えた({end})。改行できたはずの位置 {at:?}",
                        opts.max_width
                    ));
                }
                // 4. 揃え: 中央・行末揃えの行は、幅に収まるとき左右のアキが揃う・行末に付く
                if end < opts.max_width - EPS && !hang {
                    match line.align {
                        Align::Center => {
                            let (l, r) = (line.indent, opts.max_width - end);
                            if (l - r).abs() > EPS {
                                return Err(format!("{here}: 中央揃えの左右のアキ {l} と {r}"));
                            }
                        }
                        Align::End => {
                            return Err(format!("{here}: 行末揃えの行末が {end}"));
                        }
                        _ => {}
                    }
                }
                // 5. 両端揃え: 段落の最後でない行は、改行できる位置があれば行末が揃う
                let next_in_para = page
                    .lines
                    .get(ln + 1)
                    .is_some_and(|n| !n.paragraph_start && n.glyph_count > 0);
                if opts.justify
                    && line.align == Align::Start
                    && next_in_para
                    && end < opts.max_width - EPS
                    && !hang
                    && has_gap(page, c.clone(), opts.kinsoku)
                {
                    return Err(format!("{here}: 両端揃えの行末が {end}"));
                }
            }
            // 6. 字形のずれは、同じ行で始め括弧に続く始め括弧だけ。始め括弧の字面は全角の右半分
            //    なので、ずらしても字面は自分の字の枠に収まり、前の括弧の字面に重ならない
            let range = line.first_glyph..line.first_glyph + line.glyph_count;
            for i in range.clone() {
                let g = &page.glyphs[i];
                if g.shift == 0.0 {
                    continue;
                }
                let opening = |c: char| "「『（［｛〈《【〔〘〖".contains(c);
                let prev = (i > range.start).then(|| &page.glyphs[i - 1]);
                let ok = g.shift < 0.0
                    && opts.punct_squeeze
                    && opening(g.ch)
                    && g.width >= -g.shift - EPS
                    && prev.is_some_and(|p| {
                        // 字面の左端(ずらす前の送り幅 width - shift の半分から)
                        let ink = g.x + g.shift + (g.width - g.shift) / 2.0;
                        opening(p.ch) && ink >= p.x + p.width - EPS
                    });
                if !ok {
                    return Err(format!("{here}: {:?} の字形のずれ {}", g.ch, g.shift));
                }
            }
        }
    }
    Ok(())
}

/// 行の中(最後の文字の後ろを除く)に改行できる位置があるか。
fn line_has_break(page: &Page, ln: usize, kinsoku: Kinsoku, room: f32) -> bool {
    let c = content(page, ln);
    (c.start..c.end.saturating_sub(1)).any(|i| oracle::can_break(page, i, kinsoku, room))
}

/// 両端揃えで広げられる間があるか(つながった語・ルビ・縦中横の中でない字の間)。
fn has_gap(page: &Page, c: std::ops::Range<usize>, kinsoku: Kinsoku) -> bool {
    (c.start..c.end.saturating_sub(1)).any(|i| {
        let g = &page.glyphs;
        let a_word = oracle::word_at(page, i);
        let b_word = oracle::word_at(page, i + 1);
        let in_ruby = page
            .rubies
            .iter()
            .any(|r| !r.emphasis && r.first_glyph <= i && i + 1 < r.first_glyph + r.glyph_count);
        let _ = kinsoku;
        let hyphen = g[i + 1].ch == '-' && is_latin(g[i].ch);
        !(a_word && b_word || hyphen) && !in_ruby && !oracle::same_tcy(&g[i], &g[i + 1])
    })
}

fn line_text(page: &Page, ln: usize) -> String {
    let l = &page.lines[ln];
    page.glyphs[l.first_glyph..l.first_glyph + l.glyph_count]
        .iter()
        .map(|g| g.ch)
        .collect()
}

/// 文字送りの時刻: 句読点の間は、続いた句読点・閉じ括弧の最後の後に、最も長い間を1度だけ置き、
/// 合図・段落の区切りは越えない。
fn check_timing(layout: &Layout) -> Result<(), String> {
    let opts = RevealOptions::default()
        .chars_per_sec(CPS)
        .comma_pause(COMMA)
        .period_pause(PERIOD);
    let reveal = Reveal::new(layout, opts);
    for (pi, page) in layout.pages.iter().enumerate() {
        let times = reveal.appear_times(pi);
        let steps = reveal.timeline(pi);
        if times.windows(2).any(|w| w[1] < w[0] - EPS) {
            return Err(format!("ページ {pi}: 文字の出る時刻が戻る"));
        }
        let g = &page.glyphs;
        let cue_at = |k: usize| page.cues.iter().any(|(at, _)| *at == k);
        let para_at = |k: usize| {
            page.lines
                .iter()
                .any(|l| l.paragraph_start && l.first_glyph == k)
        };
        let mut held = 0.0_f32;
        for i in 0..g.len() {
            let next = g.get(i + 1).map(|n| n.ch);
            held = held.max(oracle::pause(g[i].ch, next));
            let boundary = cue_at(i + 1) || para_at(i + 1);
            let ends = boundary || !next.is_some_and(oracle::trails);
            let expected = if ends { held } else { 0.0 };
            if ends {
                held = 0.0;
            }
            // 次に起きること(表示・合図)までの時間
            let pos = steps
                .iter()
                .position(|s| matches!(s.kind, StepKind::Show(k) if k == i))
                .ok_or("表示の時刻が無い")?;
            let Some(after) = steps[pos + 1..]
                .iter()
                .find(|s| !matches!(s.kind, StepKind::Emit(nekomoji::TextEvent::Blip(_))))
            else {
                continue;
            };
            let speed = layout.styles.get(g[i].style).map_or(1.0, |s| s.speed);
            let gap = after.at - times[i];
            let base = 1.0 / (CPS * speed);
            let ok = if cue_at(i + 1) {
                gap >= base + expected - EPS
            } else {
                (gap - base - expected).abs() < EPS
            };
            if !ok {
                return Err(format!(
                    "ページ {pi} の {i} 文字目 {:?} の後の間が {gap}(期待 {})",
                    g[i].ch,
                    base + expected
                ));
            }
        }
    }
    Ok(())
}

/// 保存と復元: どの時点で保存しても、戻してから同じだけ進めた見た目は元と同じ。
fn check_restore(layout: &Layout, rng: &mut Rng) -> Result<(), String> {
    let mut reveal = Reveal::new(layout, RevealOptions::default());
    for step in 0..40 {
        match rng.below(8) {
            0 => {
                reveal.skip();
            }
            1 | 2 => {
                reveal.proceed();
            }
            _ => {
                reveal.advance(rng.unit() * 0.6);
            }
        }
        let text = reveal.state().to_string();
        let state: RevealState = text.parse().map_err(|e| format!("{e}"))?;
        let mut other = Reveal::new(layout, RevealOptions::default());
        if !other.restore(state) {
            return Err(format!("{step}: 戻せない {text}"));
        }
        let (mut a, mut b) = (reveal.clone(), other);
        let dt = 0.05 + rng.unit();
        a.advance(dt);
        b.advance(dt);
        let look = |r: &Reveal| {
            (
                r.page(),
                r.visible_count(),
                r.wait_kind(),
                r.glyphs(layout)
                    .iter()
                    .map(|g| (g.ch, (g.alpha * 100.0).round() as i32))
                    .collect::<Vec<_>>(),
            )
        };
        if look(&a) != look(&b) {
            return Err(format!(
                "{step}: {text} から戻して {dt} 進めた見た目が違う\n元 {:?}\n戻 {:?}",
                look(&a),
                look(&b)
            ));
        }
        if reveal.is_finished() {
            break;
        }
    }
    Ok(())
}

fn run_case(seed: u64) {
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let opts = random_options(&mut rng);
    let mut src = String::new();
    random_markup(&mut rng, 0, opts.writing == Writing::Vertical, &mut src);
    let Ok(script) = parse(&src) else {
        panic!("シード {seed}: 生成した入力が解析できない {src:?}");
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let result = check_layout(&layout, &opts)
        .and_then(|_| check_timing(&layout))
        .and_then(|_| check_restore(&layout, &mut rng));
    if let Err(e) = result {
        panic!("シード {seed} で失敗: {e}\n入力: {src:?}\n設定: {opts:?}");
    }
}

#[test]
fn 生成した入力で組版と文字送りの不変条件が成り立つ() {
    let cases: u64 = std::env::var("NEKOMOJI_INVARIANT_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);
    for seed in 0..cases {
        run_case(seed);
    }
}
