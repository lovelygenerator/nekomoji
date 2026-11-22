use super::*;
use crate::markup::{Align, TextEvent};
use crate::text::metrics::LineKind;
use crate::{parse, CellMetrics};

fn lines(src: &str, width: f32, kinsoku: Kinsoku) -> Vec<String> {
    let script = parse(src).unwrap();
    let opts = LayoutOptions {
        max_width: width,
        kinsoku,
        ..LayoutOptions::default()
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let page = &layout.pages[0];
    page.lines
        .iter()
        .map(|l| {
            page.glyphs[l.first_glyph..l.first_glyph + l.glyph_count]
                .iter()
                .map(Glyph::text)
                .collect()
        })
        .collect()
}

fn build(src: &str, width: f32) -> Layout {
    let opts = LayoutOptions::default().max_width(width);
    Layout::build(&parse(src).unwrap(), &CellMetrics, &opts, &Vars::new())
}

#[test]
fn 異体字セレクタ付きの文字は1つのグリフになる() {
    let layout = build("辻\u{E0100}が", 10.0);
    let g = &layout.pages[0].glyphs;
    assert_eq!(g.len(), 2);
    assert_eq!((g[0].ch, g[0].tail.as_str()), ('辻', "\u{E0100}"));
    assert_eq!(g[0].text(), "辻\u{E0100}");
    assert_eq!(g[0].width, 1.0);
    assert_eq!(g[1].x, 1.0);
}

#[test]
fn zwjの絵文字と国旗は1つのグリフになる() {
    let layout = build("👨\u{200D}👩\u{200D}👧と\u{1F1EF}\u{1F1F5}", 10.0);
    let texts: Vec<String> = layout.pages[0].glyphs.iter().map(Glyph::text).collect();
    assert_eq!(
        texts,
        ["👨\u{200D}👩\u{200D}👧", "と", "\u{1F1EF}\u{1F1F5}"]
    );
}

#[test]
fn 書記素クラスタの途中では改行しない() {
    assert_eq!(
        lines("あい辻\u{E0100}う", 3.0, Kinsoku::Off),
        ["あい辻\u{E0100}", "う"]
    );
    // 結合用の濁点の付いた「か」も途中で切らない
    assert_eq!(
        lines("あいか\u{3099}う", 3.0, Kinsoku::Standard),
        ["あいか\u{3099}", "う"]
    );
}

#[test]
fn どの行にも収まらない長い英単語だけは文字の間で折り返す() {
    // 6 文字(全角 3)の語は幅 4 の行に収まるので分けずに次の行へ送る
    assert_eq!(
        lines("あいうabcdef", 4.0, Kinsoku::Standard),
        ["あいう", "abcdef"]
    );
    // 12 文字(全角 6)の語は幅 4 に収まらないので、行を埋めて文字の間で折り返す
    // (「い」と「a」の間には和欧間のアキ 0.25 が入る)
    assert_eq!(
        lines("あいabcdefghijkl", 4.0, Kinsoku::Standard),
        ["あいabc", "defghijk", "l"]
    );
    // 字下げの分も引いた幅で決める
    let script = parse("abcdefgh").unwrap();
    let opts = LayoutOptions::default()
        .max_width(4.0)
        .indent(Indent::default().first(1.0));
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let widths: Vec<f32> = layout.pages[0].lines.iter().map(|l| l.width).collect();
    assert_eq!(widths, [4.0, 1.0]);
}

#[test]
fn アイコンは指定の長さの場所を空け_id_を持つ() {
    let layout = build("[icon btn_a]で[size 2][icon star 1.5][/size]", 10.0);
    let g = &layout.pages[0].glyphs;
    assert_eq!(g.len(), 3);
    assert_eq!((g[0].ch, g[0].icon.as_deref()), (ICON_CHAR, Some("btn_a")));
    assert_eq!((g[0].x, g[0].width), (0.0, 1.0));
    assert_eq!(g[1].icon, None);
    assert_eq!((g[2].x, g[2].width), (2.0, 3.0));
    assert_eq!(g[2].source, 23..38);
    assert_eq!(layout.pages[0].text(), "\u{FFFC}で\u{FFFC}");
    // 縦書きでも行の中の向きの長さ
    let opts = LayoutOptions::default().vertical();
    let v = Layout::build(
        &parse("[icon a 2]あ").unwrap(),
        &CellMetrics,
        &opts,
        &Vars::new(),
    );
    assert_eq!(v.pages[0].glyphs[1].x, 2.0);
}

#[test]
fn アイコンには圏点を付けない() {
    let layout = build("[em][icon a]あ[/em]", 10.0);
    let r = &layout.pages[0].rubies;
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].first_glyph, 1);
}

#[test]
fn 読み上げ用のテキストはルビを読みにし_折り返しはつなぎ_段落だけ改行する() {
    let layout = build(
        "[ruby めいきゅう]迷宮[/ruby]の[em]奥[/em]で[icon a]何かが動いた。\n「誰？」",
        4.0,
    );
    let page = &layout.pages[0];
    assert!(page.lines.len() > 2);
    assert_eq!(
        page.speech_text(),
        "めいきゅうの奥で何かが動いた。\n「誰？」"
    );
    let starts: Vec<bool> = page.lines.iter().map(|l| l.paragraph_start).collect();
    assert!(starts[0]);
    assert!(!starts[1]);
    assert!(*starts.last().unwrap());
}

#[test]
fn 自動改ページで続いた行は段落の1行目ではない() {
    let opts = LayoutOptions::default().max_width(2.0).max_lines(1);
    let layout = Layout::build(&parse("あいう").unwrap(), &CellMetrics, &opts, &Vars::new());
    assert_eq!(layout.pages.len(), 2);
    assert!(layout.pages[0].lines[0].paragraph_start);
    assert!(!layout.pages[1].lines[0].paragraph_start);
    assert_eq!(layout.pages[1].speech_text(), "う");
}

#[test]
fn 結合文字の付いた英字は単語の一部になる() {
    assert_eq!(
        lines("あe\u{301}tait", 3.0, Kinsoku::Standard),
        ["あ", "e\u{301}tait"]
    );
}

#[test]
fn 禁則は書記素クラスタの先頭の文字で判定する() {
    // 「。」に異体字セレクタが付いていても行頭禁則(ぶら下げ)
    assert_eq!(
        lines("あいう。\u{FE00}え", 3.0, Kinsoku::Standard),
        ["あいう。\u{FE00}", "え"]
    );
}

#[test]
fn ルビの親文字と縦中横も書記素クラスタで数える() {
    let layout = build(
        "[ruby つじ]辻\u{E0100}[/ruby][tcy]1\u{FE0F}\u{20E3}[/tcy]",
        10.0,
    );
    let page = &layout.pages[0];
    assert_eq!(page.glyphs.len(), 2);
    assert_eq!(page.rubies[0].glyph_count, 1);
    assert_eq!(page.text(), "｜辻\u{E0100}《つじ》1\u{FE0F}\u{20E3}");
}

#[test]
fn 幅で折り返す() {
    assert_eq!(
        lines("あいうえおかき", 3.0, Kinsoku::Off),
        ["あいう", "えおか", "き"]
    );
}

#[test]
fn 行頭禁則の文字は前の文字ごと追い出す() {
    // 「っ」が行頭に来るので、直前の「き」ごと次の行へ
    assert_eq!(
        lines("あいきっと", 3.0, Kinsoku::Standard),
        ["あい", "きっと"]
    );
    assert_eq!(lines("あいきっと", 3.0, Kinsoku::Off), ["あいき", "っと"]);
}

#[test]
fn 句読点はぶら下げる() {
    assert_eq!(
        lines("あいう。えお", 3.0, Kinsoku::Standard),
        ["あいう。", "えお"]
    );
}

#[test]
fn ぶら下げの後に閉じ括弧が続くなら追い出す() {
    assert_eq!(
        lines("「あい。」う", 3.0, Kinsoku::Standard),
        ["「あ", "い。」", "う"]
    );
}

#[test]
fn 開き括弧は行末に残さない() {
    assert_eq!(
        lines("あい「うえ」", 3.0, Kinsoku::Standard),
        ["あい", "「う", "え」"]
    );
}

#[test]
fn 三点リーダーの間で分けない() {
    // … は半角幅(0.5)として数える
    assert_eq!(lines("あい……う", 2.5, Kinsoku::Standard), ["あい", "……う"]);
}

#[test]
fn 英単語は途中で分けない() {
    assert_eq!(
        lines("あいhello", 4.0, Kinsoku::Standard),
        ["あい", "hello"]
    );
}

#[test]
fn ルビは親文字と一緒に動き_はみ出しの範囲なら親文字を広げない() {
    let script = parse("あ[ruby めいきゅう]迷宮[/ruby]").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    let page = &layout.pages[0];
    assert_eq!(page.rubies.len(), 1);
    let r = &page.rubies[0];
    // ルビ 5文字 × 0.5 = 2.5。はみ出し 0.5 まで許すので親文字 2.0 のまま
    assert_eq!(r.width, 2.0);
    assert_eq!((r.first_glyph, r.glyph_count), (1, 2));
    assert_eq!(page.glyphs[2].x, 2.0);
}

#[test]
fn はみ出しを超える長いルビは親文字を広げる() {
    let script = parse("[ruby ちょうちょう]蝶[/ruby]").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    let page = &layout.pages[0];
    // ルビ 6文字 × 0.5 = 3.0、はみ出し 0.5 を引いて 2.5
    assert_eq!(page.rubies[0].width, 2.5);
    assert_eq!(page.glyphs[0].x, 0.75);
}

#[test]
fn 改行と改ページ() {
    let script = parse("あ\nい[p]う").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    assert_eq!(layout.pages.len(), 2);
    assert_eq!(layout.pages[0].lines.len(), 2);
    assert_eq!(layout.pages[1].glyphs[0].ch, 'う');
}

#[test]
fn 末尾の改ページは空のページを作らない() {
    let script = parse("あ[p]").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    assert_eq!(layout.pages.len(), 1);
}

fn page_texts(src: &str, width: f32, max_lines: usize) -> Vec<String> {
    let script = parse(src).unwrap();
    let opts = LayoutOptions {
        max_width: width,
        max_lines: Some(max_lines),
        ..LayoutOptions::default()
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    layout.pages.iter().map(|p| p.text()).collect()
}

#[test]
fn 最大行数を超えると自動で改ページし_行の番号は0から数え直す() {
    assert_eq!(
        page_texts("あいうえおかきくけ", 2.0, 2),
        ["あい\nうえ", "おか\nきく", "け"]
    );
    let script = parse("あいうえお").unwrap();
    let opts = LayoutOptions {
        max_width: 2.0,
        max_lines: Some(2),
        ..LayoutOptions::default()
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    assert_eq!(layout.pages[1].glyphs[0].line, 0);
}

#[test]
fn 自動改ページは明示の改ページと組み合わさり_ちょうど収まれば空のページを作らない() {
    assert_eq!(page_texts("あいうえ[p]か", 2.0, 2), ["あい\nうえ", "か"]);
}

#[test]
fn 自動改ページの直後の空行は捨てる() {
    assert_eq!(page_texts("あ\nい\n\nう", 5.0, 2), ["あ\nい", "う"]);
}

#[test]
fn 自動改ページをまたぐ合図は次のページの文字の前に付く() {
    let script = parse("あい[voice v]う").unwrap();
    let opts = LayoutOptions {
        max_width: 2.0,
        max_lines: Some(1),
        ..LayoutOptions::default()
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    assert_eq!(layout.pages.len(), 2);
    assert_eq!(
        layout.pages[1].cues,
        vec![(0, Cue::Event(TextEvent::Voice("v".into())))]
    );
}

#[test]
fn 合図は直後の文字の前に付く() {
    let script = parse("あい[wait 1]う[voice v]").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    assert_eq!(
        layout.pages[0].cues,
        vec![
            (2, Cue::Wait(1.0)),
            (3, Cue::Event(TextEvent::Voice("v".into())))
        ]
    );
}

#[test]
fn 変数を展開し_ないものは記録する() {
    let script = parse("[var name]と[var x]").unwrap();
    let mut vars = Vars::new();
    vars.insert("name".into(), "ミケ".into());
    let layout = Layout::build(&script, &CellMetrics, &LayoutOptions::default(), &vars);
    let text: String = layout.pages[0].glyphs.iter().map(|g| g.ch).collect();
    assert_eq!(text, "ミケと[var x]");
    assert_eq!(layout.missing_vars, vec!["x".to_string()]);
}

fn vlines(src: &str, height: f32) -> Vec<Vec<(char, f32)>> {
    let script = parse(src).unwrap();
    let opts = LayoutOptions {
        writing: Writing::Vertical,
        max_width: height,
        latin_space: 0.0,
        ..LayoutOptions::default()
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    assert_eq!(layout.writing, Writing::Vertical);
    let page = &layout.pages[0];
    page.lines
        .iter()
        .map(|l| {
            page.glyphs[l.first_glyph..l.first_glyph + l.glyph_count]
                .iter()
                .map(|g| (g.ch, g.x))
                .collect()
        })
        .collect()
}

#[test]
fn 縦書きでは仮名も句読点も全角の送りで並び_禁則とぶら下げは横書きと同じ() {
    assert_eq!(
        vlines("あいう。えお", 3.0),
        vec![
            vec![('あ', 0.0), ('い', 1.0), ('う', 2.0), ('。', 3.0)],
            vec![('え', 0.0), ('お', 1.0)],
        ]
    );
    // 「っ」は行頭に置かない
    let l = vlines("あいきっと", 3.0);
    assert_eq!(l[1][0].0, 'き');
}

#[test]
fn 縦書きでは半角英字を寝かせて横の送り幅で数える() {
    // a(0.5) + b(0.5) + あ(1.0) + い(1.0) = 3.0 で1行に収まる
    assert_eq!(
        vlines("abあい", 3.0),
        vec![vec![('a', 0.0), ('b', 0.5), ('あ', 1.0), ('い', 2.0)]]
    );
}

#[test]
fn 縦書きのルビも全角の送りで長さを測る() {
    let script = parse("[ruby ねーむ]名[/ruby]").unwrap();
    let opts = LayoutOptions {
        writing: Writing::Vertical,
        ..LayoutOptions::default()
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    // ルビ 3文字 × 0.5 = 1.5、はみ出し 0.5 を引いて 1.0(親文字を広げない)
    assert_eq!(layout.pages[0].rubies[0].width, 1.0);
}

#[test]
fn 上付き下付きは小さい送り幅で並び_行の高さは周りの文字のまま() {
    let script = parse("x[sup]2[/sup]あ").unwrap();
    let opts = LayoutOptions {
        latin_space: 0.0,
        ..LayoutOptions::default()
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let page = &layout.pages[0];
    // 「2」は半角 0.5 × 0.6 = 0.3
    assert!((page.glyphs[1].width - 0.3).abs() < 1e-6);
    assert!((page.glyphs[2].x - 0.8).abs() < 1e-6);
    assert_eq!(page.lines[0].size, 1.0);
}

#[test]
fn 線は行ごとに分け_色が変わると分ける() {
    let script = parse("[u]あい[color red]う[/color]えお[/u]か").unwrap();
    let opts = LayoutOptions {
        max_width: 4.0,
        ..LayoutOptions::default()
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let d: Vec<_> = layout.pages[0]
        .decorations
        .iter()
        .map(|d| (d.line, d.x0, d.x1, d.first_glyph, d.glyph_count))
        .collect();
    assert_eq!(
        d,
        vec![
            (0, 0.0, 2.0, 0, 2),
            (0, 2.0, 3.0, 2, 1),
            (0, 3.0, 4.0, 3, 1),
            (1, 0.0, 1.0, 4, 1)
        ]
    );
    let u = &layout.pages[0].decorations[0];
    assert_eq!(u.kind, LineKind::Underline);
    assert!((u.y - 0.98).abs() < 1e-6 && (u.thickness - 0.05).abs() < 1e-6);
}

#[test]
fn 取り消し線と下線は重ねられ_縦書きの下線は行の右側に引く() {
    let script = parse("[s][u]あい[/u][/s]").unwrap();
    let opts = LayoutOptions {
        writing: Writing::Vertical,
        ..LayoutOptions::default()
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let d = &layout.pages[0].decorations;
    assert_eq!(d.len(), 2);
    assert_eq!((d[0].kind, d[0].y), (LineKind::Strike, 0.5));
    assert_eq!(d[1].kind, LineKind::Underline);
    assert!(d[1].y < 0.0);
}

fn xs(src: &str, opts: LayoutOptions) -> Vec<Vec<(char, f32, f32)>> {
    let script = parse(src).unwrap();
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let page = &layout.pages[0];
    page.lines
        .iter()
        .map(|l| {
            page.glyphs[l.first_glyph..l.first_glyph + l.glyph_count]
                .iter()
                .map(|g| (g.ch, g.x, g.width))
                .collect()
        })
        .collect()
}

#[test]
fn 約物が続くと前の約物の後ろの半角を詰める() {
    let l = xs("あ」「い", LayoutOptions::default());
    assert_eq!(
        l[0],
        vec![
            ('あ', 0.0, 1.0),
            ('」', 1.0, 0.5),
            ('「', 1.5, 1.0),
            ('い', 2.5, 1.0)
        ]
    );
    // 句点の後ろに閉じ括弧、読点の後ろに始め括弧
    assert_eq!(xs("。」", LayoutOptions::default())[0][0].2, 0.5);
    assert_eq!(xs("、「", LayoutOptions::default())[0][0].2, 0.5);
    // 約物でない文字が続くときは詰めない。始め括弧の連続は前の括弧を詰めない(後ろを詰める)
    assert_eq!(xs("」あ", LayoutOptions::default())[0][0].2, 1.0);
    assert_eq!(xs("「『", LayoutOptions::default())[0][0].2, 1.0);
    let off = LayoutOptions {
        punct_squeeze: false,
        ..LayoutOptions::default()
    };
    assert_eq!(xs("」「", off)[0][0].2, 1.0);
}

#[test]
fn 始め括弧が続くと後ろの括弧の前の半角を詰め_字形を前へずらす() {
    let l = build("「『（あ）』」", 20.0);
    let g = &l.pages[0].glyphs;
    let got: Vec<(char, f32, f32, f32)> = g.iter().map(|g| (g.ch, g.x, g.width, g.shift)).collect();
    assert_eq!(
        got,
        vec![
            ('「', 0.0, 1.0, 0.0),
            ('『', 1.0, 0.5, -0.5),
            ('（', 1.5, 0.5, -0.5),
            ('あ', 2.0, 1.0, 0.0),
            ('）', 3.0, 0.5, 0.0),
            ('』', 3.5, 0.5, 0.0),
            ('」', 4.0, 1.0, 0.0),
        ]
    );
    // 描画命令の位置はずれを足した値(字面は前の括弧の空きに入り、字の枠に収まる)
    let mut r = crate::Reveal::new(&l, crate::RevealOptions::default());
    r.skip();
    let xs: Vec<f32> = r.glyphs(&l).iter().map(|g| g.x).collect();
    assert_eq!(&xs[..4], &[0.0, 0.5, 1.0, 2.0]);
    // 大きさに比例する
    let l = build("[size 2]「『[/size]", 20.0);
    assert_eq!(l.pages[0].glyphs[1].width, 1.0);
    assert_eq!(l.pages[0].glyphs[1].shift, -1.0);
}

#[test]
fn 始め括弧の連続は約物の詰めを切るか禁則が無ければ詰めない() {
    let no_squeeze = LayoutOptions::default().punct_squeeze(false);
    let no_kinsoku = LayoutOptions::default().kinsoku(Kinsoku::Off);
    for opts in [no_squeeze, no_kinsoku] {
        let l = Layout::build(&parse("「『あ").unwrap(), &CellMetrics, &opts, &Vars::new());
        let g = &l.pages[0].glyphs[1];
        assert_eq!((g.x, g.width, g.shift), (1.0, 1.0, 0.0));
    }
    // ルビの親文字の始め括弧は詰めない
    let l = build("「[ruby かっこ]『[/ruby]", 20.0);
    assert_eq!(l.pages[0].glyphs[1].shift, 0.0);
}

#[test]
fn 約物の詰めで行に収まる文字が増える() {
    let opts = LayoutOptions {
        max_width: 2.5,
        ..LayoutOptions::default()
    };
    // い(1.0) + 」(詰めて 0.5) + 』(1.0) = 2.5 で 1 行に入る
    let l = xs("い」』あ", opts);
    assert_eq!(l[0].iter().map(|g| g.0).collect::<String>(), "い」』");
    assert!((l[0][2].1 - 1.5).abs() < 1e-6);
}

#[test]
fn 和文と半角英数の間に四分のアキを入れ_行頭には入れない() {
    let l = xs("あabcい", LayoutOptions::default());
    // a・b・c は半角 0.5
    let x: Vec<f32> = l[0].iter().map(|g| g.1).collect();
    assert_eq!(x, vec![0.0, 1.25, 1.75, 2.25, 3.0]);
    // 約物との間には入れない
    let l = xs("「abc」", LayoutOptions::default());
    assert_eq!(l[0][1].1, 1.0);
    // 行頭の英単語の前には入れない
    let opts = LayoutOptions {
        max_width: 2.0,
        ..LayoutOptions::default()
    };
    let l = xs("あいabc", opts);
    assert_eq!((l[1][0].0, l[1][0].1), ('a', 0.0));
}

#[test]
fn 両端揃えは段落の最後の行を除き塊の間を広げて行末を揃える() {
    let opts = LayoutOptions {
        max_width: 4.0,
        justify: true,
        ..LayoutOptions::default()
    };
    // 1 行目は「っ」の追い出しで 3 文字になる → 4.0 に揃える
    let script = parse("あいうきっと\nかき").unwrap();
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let page = &layout.pages[0];
    assert_eq!(page.lines[0].glyph_count, 3);
    assert!((page.lines[0].width - 4.0).abs() < 1e-5);
    assert!((page.glyphs[1].x - 1.5).abs() < 1e-5);
    assert!((page.glyphs[2].x - 3.0).abs() < 1e-5);
    // 段落の最後の行(「きっと」と「かき」)は揃えない
    assert_eq!(page.lines[1].width, 3.0);
    assert_eq!(page.lines[2].width, 2.0);
}

/// 行ごとの (字下げ, 行の文字列)
fn indented(src: &str, width: f32, indent: Indent) -> Vec<(f32, String)> {
    let opts = LayoutOptions::default()
        .max_width(width)
        .latin_space(0.0)
        .indent(indent);
    let script = parse(src).unwrap();
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    layout
        .pages
        .iter()
        .flat_map(|page| {
            page.lines.iter().map(move |l| {
                let gs = &page.glyphs[l.first_glyph..l.first_glyph + l.glyph_count];
                if let Some(g) = gs.first() {
                    assert!((g.x - l.indent).abs() < 1e-5, "最初の文字は字下げの位置");
                }
                (l.indent, gs.iter().map(|g| g.ch).collect())
            })
        })
        .collect()
}

#[test]
fn 字下げの既定はどの行も0() {
    let ls = indented("あいうえお\n「かきくけこ」", 3.0, Indent::default());
    assert!(ls.iter().all(|(i, _)| *i == 0.0), "{ls:?}");
}

#[test]
fn 地の文は1行目を字下げし_その分だけ行に入る文字が減る() {
    let ls = indented("あいうえおか", 4.0, Indent::novel());
    assert_eq!(
        ls,
        [(1.0, "あいう".into()), (0.0, "えおか".into())],
        "1行目は 4 - 1 = 3 文字"
    );
    // 行の幅は字下げを含む
    let script = parse("あいう").unwrap();
    let opts = LayoutOptions::default().indent(Indent::novel());
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    assert_eq!(layout.pages[0].lines[0].width, 4.0);
    assert_eq!(layout.pages[0].extent().width, 4.0);
}

#[test]
fn 会話の1行目は天付き_半角下げ_全角下げを選べる() {
    for (v, want) in [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0)] {
        let ls = indented("「あい」", 10.0, Indent::novel().bracket_first(v));
        assert_eq!(ls[0].0, want);
    }
}

#[test]
fn 会話の2行目以降は括弧の後ろに揃えられる() {
    let ls = indented("「あいうえおかき」", 4.0, Indent::dialogue());
    assert_eq!(
        ls,
        [
            (0.0, "「あいう".into()),
            (1.0, "えおか".into()),
            (1.0, "き」".into())
        ]
    );
}

#[test]
fn 括弧が閉じないまま改行した続きの段落は会話の2行目以降と同じ字下げ() {
    let src = "「あいう\nえお」\nかきく";
    let ls = indented(src, 10.0, Indent::novel().bracket_rest(1.0));
    assert_eq!(
        ls,
        [
            (0.0, "「あいう".into()),
            (1.0, "えお」".into()),
            (1.0, "かきく".into())
        ],
        "括弧が閉じた後は地の文に戻る"
    );
    // 改ページをまたいでも続き
    let ls = indented("「あい[p]うえ」", 10.0, Indent::dialogue());
    assert_eq!(ls[1], (1.0, "うえ」".into()));
    // 入れ子の括弧は外側が閉じるまで続き
    let ls = indented("「あ『い』\nう」\nえ", 10.0, Indent::dialogue());
    assert_eq!(ls[1].0, 1.0);
    assert_eq!(ls[2].0, 0.0);
}

#[test]
fn 全角の空白で始まる段落は二重に字下げしない() {
    let ls = indented("\u{3000}あい\nうえ", 10.0, Indent::novel());
    assert_eq!(ls[0], (0.0, "\u{3000}あい".into()));
    assert_eq!(ls[1], (1.0, "うえ".into()));
}

#[test]
fn 空行は字下げせず_負の字下げは0とみなす() {
    let ls = indented("あ\n\nい", 10.0, Indent::novel());
    assert_eq!(ls[1], (0.0, String::new()));
    let ls = indented("あい", 10.0, Indent::default().first(-2.0).rest(f32::NAN));
    assert_eq!(ls[0].0, 0.0);
}

#[test]
fn 字下げした行でも禁則とぶら下げは効き_両端揃えは字下げの残りの幅に揃える() {
    // 2行目の残りは 3。「。」はぶら下げる
    let ls = indented("「あいうえおか。ね」", 4.0, Indent::dialogue());
    assert_eq!(ls[1], (1.0, "えおか。".into()), "{ls:?}");
    let opts = LayoutOptions::default()
        .max_width(5.0)
        .justify(true)
        .indent(Indent::novel());
    let script = parse("あいうきっと").unwrap();
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let l = &layout.pages[0].lines[0];
    // 1行目は 1 + 「あいう」(「き」の後ろの「っ」で追い出し)→ 行末を 5 に揃える
    assert_eq!(l.glyph_count, 3);
    assert!((l.width - 5.0).abs() < 1e-5, "{}", l.width);
}

#[test]
fn 縦書きでも行の上端から字下げする() {
    let opts = LayoutOptions::default()
        .vertical()
        .max_width(4.0)
        .indent(Indent::novel());
    let script = parse("あいうえ").unwrap();
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let page = &layout.pages[0];
    assert_eq!(page.glyphs[0].x, 1.0);
    assert_eq!(page.lines[0].glyph_count, 3);
}

/// 行ごとの (行頭のアキ, 最初の文字の x, 行の幅)
fn aligned(src: &str, opts: LayoutOptions) -> Vec<(f32, f32, f32)> {
    let script = parse(src).unwrap();
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let page = &layout.pages[0];
    page.lines
        .iter()
        .map(|l| {
            let x = page.glyphs.get(l.first_glyph).map_or(0.0, |g| g.x);
            (l.indent, if l.glyph_count > 0 { x } else { 0.0 }, l.width)
        })
        .collect()
}

#[test]
fn 中央揃えと行末揃えは段落の行ごとに残りの幅だけずらす() {
    let opts = LayoutOptions::default().max_width(6.0);
    assert_eq!(
        aligned("[align center]あい[/align]", opts.clone()),
        [(2.0, 2.0, 4.0)]
    );
    assert_eq!(
        aligned("[align end]あい[/align]", opts.clone()),
        [(4.0, 4.0, 6.0)]
    );
    // 折り返した行もそれぞれ揃える
    assert_eq!(
        aligned("[align center]あいうえおかきく[/align]", opts.clone()),
        [(0.0, 0.0, 6.0), (2.0, 2.0, 4.0)]
    );
    // 既定は行頭揃え。揃えの無い段落には設定の揃えを使う
    assert_eq!(aligned("あい", opts.clone()), [(0.0, 0.0, 2.0)]);
    assert_eq!(
        aligned("あい", opts.clone().align(Align::End)),
        [(4.0, 4.0, 6.0)]
    );
}

#[test]
fn 揃えは段落ごとで_ルビも一緒にずれる() {
    let opts = LayoutOptions::default().max_width(6.0);
    let script = parse("[align center][ruby ねこ]猫[/ruby][/align]\nあ").unwrap();
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let page = &layout.pages[0];
    assert_eq!(page.lines[0].align, Align::Center);
    assert_eq!(page.lines[1].align, Align::Start);
    assert_eq!(page.glyphs[0].x, 2.5);
    assert_eq!(page.rubies[0].x, 2.5);
    assert_eq!(page.glyphs[1].x, 0.0);
}

#[test]
fn 中央揃えと行末揃えでは字下げも両端揃えもしない() {
    let opts = LayoutOptions::default()
        .max_width(4.0)
        .justify(true)
        .indent(Indent::novel());
    assert_eq!(
        aligned("[align end]あいうえおか[/align]", opts),
        [(0.0, 0.0, 4.0), (2.0, 2.0, 4.0)]
    );
}

#[test]
fn 幅を超えた行は揃えでずらさない() {
    // ぶら下げた句読点で幅を超える
    let opts = LayoutOptions::default().max_width(3.0);
    let ls = aligned("[align end]あいう。[/align]", opts);
    assert_eq!(ls[0].0, 0.0);
    assert_eq!(ls[0].2, 4.0);
}

#[test]
fn 縦書きの行末揃えは下に揃える() {
    let opts = LayoutOptions::default().vertical().max_width(5.0);
    assert_eq!(aligned("[align end]あい[/align]", opts), [(3.0, 3.0, 5.0)]);
}

#[test]
fn 両端揃えでも英単語の中とルビの親文字の中は広げない() {
    let opts = LayoutOptions {
        max_width: 5.5,
        justify: true,
        latin_space: 0.0,
        ..LayoutOptions::default()
    };
    // 「 は行末に置けないので、1 行目は ab・字字・あ の 3 塊(幅 4.0)。間 2 か所に 0.75 ずつ
    let l = xs("ab[ruby よ]字字[/ruby]あ「いい", opts);
    let first: Vec<(char, f32)> = l[0].iter().map(|g| (g.0, g.1)).collect();
    assert_eq!(
        first,
        vec![
            ('a', 0.0),
            ('b', 0.5),
            ('字', 1.75),
            ('字', 2.75),
            ('あ', 4.5)
        ]
    );
}

#[test]
fn 圏点は1文字ごとに付き_約物とルビの親文字には付けない() {
    let script = parse("[em]あ、[ruby よ]字[/ruby]い[/em]").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    let page = &layout.pages[0];
    let marks: Vec<(usize, &str, bool)> = page
        .rubies
        .iter()
        .map(|r| (r.first_glyph, r.text.as_str(), r.emphasis))
        .collect();
    assert_eq!(
        marks,
        vec![(0, "﹅", true), (2, "よ", false), (3, "﹅", true)]
    );
    // バックログには圏点を書かない
    assert_eq!(page.text(), "あ、｜字《よ》い");
}

#[test]
fn 縦中横は縦書きで全角1文字分に収め_横に並べる() {
    let script = parse("第[tcy]12[/tcy]話と[tcy]100[/tcy]").unwrap();
    let opts = LayoutOptions {
        writing: Writing::Vertical,
        ..LayoutOptions::default()
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let g = &layout.pages[0].glyphs;
    // 「1」「2」はどちらも x = 1.0 で、送りは全角1文字分。和欧間のアキは入れない
    assert_eq!((g[1].x, g[2].x, g[3].x), (1.0, 1.0, 2.0));
    assert_eq!(
        (g[1].tcy, g[2].tcy),
        (
            Some(TcyPlace {
                offset: 0.0,
                scale: 1.0
            }),
            Some(TcyPlace {
                offset: 0.5,
                scale: 1.0
            })
        )
    );
    // 3 桁は 1.5 になるので 2/3 に縮める
    let t: Vec<TcyPlace> = g[5..8].iter().map(|g| g.tcy.unwrap()).collect();
    assert!((t[0].scale - 2.0 / 3.0).abs() < 1e-6);
    assert!((t[2].offset - 2.0 / 3.0).abs() < 1e-6);
    assert_eq!(g[4].tcy, None);
}

#[test]
fn 縦中横は横書きでは普通に並び_途中で分けない() {
    let script = parse("あ[tcy]12[/tcy]").unwrap();
    let opts = LayoutOptions {
        max_width: 1.5,
        latin_space: 0.0,
        ..LayoutOptions::default()
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let page = &layout.pages[0];
    assert_eq!(page.lines.len(), 2);
    assert_eq!((page.glyphs[1].x, page.glyphs[2].x), (0.0, 0.5));
    assert!(page.glyphs.iter().all(|g| g.tcy.is_none()));
}

fn build_with(src: &str, width: f32, vars: &Vars) -> Layout {
    let script = parse(src).unwrap();
    let opts = LayoutOptions::default().max_width(width).latin_space(0.0);
    Layout::build(&script, &CellMetrics, &opts, vars)
}

#[test]
fn グリフは元の文章のバイト範囲を持つ() {
    let src = "[[あ[b]い[/b][ruby き]木[/ruby]辻\u{E0100}";
    let layout = build_with(src, 10.0, &Vars::new());
    let got: Vec<&str> = layout.pages[0]
        .glyphs
        .iter()
        .map(|g| &src[g.source.clone()])
        .collect();
    assert_eq!(got, vec!["[[", "あ", "い", "木", "辻\u{E0100}"]);
    assert!(layout.pages[0].glyphs.iter().all(|g| g.var.is_none()));
}

#[test]
fn 変数から来た文字は変数の番号を持ち_元の範囲は変数全体() {
    let mut vars = Vars::new();
    vars.insert("name".into(), "ミケ".into());
    let src = "[var who]と[var name]と[var name]";
    let layout = build_with(src, 20.0, &vars);
    assert_eq!(layout.vars, vec!["who".to_string(), "name".to_string()]);
    let g = &layout.pages[0].glyphs;
    // 値がない who は `[var who]` のまま 9 文字(元の範囲は `[var who]` 全体)
    assert!(g[..9]
        .iter()
        .all(|g| g.var == Some(0) && g.source == (0..9)));
    assert_eq!(g[9].var, None);
    assert!(g[10..12]
        .iter()
        .all(|g| g.var == Some(1) && g.source == (12..22)));
    assert!(g[13..15]
        .iter()
        .all(|g| g.var == Some(1) && g.source == (25..35)));
    assert_eq!(layout.pages[0].glyph_at_source(14), Some(10));
    assert_eq!(layout.pages[0].glyph_at_source(9), Some(9));
}

#[test]
fn 名前付きの範囲は行をまたぐと行ごとの矩形になる() {
    let layout = build_with(
        "あ[mark 竜]いうえ[/mark]お[mark x]か[/mark]",
        3.0,
        &Vars::new(),
    );
    let marks = layout.marks(0);
    assert_eq!(
        marks
            .iter()
            .map(|m| (m.id.as_str(), m.first_glyph, m.glyph_count))
            .collect::<Vec<_>>(),
        vec![("竜", 1, 3), ("x", 5, 1)]
    );
    let spans = layout.mark_spans(0, "竜");
    assert_eq!(
        spans
            .iter()
            .map(|s| (s.line, s.x0, s.x1))
            .collect::<Vec<_>>(),
        vec![(0, 1.0, 3.0), (1, 0.0, 1.0)]
    );
    let page = &layout.pages[0];
    assert_eq!(layout.marks_of(&page.glyphs[2]), ["竜".to_string()]);
    assert!(layout.marks_of(&page.glyphs[4]).is_empty());
}

#[test]
fn 同じ_id_の入れ子はひとつの範囲として数える() {
    let layout = build_with("[mark a]あ[mark a]い[/mark]う[/mark]", 10.0, &Vars::new());
    let marks = layout.marks(0);
    assert_eq!(marks.len(), 1);
    assert_eq!((marks[0].first_glyph, marks[0].glyph_count), (0, 3));
}

#[test]
fn 当たり判定は論理座標の点にあるグリフを返す() {
    let layout = build_with("あいう\nAB", 10.0, &Vars::new());
    let page = &layout.pages[0];
    assert_eq!(page.hit_test(1.5, 0.2), Some(1));
    assert_eq!(page.hit_test(0.0, 0.0), Some(0));
    assert_eq!(page.hit_test(3.0, 0.5), None, "行末より先");
    assert_eq!(page.hit_test(0.7, 1.9), Some(4), "B は 0.5〜1.0");
    assert_eq!(page.hit_test(0.5, 2.0), None, "行がない");
    assert_eq!(page.hit_test(0.5, -0.1), None);
    assert_eq!(page.hit_test(f32::NAN, 0.0), None);
}

#[test]
fn ページの外寸は最も長い行の幅と行数() {
    let layout = build_with("あいうえ\nお", 10.0, &Vars::new());
    let e = layout.pages[0].extent();
    assert_eq!((e.width, e.lines), (4.0, 2));
    let empty = build_with("", 10.0, &Vars::new());
    assert_eq!(empty.pages[0].extent().width, 0.0);
}

#[test]
fn バックログのテキストは行ごとに改行しルビを青空文庫の形で書く() {
    let script = parse("[ruby めいきゅう]迷宮[/ruby]の奥へ。").unwrap();
    let opts = LayoutOptions {
        max_width: 3.0,
        ..LayoutOptions::default()
    };
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    assert_eq!(layout.pages[0].text(), "｜迷宮《めいきゅう》の\n奥へ。");
}

#[test]
fn 数の中の区切りと前後の記号では改行しない() {
    let k = Kinsoku::Standard;
    assert_eq!(lines("ああああ3.14です", 5.0, k), ["ああああ", "3.14です"]);
    assert_eq!(lines("あああ1,000円", 5.0, k), ["あああ", "1,000円"]);
    assert_eq!(lines("ああああ$100です", 5.0, k), ["ああああ", "$100です"]);
    assert_eq!(lines("ああああ100%です", 6.5, k), ["ああああ100%", "です"]);
    // 数でなければ今までどおり記号の前後で折り返せる
    assert_eq!(lines("あああa.bcd", 5.0, k), ["あああa.", "bcd"]);
}

#[test]
fn 語の後ろのハイフンは前の語に付けハイフンの後ろで折り返す() {
    let k = Kinsoku::Standard;
    assert_eq!(
        lines("ああああwell-known", 7.0, k),
        ["ああああwell-", "known"]
    );
    assert_eq!(
        lines("ああああwell-known", 6.0, k),
        ["ああああ", "well-known"]
    );
}

#[test]
fn スタイルの境目で分かれた語は途中で改行しない() {
    for k in [Kinsoku::Standard, Kinsoku::Off] {
        assert_eq!(
            lines("ああああhel[b]lo[/b]x", 5.5, k),
            ["ああああ", "hellox"]
        );
    }
    // ルビの後ろは語の続きにしない
    assert_eq!(
        lines("あああab[ruby よみ]c[/ruby]de", 5.0, Kinsoku::Standard),
        ["あああabc", "de"]
    );
    // どの行にも収まらなければ、つながった語ごと文字の間で折り返す
    assert_eq!(
        lines("ああ[b]abcdefghijklmn[/b]opqrstu", 5.0, Kinsoku::Standard),
        ["ああabcde", "fghijklmno", "pqrstu"]
    );
}

#[test]
fn 段落ごとに始め括弧を開き直した会話の後の地の文は字下げする() {
    let script = parse("「一段落目\n「二段落目」\n地の文。[p]地の文。").unwrap();
    let opts = LayoutOptions::default()
        .max_width(20.0)
        .indent(Indent::novel());
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let indents: Vec<f32> = layout
        .pages
        .iter()
        .flat_map(|p| p.lines.iter().map(|l| l.indent))
        .collect();
    assert_eq!(indents, [0.0, 0.0, 1.0, 1.0]);
    // 閉じないまま改行した続きは、今までどおり会話の続き
    let script = parse("「一段落目\n二段落目」\n地の文。").unwrap();
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let p = &layout.pages[0];
    assert_eq!(p.lines[2].indent, 1.0);
    assert_eq!(p.lines[1].indent, Indent::novel().bracket_rest);
}

#[test]
fn 範囲の矩形は数があふれても止まらない() {
    let layout = build("あいう", 10.0);
    assert_eq!(layout.pages[0].spans(1, usize::MAX)[0].glyph_count, 2);
}

#[test]
fn 両端揃えはスタイルの境目でつながった語の中を広げない() {
    let script = parse("ああhel[b]lo[/b]いいいいいいいい").unwrap();
    let opts = LayoutOptions::default()
        .max_width(8.0)
        .latin_space(0.0)
        .justify(true);
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let g = &layout.pages[0].glyphs;
    // h e l | l o は半角ずつ詰めて並ぶ
    for k in 2..6 {
        assert!((g[k + 1].x - g[k].x - 0.5).abs() < 1e-4, "{k}");
    }
    let line = &layout.pages[0].lines[0];
    let last = &g[line.first_glyph + line.glyph_count - 1];
    assert!((last.x + 1.0 - 8.0).abs() < 1e-4, "行末は揃う");
}

#[test]
fn 変数の値の改行は改行にしタブは空白にほかの制御文字は捨てる() {
    let script = parse("こんにちは[var name]さん").unwrap();
    let mut vars = Vars::new();
    vars.insert("name".into(), "A\r\nB\tC\u{7}".into());
    let layout = Layout::build(&script, &CellMetrics, &LayoutOptions::default(), &vars);
    let page = &layout.pages[0];
    assert_eq!(page.text(), "こんにちはA\nB Cさん");
    assert!(page.lines[1].paragraph_start);
    assert!(page
        .glyphs
        .iter()
        .filter(|g| g.ch == 'B')
        .all(|g| g.var.is_some()));
}

#[test]
fn 時刻や日付の中と通貨記号の後ろでは改行しない() {
    let k = Kinsoku::Standard;
    assert_eq!(
        lines("ああああ10:30です", 5.0, k),
        ["ああああ", "10:30です"]
    );
    assert_eq!(lines("あああ12/25です", 5.0, k), ["あああ", "12/25です"]);
    assert_eq!(lines("あああ12-25です", 5.0, k), ["あああ", "12-25です"]);
    assert_eq!(lines("あああUS$100", 5.0, k), ["あああ", "US$100"]);
    assert_eq!(lines("ああああ¥500です", 5.0, k), ["ああああ", "¥500です"]);
}

#[test]
fn アクセント付きのラテン文字も英単語の一部にする() {
    assert_eq!(
        lines("あああcafé", 5.0, Kinsoku::Standard),
        ["あああ", "café"]
    );
    // 和欧間のアキも入る
    let script = parse("あé").unwrap();
    let layout = Layout::build(
        &script,
        &CellMetrics,
        &LayoutOptions::default(),
        &Vars::new(),
    );
    assert!((layout.pages[0].glyphs[1].x - 1.25).abs() < 1e-4);
}

#[test]
fn 折り返した行の頭に半角空白を置かず行末の空白は幅に数えない() {
    let k = Kinsoku::Standard;
    assert_eq!(lines("hello world foo", 6.0, k), ["hello world ", "foo"]);
    assert_eq!(
        lines("hello world foo", 4.0, k),
        ["hello ", "world ", "foo"]
    );
    let script = parse("[align end]ab cd[/align]").unwrap();
    let opts = LayoutOptions::default().max_width(2.0);
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let page = &layout.pages[0];
    // 1行目 "ab " は "ab" の幅で行末に揃う
    assert!((page.glyphs[0].x - 1.0).abs() < 1e-4);
    assert!((page.lines[0].width - 2.0).abs() < 1e-4);
    // 両端揃えでも行末の空白は広げない
    let script = parse("ab cd ef gh").unwrap();
    let opts = LayoutOptions::default().max_width(4.0).justify(true);
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let page = &layout.pages[0];
    let l0 = &page.lines[0];
    let last = &page.glyphs[l0.first_glyph + l0.glyph_count - 2];
    assert!((last.x + last.width - 4.0).abs() < 1e-4, "{}", last.x);
}

#[test]
fn 行末に残した半角空白には線を引かない() {
    let script = parse("[u]hello world[/u]").unwrap();
    let opts = LayoutOptions::default().max_width(3.0);
    let layout = Layout::build(&script, &CellMetrics, &opts, &Vars::new());
    let d = &layout.pages[0].decorations;
    assert_eq!(d.len(), 2);
    assert_eq!((d[0].glyph_count, d[0].x1), (5, 2.5));
}

#[test]
fn 空白の後ろが行頭禁則の文字なら空白の前でも改行しない() {
    // 空白の前で改行すると空白は行末に残り、っ が行頭に来てしまう
    let k = Kinsoku::Standard;
    let l = lines("ああknown  っ", 4.0, k);
    assert!(l.iter().all(|s| !s.starts_with('っ')), "{l:?}");
}

#[test]
fn スタイルの境目をまたいだ数の区切りでも改行しない() {
    let k = Kinsoku::Standard;
    assert_eq!(
        lines("ああああ3[b].14[/b]です", 5.0, k),
        ["ああああ", "3.14です"]
    );
    assert_eq!(
        lines("ああああ3.[b]14[/b]です", 5.0, k),
        ["ああああ", "3.14です"]
    );
}

#[test]
fn とても長い英単語も語の長さに比例する手間で幅に収めて折り返す() {
    // 以前は文字ごとに塊全体を複製していて、語の長さの2乗の手間がかかった
    let src = "a".repeat(20_000);
    let layout = build(&src, 10.0);
    let glyphs: usize = layout.pages.iter().map(|p| p.glyphs.len()).sum();
    assert_eq!(glyphs, 20_000);
    for page in &layout.pages {
        assert!(page.extent().width <= 10.0 + 1e-3);
    }
}

/// 「い」だけ送り幅に `v` を返し、線の位置と太さにも `v` を返す計測。
struct BadMetrics(f32);
impl FontMetrics for BadMetrics {
    fn advance(&self, ch: char, size: f32) -> f32 {
        if ch == 'い' {
            self.0
        } else {
            size
        }
    }
    fn vertical_advance(&self, ch: char, size: f32) -> f32 {
        self.advance(ch, size)
    }
    fn line_metrics(&self, _: LineKind, _: f32) -> (f32, f32) {
        (self.0, self.0)
    }
}

#[test]
fn 計測が有限でない値や負の値を返しても0として組み_位置は有限のまま() {
    let script = parse("[ruby いい]親[/ruby]あいう[u]いか[/u][tcy]1い[/tcy]\nい[s]い[/s]").unwrap();
    for v in [f32::NAN, -1.0, f32::INFINITY, f32::NEG_INFINITY] {
        for opts in [
            LayoutOptions::default().max_width(5.0),
            LayoutOptions::default().max_width(5.0).vertical(),
        ] {
            let layout = Layout::build(&script, &BadMetrics(v), &opts, &Vars::new());
            for page in &layout.pages {
                for g in &page.glyphs {
                    assert!(g.x.is_finite() && g.width >= 0.0, "{v}: {g:?}");
                }
                for d in &page.decorations {
                    assert!(
                        d.x0.is_finite()
                            && d.x1.is_finite()
                            && d.y.is_finite()
                            && d.thickness >= 0.0,
                        "{v}: {d:?}"
                    );
                }
            }
            let i = layout.pages[0]
                .glyphs
                .iter()
                .position(|g| g.ch == 'い')
                .unwrap();
            assert_eq!(layout.pages[0].glyphs[i].width, 0.0);
            // 猫モードも止まらない
            let mut r = crate::Reveal::new(&layout, crate::RevealOptions::default());
            let neko = crate::Neko::new(&layout, crate::NekoMode::default().seed(1));
            for _ in 0..20 {
                r.advance(0.3);
                for g in neko.glyphs(&layout, &r) {
                    assert!(g.x.is_finite());
                }
                r.proceed();
            }
        }
    }
}

#[test]
fn バックログのルビは短い書き方で書けないときだけ_ruby_タグで書く() {
    let build = |src: &str| {
        let script = parse(src).unwrap();
        Layout::build(
            &script,
            &CellMetrics,
            &LayoutOptions::default(),
            &Vars::new(),
        )
    };
    // 親文字が 33 文字・空白を含む・括弧を含む・読みが《》を含む
    let long = "あ".repeat(33);
    for (src, want) in [
        (
            format!("[ruby よみ]{long}[/ruby]"),
            format!("[ruby よみ]{long}[/ruby]"),
        ),
        (
            "[ruby よみ]a b[/ruby]".into(),
            "[ruby よみ]a b[/ruby]".into(),
        ),
        (
            "[ruby よみ][[x[/ruby]".into(),
            "[ruby よみ][[x[/ruby]".into(),
        ),
        (
            "[ruby 《よ》]親[/ruby]".into(),
            "[ruby 《よ》]親[/ruby]".into(),
        ),
    ] {
        let text = build(&src).pages[0].text();
        assert_eq!(text, want, "{src}");
        // 書き出したものを読み戻すと同じ文字列になる
        assert_eq!(build(&text).pages[0].text(), text, "{src}");
    }
    // 短い書き方で書けるものは今までどおり(読みの空白も読み戻せる)
    let text = build("｜親《よ み》").pages[0].text();
    assert_eq!(text, "｜親《よ み》");
    assert_eq!(build(&text).pages[0].text(), text);
}
