use super::cells::str_cells;
use super::*;
use nekomoji::{parse, Align, Blip, Indent, NekoMode, Reveal, Writing};

#[test]
fn ルビは親文字の中央の上に置く() {
    let s = Settings::default();
    let layout = s.build("あ[ruby ねこ]猫[/ruby]").unwrap();
    let rows = page_rows(&layout, 0, 2);
    assert_eq!(rows, vec!["   |  ねこ", " 1 | あ猫"]);
}

#[test]
fn 表示前のルビは出さず行数は変わらない() {
    let s = Settings::default();
    let layout = s.build("あ[ruby ねこ]猫[/ruby]").unwrap();
    let rows = page_rows(&layout, 0, 1);
    assert_eq!(rows, vec!["   |", " 1 | あ"]);
}

fn tate(width: f32) -> Settings {
    let mut s = Settings::default();
    s.layout.writing = Writing::Vertical;
    s.layout.max_width = width;
    s
}

#[test]
fn 縦書きは行を右から左へ並べ_ルビは親文字の右に置く() {
    let s = tate(3.0);
    let layout = s.build("あ[ruby ねこ]猫[/ruby]がいた").unwrap();
    let rows = page_rows(&layout, 0, 5);
    assert_eq!(rows, vec!["  2  1", " い あ", " た 猫 ね", "    が こ"]);
}

#[test]
fn 縦書きでは句読点と括弧を縦書き用の形で示し_ぶら下げは行の外に出る() {
    let s = tate(2.0);
    let layout = s.build("「あ」い。").unwrap();
    let rows = page_rows(&layout, 0, 5);
    assert_eq!(rows, vec!["  2  1", " い ﹁", " ︒ あ", "    ﹂"]);
}

#[test]
fn 縦書きでは半角英字を1段に2つまで置く() {
    let s = tate(4.0);
    let layout = s.build("HPあ").unwrap();
    assert_eq!(page_rows(&layout, 0, 3), vec!["  1", " HP", " あ"]);
}

#[test]
fn 縦書きでは落とした文字は空白になり_カーソルは最後の文字の下に出る() {
    let mut s = neko_settings();
    s.layout.writing = Writing::Vertical;
    let layout = s.build("あいう").unwrap();
    let neko = s.neko(&layout).unwrap();
    let mut r = Reveal::new(&layout, s.reveal.clone());
    r.skip();
    r.advance(1.0);
    let view = CatView::new(&neko, &layout, &r, false);
    let rows = page_rows_with(&layout, 0, r.visible_count(), &view);
    // 「う」は押して落とされて空白になる
    assert_eq!(rows, vec!["  1", " あ", " い", "", " 🐾"]);
    // 数秒後に戻る
    r.advance(5.0);
    let view = CatView::new(&neko, &layout, &r, false);
    let rows = page_rows_with(&layout, 0, r.visible_count(), &view);
    assert_eq!(rows, vec!["  1", " あ", " い", " う", " 🐾"]);
}

#[test]
fn 上付き下付きは波括弧で示す() {
    let s = Settings::default();
    let layout = s.build("H[sub]2[/sub]O と x[sup]2[/sup]").unwrap();
    let rows = page_rows(&layout, 0, 8);
    assert_eq!(rows, vec![" 1 | H_{2}O と x^{2}"]);
}

#[test]
fn 取り消し線と下線は結合文字で示す() {
    let s = Settings::default();
    let layout = s.build("[s]あ[/s][u]い[/u]").unwrap();
    let rows = page_rows(&layout, 0, 2);
    assert_eq!(rows, vec![" 1 | あ\u{0336}い\u{0332}"]);
    assert_eq!(str_cells(&rows[0]), 9);
}

#[test]
fn 文字送りの音の指定を読む() {
    assert_eq!(parse_blip("off"), Some(None));
    assert_eq!(parse_blip("pi"), Some(Some(Blip::default().id("pi"))));
    assert_eq!(
        parse_blip("pi,3"),
        Some(Some(Blip::default().id("pi").every(3)))
    );
    assert_eq!(parse_blip("pi,0"), None);
    assert_eq!(parse_blip(""), None);
    let mut s = Settings::default();
    s.reveal.blip = Some(Blip::default());
    assert!(s.summary().contains("音 blip 2文字ごと"), "{}", s.summary());
}

#[test]
fn 揃えの指定を読む() {
    assert_eq!(parse_align("center"), Some(Align::Center));
    assert_eq!(parse_align("right"), Some(Align::End));
    assert_eq!(parse_align("middle"), None);
    let mut s = Settings::default();
    s.layout.align = Align::End;
    assert!(s.summary().contains("行末揃え"), "{}", s.summary());
}

#[test]
fn 字下げの指定を読む() {
    let mut i = Indent::default();
    assert!(apply_indent(&mut i, "novel"));
    assert_eq!(i, Indent::novel());
    assert!(apply_bracket_indent(&mut i, "0.5,1"));
    assert_eq!((i.bracket_first, i.bracket_rest), (0.5, 1.0));
    // 2行目以降を省くと今の値のまま
    assert!(apply_indent(&mut i, "2"));
    assert_eq!((i.first, i.rest), (2.0, 0.0));
    assert!(!apply_indent(&mut i, "-1"));
    assert!(!apply_bracket_indent(&mut i, "a,b"));
    assert!(apply_indent(&mut i, "off"));
    assert_eq!(i, Indent::default());
}

#[test]
fn エラーの位置に印を付ける() {
    let src = "あい[foo]";
    let e = parse(src).unwrap_err();
    assert_eq!(
        format_error(src, &e),
        "エラー 1:3: 知らないタグ [foo]\n  あい[foo]\n      ^"
    );
}

#[test]
fn エラーが2件以上なら並べて件数を足す() {
    let src = "[foo]\n]";
    let e = nekomoji::parse_all(src).unwrap_err();
    assert_eq!(
        format_errors(src, &e),
        "エラー 1:1: 知らないタグ [foo]\n  [foo]\n  ^\n\
         エラー 2:1: 文字の ] は ]] と書く\n  ]\n  ^\n(エラー 2 件)"
    );
    assert_eq!(format_errors(src, &e[..1]), format_error(src, &e[0]));
}

fn neko_settings() -> Settings {
    Settings {
        neko: Some(
            NekoMode::default()
                .walk_speed(100.0)
                .knock_chance(1.0)
                .burst_chance(0.0)
                .punch_chance(0.0),
        ),
        ..Settings::default()
    }
}

#[test]
fn 猫パンチで飛んだ文字は空白になり_猫そのものは出さない() {
    let mut s = neko_settings();
    s.neko = Some(
        NekoMode::default()
            .punch_chance(1.0)
            .knock_chance(0.0)
            .burst_chance(0.0),
    );
    let layout = s.build("あいうえおかきくけこ").unwrap();
    let neko = s.neko(&layout).unwrap();
    let mut r = Reveal::new(&layout, s.reveal.clone());
    let mut punched = false;
    for _ in 0..40 {
        r.advance(0.05);
        let view = CatView::new(&neko, &layout, &r, false);
        let rows = page_rows_with(&layout, 0, r.visible_count(), &view);
        // 猫の段は無い
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert!(!rows[0].contains('🐈'));
        if !view.hidden.is_empty() {
            punched = true;
            let shown = rows[0]
                .chars()
                .filter(|c| !c.is_ascii() && *c != '🐾')
                .count();
            assert!(shown < r.visible_count(), "{rows:?}");
        }
    }
    assert!(punched);
}

#[test]
fn 落とした文字は空白になり_入力待ちでは最後の文字の後ろに肉球が出て_数秒後に戻る() {
    let s = neko_settings();
    let layout = s.build("あいう").unwrap();
    let neko = s.neko(&layout).unwrap();
    let mut r = Reveal::new(&layout, s.reveal.clone());
    r.skip();
    r.advance(1.0);
    let view = CatView::new(&neko, &layout, &r, true);
    let rows = page_rows_with(&layout, 0, r.visible_count(), &view);
    assert_eq!(rows, vec![" 1 | あい   =^.^="]);
    r.advance(5.0);
    let view = CatView::new(&neko, &layout, &r, false);
    let rows = page_rows_with(&layout, 0, r.visible_count(), &view);
    assert_eq!(rows, vec![" 1 | あいう 🐾"]);
}

#[test]
fn check_の_json_はファイルごとのエラーを位置と種類の名前つきで出す() {
    let bad = "あい[foo]\n\"q\"\\\t[var]";
    let e = nekomoji::parse_all(bad).unwrap_err();
    let json = check_json(&[("a.neko", bad, &e), ("b.neko", "OK", &[])]);
    assert_eq!(
        json,
        concat!(
            r#"{"ok":false,"error_count":2,"files":["#,
            r#"{"path":"a.neko","ok":false,"errors":["#,
            r#"{"line":1,"col":3,"code":"unknown_tag","message":"知らないタグ [foo]","line_text":"あい[foo]"},"#,
            r#"{"line":2,"col":6,"code":"missing_arg","message":"[var] に引数が必要","line_text":"\"q\"\\\t[var]"}"#,
            r#"]},{"path":"b.neko","ok":true,"errors":[]}]}"#,
        )
    );
    assert_eq!(
        check_json(&[("c.neko", "", &[])]),
        r#"{"ok":true,"error_count":0,"files":[{"path":"c.neko","ok":true,"errors":[]}]}"#
    );
}

#[test]
fn check_の人が読む形は1ファイルなら見出しを付けない() {
    let bad = "[foo]";
    let e = nekomoji::parse_all(bad).unwrap_err();
    assert_eq!(check_text(&[("a", "あ", &[])]), "OK");
    assert_eq!(check_text(&[("a", bad, &e)]), format_errors(bad, &e));
    assert_eq!(
        check_text(&[("a", "あ", &[]), ("b", bad, &e)]),
        format!("== a\nOK\n== b\n{}", format_errors(bad, &e))
    );
}
