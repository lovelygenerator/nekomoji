//! 仕様の例のテスト: docs/markup.md に書いた例が、書いたとおりに動くことを確かめる。
//!
//! テストの名前は markup.md の節と規則の名前に合わせる。markup.md に規則や例を足したら、
//! ここにも同じ例を足す(仕様と実装のずれを見つけるため)。

use nekomoji::{
    parse, CellMetrics, ErrorKind, Indent, Item, Kinsoku, Layout, LayoutOptions, Reveal,
    RevealOptions, Vars,
};

fn build(src: &str, opts: &LayoutOptions) -> Layout {
    Layout::build(&parse(src).unwrap(), &CellMetrics, opts, &Vars::new())
}

/// 横書き・幅 `w`・和欧間のアキなしで組んだ行の文字列(空白も含む)。
fn lines(src: &str, w: f32) -> Vec<String> {
    let opts = LayoutOptions::default().max_width(w).latin_space(0.0);
    let layout = build(src, &opts);
    let page = &layout.pages[0];
    page.lines
        .iter()
        .map(|l| {
            page.glyphs[l.first_glyph..l.first_glyph + l.glyph_count]
                .iter()
                .map(|g| g.ch)
                .collect()
        })
        .collect()
}

/// 文字送りで、`k` 文字目の後から次の文字までの秒数(20 文字/秒、間は既定)。
fn gap_after(src: &str, k: usize) -> f32 {
    let layout = build(src, &LayoutOptions::default());
    let t = Reveal::new(&layout, RevealOptions::default()).appear_times(0);
    t[k + 1] - t[k]
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

// ---- 基本 ----

#[test]
fn 基本_文字の角括弧は二重にし_二重にしていない閉じ括弧はエラー_波括弧は普通の文字() {
    assert_eq!(lines("[[a]]{b}", 20.0), ["[a]{b}"]);
    assert_eq!(lines("[ruby よみ]{a}[[b]][/ruby]", 20.0), ["{a}[b]"]);
    assert_eq!(lines("{name}{{x}}", 20.0), ["{name}{{x}}"]);
    assert!(matches!(
        parse("a]").unwrap_err().kind,
        ErrorKind::LoneClose(']')
    ));
    assert!(parse("a}").is_ok());
}

#[test]
fn ルビの短い書き方は_ruby_タグと同じ配置になる() {
    let long = parse("[ruby めいきゅう]迷宮[/ruby]").unwrap();
    for src in ["｜迷宮《めいきゅう》", "|迷宮《めいきゅう》"] {
        let short = parse(src).unwrap();
        let (
            Item::Ruby {
                base: b1,
                reading: r1,
                ..
            },
            Item::Ruby {
                base: b2,
                reading: r2,
                ..
            },
        ) = (&short.items[0], &long.items[0])
        else {
            panic!("{src}")
        };
        assert_eq!((b1, r1), (b2, r2), "{src}");
    }
    // 合わない縦線は文字のまま
    assert_eq!(lines("a|b|親《》", 20.0), ["a|b|親《》"]);
    assert_eq!(lines("||親《よみ》", 20.0), ["|親"]);
}

#[test]
fn 基本_引数の誤り_多すぎる引数を含む_は行と桁つきのエラー() {
    let e = parse("あ\n[color red blue]い[/color]").unwrap_err();
    assert_eq!((e.line, e.col), (2, 1));
    assert!(matches!(e.kind, ErrorKind::BadArg { .. }));
}

// ---- 単独タグ ----

#[test]
fn 単独タグ_p_の直後の改行と直前の改行1つは無視する() {
    let layout = build("文。\n[p]\n次。", &LayoutOptions::default());
    assert_eq!(layout.pages.len(), 2);
    assert_eq!(layout.pages[0].lines.len(), 1);
    assert_eq!(layout.pages[1].lines.len(), 1);
}

// ---- 変数 ----

#[test]
fn 変数_値はマークアップとして読まず_改行は改行にする() {
    let script = parse("[var n]さん").unwrap();
    let mut vars = Vars::new();
    vars.insert("n".into(), "[b]A\nB".into());
    let layout = Layout::build(&script, &CellMetrics, &LayoutOptions::default(), &vars);
    assert_eq!(layout.pages[0].text(), "[b]A\nBさん");
}

// ---- 組版の規則 ----

#[test]
fn 組版の規則_禁則_行頭禁則と行末禁則は前の文字ごと追い出す() {
    let opts = LayoutOptions::default().max_width(3.0).hanging(false);
    let layout = build("あいう。え", &opts);
    let first = &layout.pages[0].lines[1];
    assert_eq!(
        layout.pages[0].glyphs[first.first_glyph].ch, 'う',
        "。を行頭に置かない"
    );
    let layout = build("あい「う", &LayoutOptions::default().max_width(3.0));
    let second = &layout.pages[0].lines[1];
    assert_eq!(
        layout.pages[0].glyphs[second.first_glyph].ch, '「',
        "「を行末に置かない"
    );
}

#[test]
fn 組版の規則_ぶら下げ_行末の句読点は幅を超えても行に残す() {
    assert_eq!(lines("あいう。え", 3.0), ["あいう。", "え"]);
}

#[test]
fn 組版の規則_英数字の連続は1語として途中で分けない() {
    assert_eq!(lines("ああhello", 4.0), ["ああ", "hello"]);
    assert_eq!(lines("hel[b]lo[/b]ああ", 2.5), ["hello", "ああ"]);
}

#[test]
fn 組版の規則_どの行にも収まらない語だけは文字の間で折り返す() {
    assert_eq!(lines("あabcdefgh", 3.0), ["あabcd", "efgh"]);
}

#[test]
fn 組版の規則_数の中の区切りと数の前後の記号は数と1語にする() {
    for src in [
        "3.14",
        "1,000",
        "10:30",
        "2026/11/10",
        "2026-11-10",
        "$100",
        "US$100",
        "100%",
    ] {
        let w = src.chars().count() as f32 * 0.5;
        assert_eq!(lines(&format!("あ{src}"), w), ["あ", src], "{src}");
    }
}

#[test]
fn 組版の規則_アクセント付きのラテン文字も英字として扱う() {
    assert_eq!(lines("ああcafé", 3.5), ["ああ", "café"]);
}

#[test]
fn 組版の規則_折り返した行の後ろの半角空白は行末に残す() {
    assert_eq!(lines("hello world", 3.0), ["hello ", "world"]);
}

#[test]
fn 組版の規則_語の後ろのハイフンは前の語に付けハイフンの後ろでだけ折り返す() {
    assert_eq!(lines("well-known", 3.0), ["well-", "known"]);
}

#[test]
fn 組版の規則_禁則を切っていてもスタイルの境目で分かれた語は分けない() {
    let opts = LayoutOptions::default()
        .max_width(3.0)
        .latin_space(0.0)
        .kinsoku(Kinsoku::Off);
    let layout = build("あhel[b]lo[/b]", &opts);
    assert_eq!(layout.pages[0].lines.len(), 2);
}

#[test]
fn 組版の規則_和欧間のアキは漢字仮名と欧文の間に入れる() {
    let layout = build("あa", &LayoutOptions::default());
    assert!(close(layout.pages[0].glyphs[1].x, 1.25));
    let layout = build("あé", &LayoutOptions::default());
    assert!(close(layout.pages[0].glyphs[1].x, 1.25));
}

#[test]
fn 組版の規則_字下げ_開き直した会話の後の地の文は1字下げる() {
    let opts = LayoutOptions::default().indent(Indent::novel());
    let layout = build("「一\n「二」\n地の文。", &opts);
    let indents: Vec<f32> = layout.pages[0].lines.iter().map(|l| l.indent).collect();
    assert_eq!(indents, [0.0, 0.0, 1.0]);
}

#[test]
fn 組版の規則_両端揃えはスタイルの境目で分かれた語の中を広げない() {
    let opts = LayoutOptions::default()
        .max_width(6.0)
        .latin_space(0.0)
        .justify(true);
    let layout = build("ab[b]cd[/b]ああああ", &opts);
    let g = &layout.pages[0].glyphs;
    assert!(close(g[2].x - g[1].x, 0.5));
}

#[test]
fn 組版の規則_文字送り_句読点の後の間() {
    assert!(close(gap_after("あ、い", 1), 0.05 + 0.1), "、の後に 0.1 秒");
    assert!(close(gap_after("あ…い", 1), 0.05 + 0.1), "…の後に 0.1 秒");
    assert!(close(gap_after("あ。い", 1), 0.05 + 0.3), "。の後に 0.3 秒");
}

#[test]
fn 組版の規則_文字送り_続いた句読点と閉じ括弧の最後に最も長い間を1度だけ置く() {
    assert!(
        close(gap_after("「あ。」い", 2), 0.05),
        "。と」の間は空けない"
    );
    assert!(close(gap_after("「あ。」い", 3), 0.05 + 0.3));
    assert!(
        close(gap_after("あ……。い", 3), 0.05 + 0.3),
        "……。は長いほう"
    );
    assert!(
        close(gap_after("1,000", 1), 0.05),
        "数の中の , の後には置かない"
    );
}

#[test]
fn 組版の規則_分類の表_行頭に置けない文字() {
    // 波ダッシュ・半角の小書きの仮名・コロン・スラッシュ・ハイフン・後置の記号は行頭に来ない
    for c in ['〜', 'ｧ', '｡', ':', '/', '-', '%', '℃', '〟', '»'] {
        let src = format!("ああ{c}");
        let opts = LayoutOptions::default().max_width(2.0).hanging(false);
        let layout = build(&src, &opts);
        let page = &layout.pages[0];
        let heads: Vec<char> = page
            .lines
            .iter()
            .map(|l| page.glyphs[l.first_glyph].ch)
            .collect();
        assert!(!heads[1..].contains(&c), "{c}: {heads:?}");
    }
}

#[test]
fn 組版の規則_分類の表_行末に置けない文字() {
    for c in ['「', '«', '〝', '₩', '￥', '№'] {
        let src = format!("ああ{c}い");
        let layout = build(&src, &LayoutOptions::default().max_width(3.0));
        let page = &layout.pages[0];
        let l0 = &page.lines[0];
        let last = page.glyphs[l0.first_glyph + l0.glyph_count - 1].ch;
        assert_ne!(last, c, "{c}");
    }
}

#[test]
fn 組版の規則_分類の表_前置と後置の記号は数と1語にする() {
    assert_eq!(lines("あ₩1000", 3.0), ["あ", "₩1000"]);
    assert_eq!(lines("あ30℃", 2.0), ["あ", "30℃"]);
}

// ---- 基本: タグの書き方・数・色・名前と ID ----

#[test]
fn 基本_タグの書き方() {
    // 名前は小文字で、大文字小文字は区別する
    assert!(matches!(
        parse("[B]あ[/B]").unwrap_err().kind,
        ErrorKind::UnknownTag(_)
    ));
    // `[` の直後(閉じタグでは `/` の直後)に空白は書けない
    assert_eq!(
        parse("[ b]あ[/b]").unwrap_err().kind,
        ErrorKind::SpaceAfterBracket
    );
    // 引数の区切りの空白は1つ以上
    assert!(parse("[glow  breathe   2]あ[/glow]").is_ok());
    // タグは1行の中に書く
    assert_eq!(
        parse("[color\nred]あ[/color]").unwrap_err().kind,
        ErrorKind::UnterminatedTag
    );
}

#[test]
fn 基本_数の引数と範囲() {
    assert!(parse("[wait 1e0]").is_ok());
    for src in [
        "[wait nan]",
        "[wait inf]",
        "[wait １]",
        "[speed 100.5]x[/speed]",
    ] {
        assert!(parse(src).is_err(), "{src}");
    }
    // 強さは 0 より大きい
    assert!(parse("[shake 0]あ[/shake]").is_err());
    assert!(parse("[fade 0]あ[/fade]").is_ok());
}

#[test]
fn 基本_色と名前() {
    for c in ["red", "Silver", "#f00", "#FF00ff"] {
        assert!(parse(&format!("[color {c}]あ[/color]")).is_ok(), "{c}");
    }
    assert!(parse("[color #ff00]あ[/color]").is_err());
    assert!(parse("[color なぞ]あ[/color]").is_err());
    assert!(parse(&format!("[mark {}]あ[/mark]", "a".repeat(64))).is_ok());
    assert!(parse(&format!("[mark {}]あ[/mark]", "a".repeat(65))).is_err());
}

#[test]
fn 基本_同じ種類の入れ子は内側が優先で_align_だけ入れ子にできない() {
    let s = parse("[color red][color blue]あ[/color][/color]").unwrap();
    let Item::Text { style, .. } = &s.items[0] else {
        panic!()
    };
    assert_eq!(s.styles[*style].color.as_deref(), Some("blue"));
    let s = parse("[size 2][size 3]あ[/size][/size]").unwrap();
    let Item::Text { style, .. } = &s.items[0] else {
        panic!()
    };
    assert_eq!(s.styles[*style].size, 3.0);
    assert!(parse("[align center][align end]あ[/align][/align]").is_err());
}

#[test]
fn ルビ_読みは一語で_バックログは読み戻せる() {
    assert!(parse("[ruby よ み]親[/ruby]").is_err());
    let long = "あ".repeat(33);
    let layout = build(
        &format!("[ruby よみ]{long}[/ruby]"),
        &LayoutOptions::default(),
    );
    let text = layout.pages[0].text();
    assert_eq!(text, format!("[ruby よみ]{long}[/ruby]"));
    assert_eq!(
        build(&text, &LayoutOptions::default()).pages[0].text(),
        text
    );
}

#[test]
fn 揃え_段落の頭の判定は間のタグを数えない() {
    assert!(parse("あ[br][wait 1][voice v][align center]い[/align]").is_ok());
    assert!(parse("[color red][align center]い[/align][/color]").is_ok());
    assert!(parse("あ[align center]い[/align]").is_err());
}
