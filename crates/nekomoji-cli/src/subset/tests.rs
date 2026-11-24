use super::*;

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn 引数を読む() {
    let a = Args::parse(&args(&[
        "f.ttc", "a.neko", "b.neko", "-o", "out.ttf", "--index", "2", "--chars", "猫", "--set",
        "n=ミケ",
    ]))
    .unwrap();
    assert_eq!(a.font, "f.ttc");
    assert_eq!(a.scenarios, ["a.neko", "b.neko"]);
    assert_eq!(a.output, "out.ttf");
    assert_eq!(a.index, 2);
    assert_eq!(a.extra_chars, "猫");
    assert_eq!(a.settings.vars["n"], "ミケ");
    assert!(a.basic_chars);
}

#[test]
fn 出力先がないとエラー() {
    let e = Args::parse(&args(&["f.ttf", "a.neko"])).unwrap_err();
    assert!(e.contains("-o"));
}

#[test]
fn シナリオも_chars_もないとエラー() {
    assert!(Args::parse(&args(&["f.ttf", "-o", "o.ttf"])).is_err());
}

#[test]
fn 知らないオプションはエラー() {
    assert!(Args::parse(&args(&["f.ttf", "a.neko", "-o", "o", "--zzz"])).is_err());
}

#[test]
fn 本文とルビの文字を集める() {
    let mut s = Settings::default();
    s.vars.insert("x".into(), "ミ".into());
    let layout = s
        .build("[ruby めい]迷[/ruby]宮[br][var x][color red]赤[/color]")
        .unwrap();
    let set = layout_chars(&layout);
    for c in "めい迷宮赤ミ".chars() {
        assert!(set.contains(&c), "{c} が無い");
    }
    // タグの名前は含まない
    assert!(!set.contains(&'r'), "色名 red・変数のタグが混ざっている");
    assert!(!set.contains(&'['));
}

#[test]
fn アイコンの代わりの文字は集めない() {
    let layout = Settings::default().build("[icon ok]決定").unwrap();
    let set = layout_chars(&layout);
    assert!(
        !set.contains(&nekomoji::ICON_CHAR),
        "字形を描かない文字が混ざっている"
    );
    assert!(set.contains(&'決') && set.contains(&'定'));
}

#[test]
fn 変数の値があればその文字を集める() {
    let mut s = Settings::default();
    s.vars.insert("n".into(), "ミケ".into());
    let layout = s.build("[var n]").unwrap();
    let set = layout_chars(&layout);
    assert!(set.contains(&'ミ') && set.contains(&'ケ'));
    assert!(layout.missing_vars.is_empty());
}

fn report(vars: &[&str], missing_vars: &[&str]) -> Report {
    Report {
        wanted: 3,
        missing_in_font: vec![],
        substituted: vec![],
        missing_vars: missing_vars.iter().map(|s| s.to_string()).collect(),
        vars: vars.iter().map(|s| s.to_string()).collect(),
        input_bytes: 100,
        output_bytes: 10,
    }
}

#[test]
fn 変数を使うシナリオでは代わりのフォントを足すよう知らせる() {
    let a = Args::parse(&args(&["f.ttf", "a.neko", "-o", "o.ttf"])).unwrap();
    let s = format_report(&a, &report(&["player_name"], &[]));
    assert!(s.contains("[var player_name]"), "{s}");
    assert!(s.contains("with_fallback"), "{s}");
    // 値を --set で渡していても知らせる(実行時の値は分からない)
    assert!(!s.contains("値が無い変数"), "{s}");
}

#[test]
fn 変数を使わないシナリオでは代わりのフォントの注意を出さない() {
    let a = Args::parse(&args(&["f.ttf", "a.neko", "-o", "o.ttf"])).unwrap();
    let s = format_report(&a, &report(&[], &[]));
    assert!(!s.contains("with_fallback"), "{s}");
}

#[test]
fn 実行すると出てくる変数を値の有無によらず集める() {
    let Ok(font) = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/web-demo/font.ttf"
    )) else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("nekomoji-subset-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (fp, sp, op) = (dir.join("f.ttf"), dir.join("a.neko"), dir.join("o.ttf"));
    std::fs::write(&fp, font).unwrap();
    std::fs::write(&sp, "[var b]と[var a]と[var b]").unwrap();
    let mut a = Args::parse(&args(&[
        fp.to_str().unwrap(),
        sp.to_str().unwrap(),
        "-o",
        op.to_str().unwrap(),
    ]))
    .unwrap();
    a.settings.vars.insert("a".into(), "ミケ".into());
    let r = execute(&a).unwrap();
    assert_eq!(r.vars, ["a", "b"]);
    assert_eq!(r.missing_vars, ["b"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ttc_でないフォントは番号0ならそのまま返す() {
    let data = b"\x00\x01\x00\x00dummy";
    assert_eq!(extract_font(data, 0).unwrap(), data);
    assert!(extract_font(data, 1).is_err());
}

#[test]
fn ttc_の範囲外の番号はエラー() {
    // ttcf, version 1.0, numFonts 1
    let mut d = b"ttcf".to_vec();
    d.extend([0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 16]);
    let e = extract_font(&d, 5).unwrap_err();
    assert!(e.contains("範囲外"));
}

const NOTO: &str = "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc";
// TrueType 形式の日本語フォント(IPA ゴシック)。無い環境ではテストを省く
const IPA: &str = "/usr/share/fonts/opentype/ipafont-gothic/ipag.ttf";

#[test]
fn cff_形式のフォントも必要な文字だけ残る() {
    let Ok(data) = std::fs::read(NOTO) else {
        return;
    };
    let single = extract_font(&data, 0).unwrap();
    assert_eq!(&single[0..4], b"OTTO");
    let chars: BTreeSet<char> = "あ迷宮、。aA".chars().collect();
    let (out, missing) = subset_font(&single, &chars).unwrap();
    assert!(missing.is_empty());
    assert!(out.len() < 100_000, "サイズが大きい: {}", out.len());
    let font = fontdue::Font::from_bytes(&out[..], fontdue::FontSettings::default()).unwrap();
    for c in chars {
        assert_ne!(font.lookup_glyph_index(c), 0, "{c} の字形が無い");
    }
    assert_eq!(font.lookup_glyph_index('漢'), 0, "不要な字形が残っている");
    // 描ける(輪郭が空でない)
    let (m, bitmap) = font.rasterize('迷', 32.0);
    assert!(m.width > 0 && bitmap.iter().any(|&v| v > 128));
}

#[test]
fn cff_形式でもフォントに無い文字は報告される() {
    let Ok(data) = std::fs::read(NOTO) else {
        return;
    };
    let single = extract_font(&data, 0).unwrap();
    let chars: BTreeSet<char> = "あ\u{E000}".chars().collect();
    let (_, missing) = subset_font(&single, &chars).unwrap();
    assert_eq!(missing, vec!['\u{E000}']);
}

#[test]
fn 実フォントで必要な文字だけ残る() {
    let Ok(data) = std::fs::read(IPA) else {
        return;
    };
    let single = extract_font(&data, 0).unwrap();
    let chars: BTreeSet<char> = "あ迷宮、。a".chars().collect();
    let (out, missing) = subset_font(&single, &chars).unwrap();
    assert!(missing.is_empty());
    assert!(out.len() < 100_000, "サイズが大きい: {}", out.len());
    let font = fontdue::Font::from_bytes(&out[..], fontdue::FontSettings::default()).unwrap();
    for c in chars {
        assert_ne!(font.lookup_glyph_index(c), 0, "{c} の字形が無い");
    }
    assert_eq!(font.lookup_glyph_index('漢'), 0, "不要な字形が残っている");
}

#[test]
fn フォントに無い文字は報告される() {
    let Ok(data) = std::fs::read(IPA) else {
        return;
    };
    let single = extract_font(&data, 0).unwrap();
    // 私用領域の文字は IPA ゴシックに無い
    let chars: BTreeSet<char> = "あ\u{E000}".chars().collect();
    let (_, missing) = subset_font(&single, &chars).unwrap();
    assert_eq!(missing, vec!['\u{E000}']);
}
