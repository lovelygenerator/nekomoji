use super::*;

#[test]
fn 文字だけなら一つの_text_になる() {
    let s = parse("こんにちは").unwrap();
    assert_eq!(
        s.items,
        vec![Item::Text {
            text: "こんにちは".into(),
            style: 0,
            source: vec![0..3, 3..6, 6..9, 9..12, 12..15],
        }]
    );
}

#[test]
fn 範囲タグでスタイルが切り替わり閉じると戻る() {
    let s = parse("あ[color red]い[/color]う").unwrap();
    assert_eq!(s.items.len(), 3);
    let Item::Text { style, .. } = &s.items[1] else {
        panic!()
    };
    assert_eq!(s.styles[*style].color.as_deref(), Some("red"));
    assert!(matches!(&s.items[2], Item::Text { style: 0, .. }));
}

#[test]
fn ルビ_待機_イベントを読める() {
    let s = parse("[ruby めいきゅう]迷宮[/ruby][wait 0.5][voice v1][p]").unwrap();
    assert_eq!(
        s.items,
        vec![
            Item::Ruby {
                base: "迷宮".into(),
                reading: "めいきゅう".into(),
                style: 0,
                source: vec![22..25, 25..28],
            },
            Item::Wait(0.5),
            Item::Event(TextEvent::Voice("v1".into())),
            Item::Page,
        ]
    );
}

#[test]
fn 二重の括弧は文字として扱う() {
    let s = parse("[[a]] {b}").unwrap();
    assert_eq!(
        s.items,
        vec![Item::Text {
            text: "[a] {b}".into(),
            style: 0,
            source: vec![0..2, 2..3, 3..5, 5..6, 6..7, 7..8, 8..9],
        }]
    );
}

#[test]
fn mark_タグは範囲の_id_を外側から順にスタイルへ積む() {
    let s = parse("[mark a]い[mark b]ろ[/mark][/mark]は").unwrap();
    let marks: Vec<Vec<String>> = s
        .items
        .iter()
        .map(|it| match it {
            Item::Text { style, .. } => s.styles[*style].marks.clone(),
            _ => panic!(),
        })
        .collect();
    assert_eq!(
        marks,
        vec![
            vec!["a".to_string()],
            vec!["a".to_string(), "b".to_string()],
            vec![]
        ]
    );
}

#[test]
fn mark_タグは_id_を1つだけ取る() {
    assert_eq!(
        parse("[mark]あ[/mark]").unwrap_err().kind,
        ErrorKind::MissingArg("mark".into())
    );
    assert_eq!(
        parse("[mark a b]あ[/mark]").unwrap_err().kind,
        ErrorKind::BadArg {
            tag: "mark".into(),
            arg: "b".into()
        }
    );
    assert_eq!(
        parse("[mark a]あ").unwrap_err().kind,
        ErrorKind::Unclosed("mark".into())
    );
}

#[test]
fn align_タグは揃えを1つ取り_段落の頭にだけ書ける() {
    let st = |src: &str| {
        let s = parse(src).unwrap();
        s.styles[match &s.items[s.items.len() - 1] {
            Item::Text { style, .. } => *style,
            _ => unreachable!(),
        }]
        .align
    };
    assert_eq!(st("[align center]あ[/align]"), Some(Align::Center));
    assert_eq!(st("[align right]あ[/align]"), Some(Align::End));
    assert_eq!(st("[align left]あ[/align]"), Some(Align::Start));
    assert_eq!(st("い\n[align end]あ[/align]"), Some(Align::End));
    assert_eq!(st("い[br][wait 1][align end]あ[/align]"), Some(Align::End));
    assert_eq!(
        st("い[p][color red][align end]あ[/align][/color]"),
        Some(Align::End)
    );
    assert_eq!(
        parse("[align]あ[/align]").unwrap_err().kind,
        ErrorKind::MissingArg("align".into())
    );
    assert_eq!(
        parse("[align middle]あ[/align]").unwrap_err().kind,
        ErrorKind::BadArg {
            tag: "align".into(),
            arg: "middle".into()
        }
    );
    let e = parse("あ\nい[align center]う[/align]").unwrap_err();
    assert_eq!(e.kind, ErrorKind::AlignMidParagraph);
    assert_eq!((e.line, e.col), (2, 2));
}

#[test]
fn blip_タグは音を0か1つ取る() {
    let st = |src: &str| {
        let s = parse(src).unwrap();
        s.styles[match &s.items[0] {
            Item::Text { style, .. } => *style,
            _ => unreachable!(),
        }]
        .blip
        .clone()
    };
    assert_eq!(st("[blip]あ[/blip]"), Some(String::new()));
    assert_eq!(st("[blip mike]あ[/blip]"), Some("mike".into()));
    assert_eq!(st("[blip off]あ[/blip]"), Some("off".into()));
    assert_eq!(
        parse("[blip a b]あ[/blip]").unwrap_err().kind,
        ErrorKind::BadArg {
            tag: "blip".into(),
            arg: "b".into()
        }
    );
}

#[test]
fn l_タグはページの途中の入力待ちで_引数をとらない() {
    let s = parse("あ[l]い").unwrap();
    assert_eq!(s.items[1], Item::Click);
    assert_eq!(
        parse("あ[l 1]").unwrap_err().kind,
        ErrorKind::BadArg {
            tag: "l".into(),
            arg: "1".into()
        }
    );
    // [l] の後も段落の頭のまま
    assert!(parse("あ\n[l][align center]い[/align]").is_ok());
}

#[test]
fn icon_タグは_id_と長さをとり_長さの既定は1() {
    let s = parse("[b][icon btn_a][/b][icon star 1.5]").unwrap();
    assert_eq!(
        s.items[0],
        Item::Icon {
            id: "btn_a".into(),
            width: 1.0,
            style: 1,
            source: 3..15,
        }
    );
    assert!(matches!(&s.items[1], Item::Icon { width, .. } if *width == 1.5));
    assert_eq!(
        parse("[icon]").unwrap_err().kind,
        ErrorKind::MissingArg("icon".into())
    );
    for bad in ["[icon a 0]", "[icon a x]", "[icon a 1 2]"] {
        assert!(
            matches!(parse(bad).unwrap_err().kind, ErrorKind::BadArg { .. }),
            "{bad}"
        );
    }
    assert_eq!(
        parse("[ruby よみ][icon a][/ruby]").unwrap_err().kind,
        ErrorKind::NestedInRuby
    );
}

#[test]
fn 変数の元の範囲はタグを含めた全体() {
    let s = parse("あ\r\n[var name]").unwrap();
    // \r は読み飛ばすが、バイト位置は元の文章のまま
    assert!(matches!(
        &s.items[2],
        Item::Var { source, .. } if *source == (5..15)
    ));
}

#[test]
fn 知らないタグは行と桁つきのエラー() {
    let e = parse("あ\nい[foo]").unwrap_err();
    assert_eq!((e.line, e.col), (2, 2));
    assert_eq!(e.kind, ErrorKind::UnknownTag("foo".into()));
}

#[test]
fn 閉じ忘れは開いた位置で報告する() {
    let e = parse("あ[b]い").unwrap_err();
    assert_eq!((e.line, e.col), (1, 2));
    assert_eq!(e.kind, ErrorKind::Unclosed("b".into()));
}

#[test]
fn 入れ子の閉じ順が違うとエラー() {
    let e = parse("[b][color red]あ[/b][/color]").unwrap_err();
    assert!(matches!(e.kind, ErrorKind::UnexpectedClose { .. }));
}

#[test]
fn ルビの中にタグは書けない() {
    let e = parse("[ruby よみ]あ[b]い[/b][/ruby]").unwrap_err();
    assert_eq!(e.kind, ErrorKind::NestedInRuby);
}

#[test]
fn 数値の引数を検証する() {
    assert!(parse("[wait abc]").is_err());
    assert!(parse("[speed 0]").is_err());
    assert!(parse("[wait 0]").is_ok());
}

#[test]
fn 揺れと波は強さと弱まって止まる秒数をとり_秒は0より大きい() {
    let s = parse("[shake]あ[/shake][shake 2 1.5]い[/shake][wave 1 0.5]う[/wave]").unwrap();
    let effects = |i: usize| s.styles[i].effects.clone();
    let styles: Vec<_> = (0..s.styles.len()).map(effects).collect();
    assert!(styles.contains(&vec![Effect::Shake {
        strength: 1.0,
        decay: None
    }]));
    assert!(styles.contains(&vec![Effect::Shake {
        strength: 2.0,
        decay: Some(1.5)
    }]));
    assert!(styles.contains(&vec![Effect::Wave {
        strength: 1.0,
        decay: Some(0.5)
    }]));
    for bad in [
        "[shake 1 0]あ[/shake]",
        "[wave 1 x]あ[/wave]",
        "[shake 1 1 1]あ[/shake]",
    ] {
        let e = parse(bad).unwrap_err();
        assert!(matches!(e.kind, ErrorKind::BadArg { .. }), "{bad}");
    }
}

#[test]
fn 発光は種類と強さと秒数をとり_種類を省くと_in() {
    let effects = |src: &str| parse(src).unwrap().styles.last().unwrap().effects.clone();
    assert_eq!(
        effects("[glow]あ[/glow]"),
        vec![Effect::Glow {
            kind: GlowKind::In,
            strength: 1.0,
            secs: 1.5
        }]
    );
    assert_eq!(
        effects("[glow 2]あ[/glow]"),
        vec![Effect::Glow {
            kind: GlowKind::In,
            strength: 2.0,
            secs: 1.5
        }]
    );
    assert_eq!(
        effects("[glow breathe 1 3]あ[/glow]"),
        vec![Effect::Glow {
            kind: GlowKind::Breathe,
            strength: 1.0,
            secs: 3.0
        }]
    );
    assert_eq!(
        effects("[glow flicker]あ[/glow]"),
        vec![Effect::Glow {
            kind: GlowKind::Flicker,
            strength: 1.0,
            secs: 3.0
        }]
    );
    for bad in [
        "[glow neon]あ[/glow]",
        "[glow in 1 0]あ[/glow]",
        "[glow in 1 1 1]あ[/glow]",
    ] {
        let e = parse(bad).unwrap_err();
        assert!(matches!(e.kind, ErrorKind::BadArg { .. }), "{bad}");
    }
}

#[test]
fn 縁取りは色と太さをとり_省くと黒の_0_08() {
    let outline = |src: &str| parse(src).unwrap().styles.last().unwrap().outline.clone();
    assert_eq!(outline("[outline]あ[/outline]"), Some(Outline::default()));
    assert_eq!(
        outline("[outline white]あ[/outline]"),
        Some(Outline::default().color("white"))
    );
    assert_eq!(
        outline("[outline 0.15]あ[/outline]"),
        Some(Outline::default().width(0.15))
    );
    assert_eq!(
        outline("[outline #102030 0.1]あ[/outline]"),
        Some(Outline::default().color("#102030").width(0.1))
    );
    for bad in [
        "[outline red 0]あ[/outline]",
        "[outline red -1]あ[/outline]",
        "[outline red inf]あ[/outline]",
        "[outline red 0.1 2]あ[/outline]",
        "[outline 0.1 0.2]あ[/outline]",
        "[outline red blue]あ[/outline]",
    ] {
        let e = parse(bad).unwrap_err();
        assert!(matches!(e.kind, ErrorKind::BadArg { .. }), "{bad}");
    }
}

#[test]
fn 影は色とずれをとり_ずれは負でもよく_省くと黒で右下へ_0_06() {
    let shadow = |src: &str| parse(src).unwrap().styles.last().unwrap().shadow.clone();
    assert_eq!(shadow("[shadow]あ[/shadow]"), Some(Shadow::default()));
    assert_eq!(
        shadow("[shadow gray 0.1]あ[/shadow]"),
        Some(Shadow::default().color("gray").x(0.1))
    );
    assert_eq!(
        shadow("[shadow -0.05 0]あ[/shadow]"),
        Some(Shadow::default().x(-0.05).y(0.0))
    );
    for bad in [
        "[shadow red nan]あ[/shadow]",
        "[shadow red 0.1 0.1 0.1]あ[/shadow]",
        "[shadow 0.1 x]あ[/shadow]",
    ] {
        let e = parse(bad).unwrap_err();
        assert!(matches!(e.kind, ErrorKind::BadArg { .. }), "{bad}");
    }
}

#[test]
fn 縁取りと影は内側のタグが外側を上書きし_ほかの見た目と重なる() {
    let s =
        parse("[outline red][shadow][outline blue 0.2][color white]あ[/color][/outline][/shadow]い[/outline]")
            .unwrap();
    let style_of = |i: usize| match &s.items[i] {
        Item::Text { style, .. } => &s.styles[*style],
        other => panic!("{other:?}"),
    };
    let a = style_of(0);
    assert_eq!(a.outline, Some(Outline::default().color("blue").width(0.2)));
    assert_eq!(a.shadow, Some(Shadow::default()));
    assert_eq!(a.color.as_deref(), Some("white"));
    let i = style_of(1);
    assert_eq!(i.outline, Some(Outline::default().color("red")));
    assert_eq!(i.shadow, None);
}

#[test]
fn 猫の出し方_hop_を読み_強さの既定は_1() {
    let s = parse("[neko hop]あ[/neko][neko hop 2.5]い[/neko]").unwrap();
    assert_eq!(s.styles[1].effects, vec![Effect::NekoHop(1.0)]);
    assert_eq!(s.styles[2].effects, vec![Effect::NekoHop(2.5)]);
}

#[test]
fn 猫の出し方の種類がないか知らなければエラー() {
    let e = parse("[neko]あ[/neko]").unwrap_err();
    assert_eq!(e.kind, ErrorKind::MissingArg("neko".into()));
    let e = parse("あ[neko jump]い[/neko]").unwrap_err();
    assert_eq!((e.line, e.col), (1, 2));
    assert_eq!(
        e.kind,
        ErrorKind::BadArg {
            tag: "neko".into(),
            arg: "jump".into()
        }
    );
}

#[test]
fn 上付き下付きは字を_0_6_倍にしベースラインをずらす() {
    let s = parse("H[sub]2[/sub]O x[sup]2[/sup] [size 2][sup]n[/sup][/size]").unwrap();
    let sub = &s.styles[1];
    assert_eq!(sub.baseline, Baseline::Sub);
    assert!((sub.glyph_size() - 0.6).abs() < 1e-6);
    assert!((sub.rise() + 0.15).abs() < 1e-6);
    let sup = &s.styles[2];
    assert!((sup.rise() - 0.4).abs() < 1e-6);
    // [size] と掛け合わせる。行の高さの基準(size)は周りの文字のまま
    let big = s
        .styles
        .iter()
        .find(|st| st.size == 2.0 && st.baseline == Baseline::Sup)
        .unwrap();
    assert!((big.glyph_size() - 1.2).abs() < 1e-6);
    assert!((big.rise() - 0.8).abs() < 1e-6);
}

#[test]
fn 上付き下付きは入れ子にできず_中にルビも書けない() {
    let e = parse("[sup]a[sub]b[/sub][/sup]").unwrap_err();
    assert_eq!((e.col, e.kind), (7, ErrorKind::NestedInShift));
    let e = parse("[sup][ruby よみ]字[/ruby][/sup]").unwrap_err();
    assert_eq!(e.kind, ErrorKind::NestedInShift);
    let e = parse("[ruby よみ]字[sup]a[/sup][/ruby]").unwrap_err();
    assert_eq!(e.kind, ErrorKind::NestedInRuby);
}

#[test]
fn 取り消し線と下線を読み_引数はとらない() {
    let s = parse("[s]あ[u]い[/u][/s]").unwrap();
    assert!(s.styles[1].strike && !s.styles[1].underline);
    assert!(s.styles[2].strike && s.styles[2].underline);
    let e = parse("[u 2]あ[/u]").unwrap_err();
    assert_eq!(
        e.kind,
        ErrorKind::BadArg {
            tag: "u".into(),
            arg: "2".into()
        }
    );
}

#[test]
fn 圏点は既定でゴマ点_引数で記号を1文字だけ選べる() {
    let s = parse("[em]あ[/em][em ●]い[/em]").unwrap();
    assert_eq!(s.styles[1].emphasis, Some('﹅'));
    assert_eq!(s.styles[2].emphasis, Some('●'));
    let e = parse("[em ●●]あ[/em]").unwrap_err();
    assert_eq!(
        e.kind,
        ErrorKind::BadArg {
            tag: "em".into(),
            arg: "●●".into()
        }
    );
}

#[test]
fn 縦中横は中にタグを書けず_空にできない() {
    let s = parse("第[tcy]12[/tcy]話").unwrap();
    assert_eq!(
        s.items[1],
        Item::Tcy {
            text: "12".into(),
            style: 0,
            source: vec![8..9, 9..10],
        }
    );
    assert_eq!(
        parse("[tcy]1[b]2[/b][/tcy]").unwrap_err().kind,
        ErrorKind::NestedInTcy
    );
    assert_eq!(parse("[tcy][/tcy]").unwrap_err().kind, ErrorKind::EmptyTcy);
    assert_eq!(
        parse("[sup][tcy]1[/tcy][/sup]").unwrap_err().kind,
        ErrorKind::NestedInShift
    );
}

#[test]
fn 引数の数が決まっているタグの余った引数はエラー() {
    for src in [
        "[b x]あ[/b]",
        "[color red blue]あ[/color]",
        "[size 2 3]あ[/size]",
        "[speed 2 3]あ[/speed]",
        "[fade 0.3 1]あ[/fade]",
        "[neko hop 1 2]あ[/neko]",
        "あ[wait 1 2]",
        "あ[voice a b]",
        "あ[se a b]",
        "あ[p x]",
        "あ[br x]",
    ] {
        let e = parse(src).unwrap_err();
        assert!(matches!(e.kind, ErrorKind::BadArg { .. }), "{src}: {e}");
    }
    assert!(parse("[neko hop 1]あ[/neko][wait 1][voice a][event e a b c]").is_ok());
}

#[test]
fn 二重にしていない閉じ括弧はエラー() {
    let e = parse("あ]い").unwrap_err();
    assert_eq!((e.line, e.col, e.kind), (1, 2, ErrorKind::LoneClose(']')));
    let e = parse("あ\nい]").unwrap_err();
    assert_eq!((e.line, e.col, &e.kind), (2, 2, &ErrorKind::LoneClose(']')));
    assert_eq!(e.to_string(), "2:2: 文字の ] は ]] と書く");
    assert!(parse("[ruby よみ]あ]い[/ruby]").is_err());
}

#[test]
fn ルビの親文字と縦中横の中でもエスケープを書ける() {
    let s = parse("[ruby よみ]{a}[[b]][/ruby][tcy]{}[/tcy]").unwrap();
    let Item::Ruby { base, source, .. } = &s.items[0] else {
        panic!()
    };
    assert_eq!(base, "{a}[b]");
    assert_eq!(source[0], 13..14, "{{ は普通の文字で1バイト");
    assert_eq!(source[3], 16..18, "[[ は2バイト分");
    let Item::Tcy { text, .. } = &s.items[1] else {
        panic!()
    };
    assert_eq!(text, "{}");
}

#[test]
fn 改ページの直前の改行1つはページの最後の空行にしない() {
    let s = parse("あ\n[p]\nい").unwrap();
    assert!(matches!(
        s.items.as_slice(),
        [Item::Text { .. }, Item::Page, Item::Text { .. }]
    ));
    // 待機・イベントは飛ばして見る。改行が2つなら1つだけ残る
    let s = parse("あ\n[wait 1][p]い\n\n[p]").unwrap();
    assert!(matches!(
        s.items.as_slice(),
        [
            Item::Text { .. },
            Item::Wait(_),
            Item::Page,
            Item::Text { .. },
            Item::Break,
            Item::Page
        ]
    ));
}

#[test]
fn スタイルは使われたものだけを使われた順に持つ() {
    let s = parse("[b][color red][/color]あ[size 2]い[/size][/b]う").unwrap();
    assert_eq!(s.styles.len(), 3);
    assert!(s.styles[1].bold && s.styles[1].color.is_none());
    assert_eq!(s.styles[2].size, 2.0);
}

#[test]
fn 圏点の余った引数はその引数を誤りとして示す() {
    let e = parse("[em ● x]あ[/em]").unwrap_err();
    assert!(matches!(&e.kind, ErrorKind::BadArg { arg, .. } if arg == "x"));
    let e = parse("[em ab]あ[/em]").unwrap_err();
    assert!(matches!(&e.kind, ErrorKind::BadArg { arg, .. } if arg == "ab"));
}

/// `parse_all` のエラーの(行, 桁, 種類)。
fn all_errors(src: &str) -> Vec<(usize, usize, ErrorKind)> {
    parse_all(src)
        .unwrap_err()
        .into_iter()
        .map(|e| (e.line, e.col, e.kind))
        .collect()
}

#[test]
fn parse_all_はエラーをすべて位置の順に集める() {
    let got = all_errors("[foo]あ[/foo]\n[size x]い[/size]\n]う[var]");
    assert_eq!(
        got,
        vec![
            (1, 1, ErrorKind::UnknownTag("foo".into())),
            (
                2,
                1,
                ErrorKind::BadArg {
                    tag: "size".into(),
                    arg: "x".into()
                }
            ),
            (3, 1, ErrorKind::LoneClose(']')),
            (3, 3, ErrorKind::MissingArg("var".into())),
        ]
    );
}

#[test]
fn parse_all_はエラーが無ければ_parse_と同じ結果() {
    let src = "[ruby ねこ]猫[/ruby]が[var name]と[b]歩く[/b]。[p]次";
    assert_eq!(parse_all(src).unwrap(), parse(src).unwrap());
}

#[test]
fn 閉じていない開き文字の後は行の残りを読み飛ばす() {
    let got = all_errors("あ[b い\n[zzz][var x\nう");
    assert_eq!(
        got,
        vec![
            (1, 2, ErrorKind::UnterminatedTag),
            (2, 1, ErrorKind::UnknownTag("zzz".into())),
            (2, 6, ErrorKind::UnterminatedTag),
        ]
    );
}

#[test]
fn 閉じる順の誤りは一度だけ知らせ_閉じ忘れと重ねない() {
    let got = all_errors("[b][color red]あ[/b][/color]");
    assert_eq!(
        got,
        vec![(
            1,
            16,
            ErrorKind::UnexpectedClose {
                expected: Some("color".into()),
                found: "b".into()
            }
        )]
    );
}

#[test]
fn ルビと縦中横の中のエラーは閉じタグまで読み飛ばす() {
    let got = all_errors("[ruby よ][b]x[/b][/ruby]い[tcy][var n][/tcy][zzz]");
    assert_eq!(
        got,
        vec![
            (1, 9, ErrorKind::NestedInRuby),
            (1, 30, ErrorKind::NestedInTcy),
            (1, 43, ErrorKind::UnknownTag("zzz".into())),
        ]
    );
}

#[test]
fn 閉じ忘れは開いているタグをすべて知らせ_parse_は内側を返す() {
    let got = all_errors("[b][color red]あ");
    assert_eq!(
        got,
        vec![
            (1, 1, ErrorKind::Unclosed("b".into())),
            (1, 4, ErrorKind::Unclosed("color".into())),
        ]
    );
    let first = parse("[b][color red]あ").unwrap_err();
    assert_eq!((first.line, first.col), (1, 4));
}

#[test]
fn エラーの種類は変わらない英語の名前と位置なしのメッセージを持つ() {
    let e = parse("あい[foo]").unwrap_err();
    assert_eq!(e.kind.code(), "unknown_tag");
    assert_eq!(e.kind.to_string(), "知らないタグ [foo]");
    assert_eq!(e.to_string(), "1:3: 知らないタグ [foo]");
    let codes: Vec<&str> = [
        "[ruby]x[/ruby]",
        "[size x]a[/size]",
        "[b]a[/i]",
        "[b]a",
        "[b",
        "[var]",
        "[ruby a]x[b][/b][/ruby]",
        "[ruby a][/ruby]",
        "[sup][sub]a[/sub][/sup]",
        "[tcy][var x][/tcy]",
        "[tcy][/tcy]",
        "あ[align center]い[/align]",
        "]",
    ]
    .iter()
    .map(|s| parse(s).unwrap_err().kind.code())
    .collect();
    assert_eq!(
        codes,
        vec![
            "missing_arg",
            "bad_arg",
            "unexpected_close",
            "unclosed",
            "unterminated_tag",
            "missing_arg",
            "nested_in_ruby",
            "empty_ruby",
            "nested_in_shift",
            "nested_in_tcy",
            "empty_tcy",
            "align_mid_paragraph",
            "lone_close",
        ]
    );
}

#[test]
fn 先頭のbomは文字にせず_位置は元の文章のバイトのまま() {
    let s = parse("\u{FEFF}あい").unwrap();
    assert_eq!(
        s.items,
        vec![Item::Text {
            text: "あい".into(),
            style: 0,
            source: vec![3..6, 6..9],
        }]
    );
    // 桁は BOM を数えない
    let e = parse("\u{FEFF}[foo]").unwrap_err();
    assert_eq!((e.line, e.col), (1, 1));
    // 先頭以外の U+FEFF は今までどおり文字
    let s = parse("あ\u{FEFF}").unwrap();
    assert!(matches!(&s.items[0], Item::Text { text, .. } if text == "あ\u{FEFF}"));
}

#[test]
fn waitは3600秒まで_speedは001倍から() {
    assert!(parse("[wait 3600]").is_ok());
    assert!(parse("[speed 0.01]あ[/speed]").is_ok());
    for src in [
        "[wait 3600.5]",
        "[wait 1e38]",
        "[speed 0.009]あ[/speed]",
        "[speed 1e-30]あ[/speed]",
    ] {
        let e = parse(src).unwrap_err();
        assert!(matches!(e.kind, ErrorKind::BadArg { .. }), "{src}: {e}");
    }
}

#[test]
fn sizeは10まで_縁取りの太さは05まで_アイコンの幅は10まで() {
    assert!(parse("[size 10]あ[/size]").is_ok());
    assert!(parse("[outline red 0.5]あ[/outline]").is_ok());
    assert!(parse("[icon a 10]").is_ok());
    for src in [
        "[size 10.5]あ[/size]",
        "[size 1e6]あ[/size]",
        "[outline 0.6]あ[/outline]",
        "[outline red 100]あ[/outline]",
        "[icon a 10.5]",
    ] {
        let e = parse(src).unwrap_err();
        assert!(matches!(e.kind, ErrorKind::BadArg { .. }), "{src}: {e}");
    }
}

#[test]
fn 揺れ_波_猫の跳ね方の強さは10まで_影のずれは10まで() {
    for src in [
        "[shake 10]あ[/shake]",
        "[wave 10 2]あ[/wave]",
        "[neko hop 10]あ[/neko]",
        "[shadow red 10 -10]あ[/shadow]",
    ] {
        assert!(parse(src).is_ok(), "{src}");
    }
    for src in [
        "[shake 10.5]あ[/shake]",
        "[shake 1e38]あ[/shake]",
        "[wave 1e38 1e-30]あ[/wave]",
        "[neko hop 1e38]あ[/neko]",
        "[shadow red 10.5 0]あ[/shadow]",
        "[shadow red 0 -1e38]あ[/shadow]",
    ] {
        let e = parse(src).unwrap_err();
        assert!(matches!(e.kind, ErrorKind::BadArg { .. }), "{src}: {e}");
    }
}

#[test]
fn ルビの短い書き方は全角と半角の縦線で読める() {
    let full = parse("｜迷宮《めいきゅう》").unwrap();
    let half = parse("|迷宮《めいきゅう》").unwrap();
    for s in [&full, &half] {
        let Item::Ruby {
            base,
            reading,
            source,
            ..
        } = &s.items[0]
        else {
            panic!("ルビになっていない: {:?}", s.items)
        };
        assert_eq!((base.as_str(), reading.as_str()), ("迷宮", "めいきゅう"));
        assert_eq!(source.len(), 2);
    }
    // 親文字の元の位置は縦線の後ろから(`｜` は3バイト)
    let Item::Ruby { source, .. } = &full.items[0] else {
        panic!()
    };
    assert_eq!(source, &vec![3..6, 6..9]);
}

#[test]
fn ルビの短い書き方の前後の文字とスタイルを保つ() {
    let s = parse("あ[b]い|漢字《かんじ》う[/b]え").unwrap();
    let kinds: Vec<&str> = s
        .items
        .iter()
        .map(|it| match it {
            Item::Text { .. } => "text",
            Item::Ruby { .. } => "ruby",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["text", "text", "ruby", "text", "text"]);
    let (Item::Text { style: a, .. }, Item::Ruby { style: b, .. }) = (&s.items[1], &s.items[2])
    else {
        panic!()
    };
    assert_eq!(a, b);
}

#[test]
fn ルビの短い書き方に合わない縦線は文字のまま() {
    for src in [
        "a|b",
        "|《よみ》",
        "|親《》",
        "|親 子《よみ》",
        "|親",
        "|親《よみ",
        "｜",
    ] {
        let s = parse(src).unwrap();
        assert!(
            s.items.iter().all(|it| !matches!(it, Item::Ruby { .. })),
            "ルビになってはいけない: {src}"
        );
    }
    // 縦線そのものは文字として残る
    let Item::Text { text, .. } = &parse("a|b").unwrap().items[0] else {
        panic!()
    };
    assert_eq!(text, "a|b");
}

#[test]
fn 縦線を重ねると前の縦線は文字でその後ろがルビ() {
    let s = parse("||親《よみ》").unwrap();
    assert!(matches!(&s.items[0], Item::Text { text, .. } if text == "|"));
    assert!(matches!(&s.items[1], Item::Ruby { base, .. } if base == "親"));
}

#[test]
fn ルビの短い書き方の親文字は32文字まで() {
    let ok = format!("|{}《よみ》", "あ".repeat(32));
    assert!(matches!(&parse(&ok).unwrap().items[0], Item::Ruby { .. }));
    let ng = format!("|{}《よみ》", "あ".repeat(33));
    assert!(parse(&ng)
        .unwrap()
        .items
        .iter()
        .all(|it| !matches!(it, Item::Ruby { .. })));
}

#[test]
fn ルビの短い書き方の読みにタグは書けず_閉じ括弧はエラーのまま() {
    // 読みに `[` を含む形はルビにならず、`[b]` は通常のタグとして読まれる
    let s = parse("|親《[b]》").unwrap_err();
    assert!(matches!(s.kind, ErrorKind::Unclosed(_)));
}

#[test]
fn 波括弧は普通の文字で_変数は_var_タグで書く() {
    let s = parse("{name}{{}}}").unwrap();
    assert!(matches!(&s.items[..], [Item::Text { text, .. }] if text == "{name}{{}}}"));
    let s = parse("あ[var name]い").unwrap();
    assert!(
        matches!(&s.items[1], Item::Var { name, source, .. } if name == "name" && *source == (3..13))
    );
    assert_eq!(
        parse("[var]").unwrap_err().kind,
        ErrorKind::MissingArg("var".into())
    );
    assert!(matches!(
        parse("[var a b]").unwrap_err().kind,
        ErrorKind::BadArg { .. }
    ));
    assert_eq!(
        parse("[var a").unwrap_err().kind,
        ErrorKind::UnterminatedTag
    );
    // 閉じタグは無い
    assert!(parse("[var a][/var]").is_err());
}

#[test]
fn ルビと縦中横の中に変数は書けない() {
    assert_eq!(
        parse("[ruby よみ][var a][/ruby]").unwrap_err().kind,
        ErrorKind::NestedInRuby
    );
    assert_eq!(
        parse("[tcy][var a][/tcy]").unwrap_err().kind,
        ErrorKind::NestedInTcy
    );
}

#[test]
fn ルビの短い書き方の親文字と読みに波括弧を含められる() {
    let s = parse("｜{鍵}《{かぎ}》").unwrap();
    assert!(matches!(
        &s.items[..],
        [Item::Ruby { base, reading, .. }] if base == "{鍵}" && reading == "{かぎ}"
    ));
}

#[test]
fn 色は色名か_rgb_か_rrggbb_で大文字小文字は区別しない() {
    for c in ["red", "RED", "Silver", "#f00", "#FF00ff", "#00ff7f"] {
        assert!(parse(&format!("[color {c}]あ[/color]")).is_ok(), "{c}");
        assert!(parse(&format!("[outline {c}]あ[/outline]")).is_ok(), "{c}");
        assert!(parse(&format!("[shadow {c}]あ[/shadow]")).is_ok(), "{c}");
    }
}

#[test]
fn 知らない色名と形の違う色はエラーになる() {
    for c in ["なぞ", "#ff", "#ff00", "#ff00ff00", "#gggggg", "rgb"] {
        for tag in ["color", "outline", "shadow"] {
            let e = parse(&format!("[{tag} {c}]あ[/{tag}]")).unwrap_err();
            assert!(matches!(e.kind, ErrorKind::BadArg { .. }), "{tag} {c}");
        }
    }
}

#[test]
fn speed_は百倍まで() {
    assert!(parse("[speed 100]あ[/speed]").is_ok());
    assert!(matches!(
        parse("[speed 100.5]あ[/speed]").unwrap_err().kind,
        ErrorKind::BadArg { .. }
    ));
}

#[test]
fn 揺れ_波_猫の跳ね方_発光の強さは_0_より大きい() {
    for src in [
        "[shake 0]あ[/shake]",
        "[wave 0]あ[/wave]",
        "[neko hop 0]あ[/neko]",
        "[glow 0]あ[/glow]",
        "[glow breathe 0]あ[/glow]",
    ] {
        assert!(
            matches!(parse(src).unwrap_err().kind, ErrorKind::BadArg { .. }),
            "{src}"
        );
    }
    // フェードの秒は 0(すぐ出る)でもよい
    assert!(parse("[fade 0]あ[/fade]").is_ok());
}

#[test]
fn 名前と_id_は六十四文字までで開き括弧を含まない() {
    let ok = "あ".repeat(64);
    let long = "あ".repeat(65);
    for tag in ["voice", "se", "var", "icon", "mark", "blip", "event"] {
        let single = matches!(tag, "voice" | "se" | "var" | "icon");
        let src = |id: &str| match tag {
            "event" => format!("[event {id}]"),
            _ if single => format!("[{tag} {id}]"),
            _ => format!("[{tag} {id}]あ[/{tag}]"),
        };
        assert!(parse(&src(&ok)).is_ok(), "{tag}");
        assert!(parse(&src(&long)).is_err(), "{tag}");
        assert!(parse(&src("a[b")).is_err(), "{tag}");
    }
}

#[test]
fn event_の引数は十六個まで() {
    let args = |n: usize| (0..n).map(|i| format!(" a{i}")).collect::<String>();
    assert!(parse(&format!("[event e{}]", args(16))).is_ok());
    assert!(matches!(
        parse(&format!("[event e{}]", args(17))).unwrap_err().kind,
        ErrorKind::BadArg { .. }
    ));
}

#[test]
fn 開き括弧と閉じタグのスラッシュの直後に空白は書けない() {
    assert_eq!(
        parse("[ b]あ[/b]").unwrap_err().kind,
        ErrorKind::SpaceAfterBracket
    );
    assert_eq!(
        parse("[b]あ[/ b]").unwrap_err().kind,
        ErrorKind::SpaceAfterBracket
    );
    // 名前のあとの空白は書いてよい
    assert!(parse("[b ]あ[/b ]").is_ok());
}

#[test]
fn ruby_の読みは空白を含まない一語() {
    assert!(matches!(
        parse("[ruby よ み]親[/ruby]").unwrap_err().kind,
        ErrorKind::BadArg { .. }
    ));
    // 空白のある読みは短い書き方で書ける
    assert!(parse("｜親《よ み》").is_ok());
}

#[test]
fn align_は入れ子にできない() {
    assert!(matches!(
        parse("[align center][align end]あ[/align][/align]")
            .unwrap_err()
            .kind,
        ErrorKind::BadArg { .. }
    ));
}

#[test]
fn 圏点の記号に制御文字は使えない() {
    assert!(parse("[em \u{7}]あ[/em]").is_err());
    assert!(parse("[em ●]あ[/em]").is_ok());
}
