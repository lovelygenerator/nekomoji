//! 頑健さのテスト: でたらめなマークアップと設定で、解析 → レイアウト → 文字送り → 猫モードを
//! 通し、パニックしないことと、出力の基本の性質が崩れないことを確かめる。
//!
//! 乱数はシード付きの xorshift(外部依存なし)。失敗したときはシードと入力が表示されるので、
//! そのまま単体テストにできる。回数は `NEKOMOJI_ROBUST_CASES` で増やせる(既定 5000)。
//! より深く探すときは `fuzz/`(cargo-fuzz)を使う。

use nekomoji::{
    chars::graphemes, parse, parse_all, Align, Blip, CellMetrics, Indent, Kinsoku, Layout,
    LayoutOptions, Motion, Neko, NekoMode, Reveal, RevealOptions, RevealState, TextEvent, Vars,
    Writing,
};

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

/// 文字の断片。禁則に関わる文字・幅の違う文字・単独タグ・変数・エスケープを混ぜる。
const ATOMS: &[&str] = &[
    "あ",
    "漢",
    "ア",
    "ｱ",
    "a",
    "Z",
    "1",
    "１",
    "hello",
    "it's",
    " ",
    "　",
    "、",
    "。",
    "！",
    "？",
    "「",
    "」",
    "『",
    "』",
    "（",
    "）",
    "っ",
    "ャ",
    "ー",
    "…",
    "――",
    "〜",
    "🐈",
    "e\u{301}",
    "辻\u{E0100}",
    "葛\u{FE00}",
    "か\u{3099}",
    "\u{3099}",
    "\u{FE0F}",
    "\u{200D}",
    "👨\u{200D}👩\u{200D}👧",
    "👍\u{1F3FD}",
    "\u{1F1EF}\u{1F1F5}",
    "\u{1F1EF}",
    "1\u{FE0F}\u{20E3}",
    "[tcy]1\u{FE0F}\u{20E3}[/tcy]",
    "[ruby つじ]辻\u{E0100}[/ruby]",
    "\t",
    "\n",
    "\n\n",
    "[p]",
    "[l]",
    "[l][l]",
    "[icon a]",
    "[icon btn_b 2.5]",
    "abcdefghijklmnopqrstuvwxyz0123456789",
    "[br]",
    "[wait 0.1]",
    "[wait 0]",
    "[voice v1]",
    "[se s]",
    "[event e a b]",
    "[var name]",
    "[var missing]",
    "[var]",
    "[var a b]",
    "[ruby よ][var x][/ruby]",
    "[tcy][var x][/tcy]",
    "[[",
    "]]",
    "{",
    "}",
    "{name}",
    "[ruby めいきゅう]迷宮[/ruby]",
    "｜迷宮《めいきゅう》",
    "|親《よみ》|",
    "||《》|[b]|a《b》",
    "[ruby ちょうちょう]蝶[/ruby]",
    "[ruby a]bc[/ruby]",
];

/// 範囲タグ(開きと閉じ)。
const RANGES: &[(&str, &str)] = &[
    ("[color red]", "[/color]"),
    ("[color #ff00ff]", "[/color]"),
    ("[size 2]", "[/size]"),
    ("[size 0.5]", "[/size]"),
    ("[b]", "[/b]"),
    ("[speed 3]", "[/speed]"),
    ("[speed 0.1]", "[/speed]"),
    ("[shake]", "[/shake]"),
    ("[wave 2]", "[/wave]"),
    ("[shake 1 0.8]", "[/shake]"),
    ("[glow]", "[/glow]"),
    ("[glow flicker 2]", "[/glow]"),
    ("[glow breathe 1 0.5]", "[/glow]"),
    ("[outline]", "[/outline]"),
    ("[outline #ffffff 0.12]", "[/outline]"),
    ("[shadow]", "[/shadow]"),
    ("[shadow gray -0.05 0.1]", "[/shadow]"),
    ("[wave 2 1]", "[/wave]"),
    ("[fade 1]", "[/fade]"),
    ("[neko hop]", "[/neko]"),
    ("[neko hop 2]", "[/neko]"),
    ("[sup]", "[/sup]"),
    ("[sub]", "[/sub]"),
    ("[s]", "[/s]"),
    ("[u]", "[/u]"),
    ("[em]", "[/em]"),
    ("[em ●]", "[/em]"),
    ("[mark a]", "[/mark]"),
    ("[mark 用語_1]", "[/mark]"),
    ("\n[align center]", "[/align]\n"),
    ("[br][align end]", "[/align][br]"),
    ("[blip]", "[/blip]"),
    ("[blip mike]", "[/blip]"),
    ("[blip off]", "[/blip]"),
];

/// 壊れた断片(エラーの経路を通す)。
const BROKEN: &[&str] = &[
    "[",
    "]",
    "{",
    "}",
    "[unknown]",
    "[/p]",
    "[size]",
    "[size -1]",
    "[speed 0]",
    "[ruby]",
    "[neko jump]",
    "[color]",
    "[/b]",
    "[ruby よみ]",
    "[mark]",
    "[mark a b]",
    "[b]",
    "[align]",
    "[align middle]",
    "あ[align center]",
    "[blip a b]",
];

fn random_markup(rng: &mut Rng, depth: usize, out: &mut String) {
    for _ in 0..rng.below(8) {
        match rng.below(20) {
            0 => out.push_str(BROKEN[rng.below(BROKEN.len())]),
            1..=4 if depth < 4 => {
                let (open, close) = RANGES[rng.below(RANGES.len())];
                out.push_str(open);
                random_markup(rng, depth + 1, out);
                out.push_str(close);
            }
            _ => out.push_str(ATOMS[rng.below(ATOMS.len())]),
        }
    }
}

fn random_source(rng: &mut Rng) -> String {
    let mut out = String::new();
    random_markup(rng, 0, &mut out);
    out
}

fn random_options(rng: &mut Rng) -> LayoutOptions {
    LayoutOptions::default()
        .writing(if rng.below(2) == 0 {
            Writing::Horizontal
        } else {
            Writing::Vertical
        })
        .max_width([0.1, 0.5, 1.0, 2.0, 3.5, 8.0, 40.0][rng.below(7)])
        .kinsoku(if rng.below(4) == 0 {
            Kinsoku::Off
        } else {
            Kinsoku::Standard
        })
        .hanging(rng.below(2) == 0)
        .max_lines([None, Some(1), Some(2), Some(5)][rng.below(4)])
        .punct_squeeze(rng.below(2) == 0)
        .latin_space([0.0, 0.25, 1.0][rng.below(3)])
        .justify(rng.below(2) == 0)
        .indent(
            Indent::default()
                .first([0.0, 1.0, 50.0][rng.below(3)])
                .rest([0.0, 0.5, -1.0][rng.below(3)])
                .bracket_first([0.0, 0.5, 1.0][rng.below(3)])
                .bracket_rest([0.0, 1.0, 3.0][rng.below(3)]),
        )
        .align([Align::Start, Align::Center, Align::End][rng.below(3)])
}

fn check_layout(layout: &Layout, src: &str) {
    assert!(!layout.pages.is_empty(), "ページは1つ以上");
    for page in &layout.pages {
        let mut next = 0;
        for (ln, line) in page.lines.iter().enumerate() {
            assert_eq!(line.first_glyph, next, "行のグリフは連続する");
            assert!(line.indent >= 0.0, "字下げは 0 以上");
            assert!(
                line.width >= line.indent - 1e-4,
                "行の幅は字下げ・揃えのアキを含む"
            );
            if let Some(g) = page
                .glyphs
                .get(line.first_glyph)
                .filter(|_| line.glyph_count > 0)
            {
                assert!(g.x >= line.indent - 1e-4, "最初の文字は字下げより後ろ");
            }
            next += line.glyph_count;
            for g in &page.glyphs[line.first_glyph..next] {
                assert_eq!(g.line, ln);
                assert!(g.x.is_finite() && g.width.is_finite() && g.width >= 0.0);
                assert!(g.style < layout.styles.len().max(1));
                assert_eq!(
                    graphemes(&g.text()).len(),
                    1,
                    "グリフは書記素クラスタ1つ: {:?}",
                    g.text()
                );
                // 元の文章の範囲は文字の境界にあり、変数でなければその文字を指す
                let orig = src.get(g.source.clone()).expect("元の範囲は文字の境界");
                assert!(!orig.is_empty(), "元の範囲は空でない");
                assert!(g.var.map_or(true, |v| v < layout.vars.len()));
                // (書記素クラスタの途中に空の範囲タグがあると、その分も範囲に入る)
                if g.var.is_none() && !orig.starts_with(['[', ']', '{', '}']) {
                    assert!(orig.starts_with(g.ch), "元の範囲はその文字から始まる");
                }
            }
        }
        assert_eq!(next, page.glyphs.len(), "すべてのグリフがどこかの行にある");
        // 当たり判定: 幅のある字の枠の中央はそのグリフ(縦中横は塊の最初)に当たる
        for (i, g) in page.glyphs.iter().enumerate() {
            if g.width > 0.0 {
                let hit = page.hit_test(g.x + g.width / 2.0, g.line as f32 + 0.5);
                let hit = hit.expect("字の枠の中は当たる");
                assert!(hit <= i && page.glyphs[hit].line == g.line);
            }
        }
        let extent = page.extent();
        assert_eq!(extent.lines, page.lines.len());
        let page_no = layout
            .pages
            .iter()
            .position(|p| std::ptr::eq(p, page))
            .unwrap();
        for m in layout.marks(page_no) {
            assert!(m.glyph_count > 0 && m.first_glyph + m.glyph_count <= page.glyphs.len());
            let spans = page.spans(m.first_glyph, m.glyph_count);
            let total: usize = spans.iter().map(|s| s.glyph_count).sum();
            assert_eq!(total, m.glyph_count, "矩形は範囲の文字をすべて含む");
        }
        for r in &page.rubies {
            assert!(r.first_glyph + r.glyph_count <= page.glyphs.len());
            assert!(r.line < page.lines.len());
        }
        for d in &page.decorations {
            assert!(d.first_glyph + d.glyph_count <= page.glyphs.len());
            assert!(d.x0 <= d.x1);
        }
        let mut prev = 0;
        for (at, _) in &page.cues {
            assert!(*at >= prev && *at <= page.glyphs.len(), "合図は順に並ぶ");
            prev = *at;
        }
        let _ = page.text();
        let _ = page.speech_text();
    }
}

/// `parse_all` は `parse` と同じ入力を通し、`parse` のエラーを含み、位置の順に並べる。
fn check_parse_all(src: &str) {
    match (parse(src), parse_all(src)) {
        (Ok(a), Ok(b)) => assert_eq!(a, b, "parse と parse_all の結果が同じ"),
        (Err(first), Err(all)) => {
            assert!(
                all.contains(&first),
                "parse のエラー {first} が parse_all にある"
            );
            assert!(
                all.windows(2)
                    .all(|w| (w[0].line, w[0].col) <= (w[1].line, w[1].col)),
                "位置の順"
            );
            let lines = src.lines().count().max(1);
            for e in &all {
                assert!(e.line >= 1 && e.line <= lines && e.col >= 1, "位置 {e}");
            }
        }
        (a, b) => panic!(
            "parse と parse_all で成否が違う: {:?} / {:?}",
            a.err(),
            b.err()
        ),
    }
}

fn run_case(seed: u64) {
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let src = random_source(&mut rng);
    let opts = random_options(&mut rng);
    let result = std::panic::catch_unwind(|| {
        check_parse_all(&src);
        let Ok(script) = parse(&src) else {
            return;
        };
        let mut vars = Vars::new();
        vars.insert("name".into(), "ミケ[b]".into());
        let layout = Layout::build(&script, &CellMetrics, &opts, &vars);
        check_layout(&layout, &src);
        let neko = Neko::new(&layout, NekoMode::default().seed(seed).knock_chance(0.5));
        let blip = (seed % 2 == 0).then(|| Blip::default().every(1 + (seed % 3) as usize));
        let mut reveal = Reveal::new(&layout, RevealOptions::default().blip(blip));
        let mut rng = Rng(seed | 1);
        for _ in 0..60 {
            match rng.below(10) {
                0 => {
                    let ev = reveal.skip();
                    assert!(
                        !ev.iter().any(|e| matches!(e, TextEvent::Blip(_))),
                        "skip では鳴らさない"
                    );
                }
                1 => {
                    reveal.proceed();
                }
                2 => {
                    // ページの始めで一度も進めていない状態は、戻すと advance(0) 後と同じになる
                    reveal.advance(0.0);
                    let saved = reveal.state();
                    let text: RevealState = saved.to_string().parse().expect("読み戻せる");
                    let mut other = Reveal::new(&layout, RevealOptions::default());
                    assert!(other.restore(text));
                    assert_eq!(other.visible_count(), reveal.visible_count());
                    assert_eq!(other.is_waiting(), reveal.is_waiting());
                    assert_eq!(other.wait_kind(), reveal.wait_kind());
                }
                _ => {
                    let ev = reveal.advance(rng.unit() * 0.5);
                    let n = ev
                        .iter()
                        .filter(|e| matches!(e, TextEvent::Blip(_)))
                        .count();
                    assert!(n <= 1, "1回の advance で音は1つまで");
                }
            }
            let page = &layout.pages[reveal.page()];
            assert!(reveal.visible_count() <= page.glyphs.len());
            let wait = reveal.auto_wait(reveal.page(), |id: &str| Some(id.len() as f32 * 0.3));
            assert!(wait.is_finite() && wait >= 0.0, "オートの待ち時間は 0 以上");
            if reveal.auto_due(|_: &str| None) {
                assert!(reveal.is_waiting());
            }
            assert_eq!(
                reveal.cursor(&layout).is_some(),
                reveal.wait_kind().is_some()
            );
            let times = reveal.appear_times(reveal.page());
            for (i, t) in times.iter().enumerate().take(reveal.visible_count()) {
                assert!(
                    *t <= reveal.time() + 1e-3,
                    "表示済みの文字は今より前に出ている {i}"
                );
            }
            for g in reveal.glyphs(&layout) {
                assert!(g.alpha.is_finite() && g.offset.0.is_finite() && g.offset.1.is_finite());
            }
            // 動きなしでは文字は動かず、猫のちょっかいも出ない
            let mut still = reveal.clone();
            still.set_motion(Motion::Off);
            for g in still.glyphs(&layout) {
                assert_eq!((g.offset, g.rotation), ((0.0, 0.0), 0.0), "{g:?}");
            }
            assert!(neko.knocked(&layout, &still).is_empty());
            let _ = reveal.decorations(&layout);
            if let Some(cat) = neko.cat(&layout, &reveal) {
                assert!(cat.x.is_finite(), "{cat:?}");
            }
            let _ = neko.glyphs(&layout, &reveal);
            if reveal.is_finished() {
                break;
            }
        }
    });
    if let Err(e) = result {
        panic!("シード {seed} で失敗\n入力: {src:?}\n設定: {opts:?}\n{e:?}");
    }
}

#[test]
fn 生成した入力の多くは解析を通る() {
    let mut ok = 0;
    for seed in 0..1000u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        if parse(&random_source(&mut rng)).is_ok() {
            ok += 1;
        }
    }
    assert!(ok > 500, "解析を通ったのは {ok}/1000");
}

#[test]
fn でたらめな入力と設定でもパニックせず出力の性質が崩れない() {
    let cases: u64 = std::env::var("NEKOMOJI_ROBUST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5000);
    for seed in 0..cases {
        run_case(seed);
    }
}
