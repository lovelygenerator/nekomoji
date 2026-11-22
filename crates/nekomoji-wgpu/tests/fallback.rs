//! フォントの代わりの連鎖。本命は web-demo のサブセット(リポジトリにある)、代わりは
//! OS の大きな日本語フォント。代わりのフォントが見つからない環境では、それが要るテストを飛ばす。

use nekomoji::{parse, FontMetrics, Layout, LayoutOptions, Reveal, RevealOptions, Vars};
use nekomoji_wgpu::{atlas::Atlas, build_quads, FontSet, Placement};

/// demo.neko の文字だけを持つ Zen Maru Gothic のサブセット
fn subset() -> Vec<u8> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/web-demo/font.ttf"
    );
    std::fs::read(path).expect("web-demo のフォント")
}

/// 大きな日本語フォント(無ければ None)
fn large() -> Option<Vec<u8>> {
    let path = std::env::var("NEKOMOJI_FONT")
        .unwrap_or_else(|_| "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into());
    let data = std::fs::read(&path).ok();
    if data.is_none() {
        eprintln!("{path} が無いので飛ばす");
    }
    data
}

/// サブセットに無く、大きなフォントにはある文字
const NOT_IN_SUBSET: char = '鬱';

#[test]
fn 本命だけなら連鎖は1つで_本命の字形を使う() {
    let font = FontSet::from_bytes(&subset(), 0, 32.0).unwrap();
    assert_eq!(font.font_count(), 1);
    assert_eq!(font.glyph('迷').unwrap().font, 0);
    assert!(font.glyph(NOT_IN_SUBSET).is_none());
    assert!(!font.has_glyph(NOT_IN_SUBSET));
}

#[test]
fn 本命にある文字は代わりのフォントがあっても本命で描く() {
    let data = subset();
    // 同じフォントを代わりに足しても、先に見つかる本命が使われる
    let font = FontSet::from_bytes(&data, 0, 32.0)
        .unwrap()
        .with_fallback(&data, 0)
        .unwrap();
    assert_eq!(font.font_count(), 2);
    for c in "迷宮ミケ".chars() {
        assert_eq!(font.glyph(c).unwrap().font, 0, "{c}");
    }
}

#[test]
fn 代わりのフォントの読み込みに失敗したらエラー() {
    let font = FontSet::from_bytes(&subset(), 0, 32.0).unwrap();
    assert!(font.with_fallback(&[0, 1, 2, 3], 0).is_err());
}

#[test]
fn 本命に無い文字は代わりのフォントで描き_送り幅もそのフォントで測る() {
    let Some(big) = large() else { return };
    let font = FontSet::from_bytes(&subset(), 0, 32.0)
        .unwrap()
        .with_fallback(&big, 0)
        .unwrap();
    let g = font.glyph(NOT_IN_SUBSET).expect("代わりのフォントにある");
    assert_eq!(g.font, 1);
    assert!(font.has_glyph(NOT_IN_SUBSET));
    let r = font.rasterize(NOT_IN_SUBSET, 32.0);
    assert!(r.width > 0 && r.bitmap.iter().any(|&v| v > 128), "描ける");
    // 漢字は全角
    let adv = font.advance(NOT_IN_SUBSET, 1.0);
    assert!((adv - 1.0).abs() < 0.05, "{adv}");
    // 本命の .notdef の送り幅ではない(本命だけのときと比べる)
    let alone = FontSet::from_bytes(&subset(), 0, 32.0).unwrap();
    let notdef = alone.rasterize(NOT_IN_SUBSET, 32.0);
    assert_ne!(notdef.bitmap, r.bitmap, "豆腐ではない");
}

#[test]
fn どのフォントにも無い文字は_本命の_notdef_で描く() {
    let Some(big) = large() else { return };
    let font = FontSet::from_bytes(&subset(), 0, 32.0)
        .unwrap()
        .with_fallback(&big, 0)
        .unwrap();
    // 私用領域
    assert!(font.glyph('\u{E000}').is_none());
    let alone = FontSet::from_bytes(&subset(), 0, 32.0).unwrap();
    assert_eq!(
        font.rasterize('\u{E000}', 32.0).bitmap,
        alone.rasterize('\u{E000}', 32.0).bitmap
    );
}

#[test]
fn 文字そのものを持つ代わりのフォントは_本命の代わりの文字より先に使う() {
    let Some(big) = large() else { return };
    let alone = FontSet::from_bytes(&subset(), 0, 32.0).unwrap();
    // Zen Maru Gothic には ― が無く、本命だけなら代わりの文字(—)で描く
    assert!(!alone.has_glyph('―'));
    assert_eq!(alone.glyph('―').map(|g| g.font), Some(0));
    let font = FontSet::from_bytes(&subset(), 0, 32.0)
        .unwrap()
        .with_fallback(&big, 0)
        .unwrap();
    assert_eq!(font.glyph('―').map(|g| g.font), Some(1));
}

#[test]
fn 縦書き用の字形は文字を描くフォントのものを使う() {
    let Some(big) = large() else { return };
    let font = FontSet::from_bytes(&subset(), 0, 32.0)
        .unwrap()
        .with_fallback(&big, 0)
        .unwrap();
    // 「〔」はサブセットに無く、Noto Sans CJK は縦書き用の字形を持つ
    assert!(!FontSet::from_bytes(&subset(), 0, 32.0)
        .unwrap()
        .has_glyph('〔'));
    let v = font.vertical_glyph('〔').expect("縦書き用の字形");
    assert_eq!(v.font, 1);
    assert_ne!(v, font.glyph('〔').unwrap());
    assert!(font.rasterize_vertical('〔', 32.0).is_some());
}

#[test]
fn 変数の値がサブセットに無くても代わりのフォントで四角形になる() {
    let Some(big) = large() else { return };
    let font = FontSet::from_bytes(&subset(), 0, 32.0)
        .unwrap()
        .with_fallback(&big, 0)
        .unwrap();
    let mut vars = Vars::new();
    vars.insert("player_name".into(), "鬱金".into());
    let script = parse("[var player_name]が来た").unwrap();
    let layout = Layout::build(&script, &font, &LayoutOptions::default(), &vars);
    let glyphs = &layout.pages[0].glyphs;
    assert!(
        glyphs.iter().all(|g| font.glyph(g.ch).is_some()),
        "豆腐が無い"
    );
    assert_eq!(font.glyph('鬱').unwrap().font, 1);
    let mut reveal = Reveal::new(&layout, RevealOptions::default());
    reveal.skip();
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    assert!(!built.overflow);
    // 鬱・金・が・来・た の5文字すべてが描ける(空白の字形にならない)
    assert_eq!(built.quads.len(), 5, "{:?}", built.quads.len());
    assert!(built
        .quads
        .iter()
        .all(|q| q.size[0] > 1.0 && q.size[1] > 1.0));
}
