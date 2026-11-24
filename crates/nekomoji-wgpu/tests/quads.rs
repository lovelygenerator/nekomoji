//! 実フォントを使う結合テスト。フォントが見つからない環境では黙って抜ける。

use nekomoji::{
    parse, Layout, LayoutOptions, Neko, NekoMode, Reveal, RevealOptions, Vars, Writing,
};
use nekomoji_wgpu::{
    atlas::Atlas, build_neko_quads, build_neko_quads_into, build_quads, build_quads_into, Built,
    FontSet, Placement,
};

fn load_font() -> Option<FontSet> {
    let path = std::env::var("NEKOMOJI_FONT")
        .unwrap_or_else(|_| "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into());
    let data = std::fs::read(path).ok()?;
    // ttc の 0 番は中国語向けの並びのことがあるが、字形の有無の確認には十分
    FontSet::from_bytes(&data, 0, 32.0).ok()
}

fn setup(src: &str, font: &FontSet) -> (Layout, Reveal) {
    let script = parse(src).unwrap();
    let layout = Layout::build(&script, font, &LayoutOptions::default(), &Vars::new());
    let reveal = Reveal::new(
        &layout,
        RevealOptions::default()
            .chars_per_sec(10.0)
            .comma_pause(0.0)
            .period_pause(0.0),
    );
    (layout, reveal)
}

#[test]
fn 表示済みの文字だけが四角形になる() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("あいうえお", &font);
    let mut atlas = Atlas::new(1024);
    reveal.advance(0.25); // 3文字
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    assert_eq!(built.quads.len(), 3);
    assert!(!built.overflow);
}

#[test]
fn ルビは親文字が出たときに一緒に出る() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("[ruby めい]迷[/ruby]宮", &font);
    let mut atlas = Atlas::new(1024);
    reveal.advance(0.0); // 1文字目だけ
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    // 迷 + ルビ「め」「い」
    assert_eq!(built.quads.len(), 3);
    // ルビは親文字より上にある
    let ruby_y = built.quads[1].pos[1];
    let base_y = built.quads[0].pos[1];
    assert!(ruby_y < base_y);
}

#[test]
fn 同じ文字はアトラスを使い回す() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("ああああ", &font);
    let mut atlas = Atlas::new(1024);
    reveal.advance(1.0);
    build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    assert_eq!(atlas.len(), 1);
}

#[test]
fn 小さなアトラスでは溢れを知らせる() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("あいうえおかきくけこ", &font);
    let mut atlas = Atlas::new(64);
    reveal.advance(2.0);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    assert!(built.overflow);
}

#[test]
fn 押して落とした文字とルビは回りながら下へ落ち_入力待ちでは肉球を最後に積む() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("あ[ruby めい]迷[/ruby]", &font);
    let neko = Neko::new(
        &layout,
        NekoMode::default()
            .knock_chance(1.0)
            .burst_chance(0.0)
            .punch_chance(0.0)
            .walk_speed(100.0),
    );
    let mut atlas = Atlas::new(1024);
    let place = Placement::default();
    reveal.advance(0.3);
    // 最後の文字のそばに着いてから押す。文字が動き出す直前
    let hit = neko.knock_time(&layout, &reveal, 0);
    reveal.advance(hit - 0.05 - reveal.time());
    assert!(neko.knocked(&layout, &reveal).is_empty());
    let before = build_neko_quads(&layout, &reveal, &neko, &font, &mut atlas, &place);
    reveal.advance(0.5);
    assert_eq!(neko.knocked(&layout, &reveal).len(), 1);
    let after = build_neko_quads(&layout, &reveal, &neko, &font, &mut atlas, &place);
    // あ・迷・め・い・肉球
    assert_eq!(after.quads.len(), 5);
    assert_eq!(after.quads[0].extra[0], 0.0);
    for i in [1, 2, 3] {
        assert!(after.quads[i].extra[0] > 0.0, "回っていない: {i}");
        assert!(
            after.quads[i].pos[1] > before.quads[i].pos[1],
            "落ちていない: {i}"
        );
    }
    // 肉球は最後の文字の右
    let paw = after.quads[4];
    assert!(paw.pos[0] > before.quads[1].pos[0], "{paw:?}");
}

#[test]
fn 猫モードを使わなければ肉球は出ない() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("あい", &font);
    reveal.advance(5.0);
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    assert_eq!(built.quads.len(), 2);
}

fn setup_vertical(src: &str, font: &FontSet) -> (Layout, Reveal) {
    let script = parse(src).unwrap();
    let opts = LayoutOptions::default()
        .writing(Writing::Vertical)
        .max_width(3.0);
    let layout = Layout::build(&script, font, &opts, &Vars::new());
    let mut reveal = Reveal::new(&layout, RevealOptions::default());
    reveal.skip();
    (layout, reveal)
}

fn vertical_place() -> Placement {
    Placement::default().origin([400.0, 20.0])
}

fn center(q: &nekomoji_wgpu::Quad) -> [f32; 2] {
    [q.pos[0] + q.size[0] / 2.0, q.pos[1] + q.size[1] / 2.0]
}

#[test]
fn 縦書きでは行を右から左へ_行の中を上から下へ置く() {
    let Some(font) = load_font() else { return };
    let (layout, reveal) = setup_vertical("あいうえ", &font);
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &vertical_place());
    let c: Vec<_> = built.quads.iter().map(center).collect();
    // あいう が1行目、え が2行目
    assert!(c[1][1] > c[0][1] && c[2][1] > c[1][1], "{c:?}");
    assert!((c[0][0] - c[1][0]).abs() < 2.0, "{c:?}");
    assert!(c[3][0] < c[0][0] - font.base_px, "{c:?}");
    // 1行目は右上(origin)の左にある
    assert!(c[0][0] < 400.0 && c[0][1] > 20.0, "{c:?}");
}

#[test]
fn 縦書きのルビは親文字の右に置く() {
    let Some(font) = load_font() else { return };
    let (layout, reveal) = setup_vertical("[ruby めい]迷[/ruby]", &font);
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &vertical_place());
    assert_eq!(built.quads.len(), 3);
    let base = center(&built.quads[0]);
    let me = center(&built.quads[1]);
    let i = center(&built.quads[2]);
    assert!(me[0] > base[0] + font.base_px / 2.0, "{base:?} {me:?}");
    assert!(i[1] > me[1], "ルビは上から下へ並ぶ: {me:?} {i:?}");
}

#[test]
fn 縦書き用の字形があれば回さずに使い_なければ寝かせる() {
    let Some(font) = load_font() else { return };
    if !font.has_vertical_forms() {
        return;
    }
    let (layout, reveal) = setup_vertical("ー", &font);
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &vertical_place());
    assert_eq!(built.quads[0].extra[0], 0.0);
    // 縦書き用の「ー」は縦長
    assert!(built.quads[0].size[1] > built.quads[0].size[0]);

    let font = font.without_vertical_forms();
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &vertical_place());
    assert!((built.quads[0].extra[0] - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
}

#[test]
fn 置き換えの無い句読点は字の枠の右上へずらす() {
    let Some(font) = load_font() else { return };
    let font = font.without_vertical_forms();
    let (layout, reveal) = setup_vertical("あ、", &font);
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &vertical_place());
    let a = center(&built.quads[0]);
    let ten = center(&built.quads[1]);
    // 「、」の枠は「あ」の下。字面は枠の右上(あの中心より右、枠の上半分)
    assert!(ten[0] > a[0], "{a:?} {ten:?}");
    let box_top = 20.0 + font.base_px;
    assert!(ten[1] < box_top + font.base_px / 2.0, "{ten:?}");
}

#[test]
fn 縦中横は立てたまま横に並べ_全角1文字分の枠に収める() {
    let Some(font) = load_font() else { return };
    let (layout, reveal) = setup_vertical("あ[tcy]12[/tcy]い", &font);
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &vertical_place());
    assert_eq!(built.quads.len(), 4);
    let (a, one, two, i) = (
        center(&built.quads[0]),
        center(&built.quads[1]),
        center(&built.quads[2]),
        center(&built.quads[3]),
    );
    // 1 と 2 は回さず、同じ高さに左右に並ぶ
    assert_eq!(built.quads[1].extra[0], 0.0);
    assert!((one[1] - two[1]).abs() < 2.0, "{one:?} {two:?}");
    assert!(two[0] > one[0], "{one:?} {two:?}");
    // 「あ」と「い」の間の1文字分に入り、列の中に収まる
    assert!(one[1] > a[1] && one[1] < i[1], "{a:?} {one:?} {i:?}");
    let left = a[0] - font.base_px / 2.0;
    assert!(one[0] > left && two[0] < a[0] + font.base_px / 2.0);

    // 3 桁は横に縮める
    let (layout, reveal) = setup_vertical("[tcy]100[/tcy]", &font);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &vertical_place());
    let raster = font.rasterize('1', font.base_px);
    assert!(built.quads[0].size[0] < raster.width as f32);
}

#[test]
fn ダッシュは送り幅いっぱいに引き伸ばして続けて書くと途切れない() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("――", &font);
    reveal.skip();
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    let (a, b) = (&built.quads[0], &built.quads[1]);
    assert!(a.pos[0] + a.size[0] >= b.pos[0], "{a:?} {b:?}");
    let adv = nekomoji::FontMetrics::advance(&font, '―', 1.0) * font.base_px;
    assert!(a.size[0] >= adv);
}

#[test]
fn 圏点は字面の中心を親文字の中心にそろえる() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("[em]あ[/em]", &font);
    reveal.skip();
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    assert_eq!(built.quads.len(), 2);
    let base = center(&built.quads[0]);
    let mark = center(&built.quads[1]);
    assert!((base[0] - mark[0]).abs() < 2.0, "{base:?} {mark:?}");
    assert!(mark[1] < base[1]);
}

#[test]
fn 縦書きの肉球は最後の文字の下に出る() {
    let Some(font) = load_font() else { return };
    let script = parse("あいう").unwrap();
    let opts = LayoutOptions::default().writing(Writing::Vertical);
    let layout = Layout::build(&script, &font, &opts, &Vars::new());
    let mut reveal = Reveal::new(&layout, RevealOptions::default());
    reveal.advance(5.0);
    let neko = Neko::new(&layout, NekoMode::default().walk(false));
    let mut atlas = Atlas::new(1024);
    let built = build_neko_quads(
        &layout,
        &reveal,
        &neko,
        &font,
        &mut atlas,
        &vertical_place(),
    );
    let paw = built.quads.last().unwrap();
    let last = center(&built.quads[2]);
    assert!(paw.pos[1] > last[1], "{paw:?}");
    assert!(
        (paw.pos[0] + paw.size[0] / 2.0 - last[0]).abs() < 4.0,
        "{paw:?}"
    );
}

fn is_line(q: &nekomoji_wgpu::Quad) -> bool {
    q.uv_min == q.uv_max
}

#[test]
fn 上付きは周りの文字より上に_下付きは下に置く() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("x[sup]2[/sup]x[sub]2[/sub]", &font);
    reveal.skip();
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    let bottom = |q: &nekomoji_wgpu::Quad| q.pos[1] + q.size[1];
    let q = &built.quads;
    assert!(q[1].size[1] < q[0].size[1], "上付きは小さい");
    assert!(
        bottom(&q[1]) < bottom(&q[0]) - 5.0,
        "上付きが上がっていない"
    );
    assert!(bottom(&q[3]) > bottom(&q[2]), "下付きが下がっていない");
}

#[test]
fn 取り消し線と下線は表示済みの文字の分だけ四角形で描く() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("[u]あいう[/u][s]え[/s]", &font);
    let mut atlas = Atlas::new(1024);
    reveal.advance(0.15); // 2文字
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    let lines: Vec<_> = built.quads.iter().filter(|q| is_line(q)).collect();
    assert_eq!(lines.len(), 1);
    assert!(
        (lines[0].size[0] - 2.0 * font.base_px).abs() < 1.0,
        "{lines:?}"
    );
    // 下線は文字の下のほう
    let glyph_mid = built.quads[0].pos[1] + built.quads[0].size[1] / 2.0;
    assert!(lines[0].pos[1] > glyph_mid);
    reveal.skip();
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    assert_eq!(built.quads.iter().filter(|q| is_line(q)).count(), 2);
}

#[test]
fn 縦書きの下線は行の右側に縦に引く() {
    let Some(font) = load_font() else { return };
    let (layout, reveal) = setup_vertical("[u]あい[/u]", &font);
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &vertical_place());
    let line = built.quads.iter().find(|q| is_line(q)).unwrap();
    assert!(line.size[1] > line.size[0], "縦長でない: {line:?}");
    let glyph_right = built.quads[0].pos[0] + built.quads[0].size[0];
    assert!(line.pos[0] >= glyph_right, "{line:?}");
}

#[test]
fn 発光する文字は光を文字より先に積み_光は文字より大きく文字の色() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("[color red][glow]あ[/glow][/color]い", &font);
    reveal.advance(0.15);
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    // 外の光・芯の光・あ・い
    assert_eq!(built.quads.len(), 4);
    let (outer, halo, glyph) = (built.quads[0], built.quads[1], built.quads[2]);
    assert!(outer.size[0] > halo.size[0]);
    assert!(halo.size[0] > glyph.size[0] && halo.size[1] > glyph.size[1]);
    // 光と文字の中心は同じ
    let c = |q: nekomoji_wgpu::Quad| [q.pos[0] + q.size[0] / 2.0, q.pos[1] + q.size[1] / 2.0];
    assert!((c(halo)[0] - c(glyph)[0]).abs() < 1.0 && (c(halo)[1] - c(glyph)[1]).abs() < 1.0);
    // 光は文字の色(赤)、文字の芯は白へ寄る
    assert!(halo.color[0] > 0.9 && halo.color[1] < 0.3 && halo.color[3] > 0.3);
    assert!(glyph.color[1] > 0.4);
    // 光り終えると光は描かない
    reveal.advance(3.0);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    assert_eq!(built.quads.len(), 2);
}

fn quad_center(q: nekomoji_wgpu::Quad) -> [f32; 2] {
    [q.pos[0] + q.size[0] / 2.0, q.pos[1] + q.size[1] / 2.0]
}

#[test]
fn 縁取りと影は後ろから_影_光_縁取り_文字の順に積み_文字の不透明度に従う() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup(
        "[outline white 0.1][shadow #000000 0.1 0.2][glow][fade 1]あ[/fade][/glow][/shadow][/outline]",
        &font,
    );
    reveal.advance(0.5);
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    // 影・外の光・芯の光・縁取り・文字
    assert_eq!(built.quads.len(), 5);
    let (shadow, outline, glyph) = (built.quads[0], built.quads[3], built.quads[4]);
    // 縁取りは文字より太らせた分だけ大きく、中心は同じ。色は白
    let grow = 0.1 * 32.0;
    assert!(outline.size[0] >= glyph.size[0] + 2.0 * grow);
    let (co, cg) = (quad_center(outline), quad_center(glyph));
    assert!((co[0] - cg[0]).abs() < 0.5 && (co[1] - cg[1]).abs() < 0.5);
    assert_eq!(&outline.color[..3], &[1.0, 1.0, 1.0]);
    // 影は縁取りの形を右下へずらし、黒
    assert_eq!(shadow.size, outline.size);
    assert!((shadow.pos[0] - outline.pos[0] - 0.1 * 32.0).abs() < 1e-3);
    assert!((shadow.pos[1] - outline.pos[1] - 0.2 * 32.0).abs() < 1e-3);
    assert_eq!(&shadow.color[..3], &[0.0, 0.0, 0.0]);
    // フェードイン中は縁取りも影も文字と同じ不透明度
    assert!(glyph.color[3] < 0.9);
    assert_eq!(outline.color[3], glyph.color[3]);
    assert_eq!(shadow.color[3], glyph.color[3]);
}

#[test]
fn 配置の既定の縁取りと影はタグの無い文字に付き_タグが優先する() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("[outline red 0.2]あ[/outline]い", &font);
    reveal.advance(1.0);
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    // 既定なし: 「あ」の縁取りだけ
    assert_eq!(built.quads.len(), 3);
    let place = Placement::default()
        .outline(Some(nekomoji::Outline::default()))
        .shadow(Some(nekomoji::Shadow::default()));
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
    // 影2・縁取り2・文字2
    assert_eq!(built.quads.len(), 6);
    let (o_a, o_i) = (built.quads[2], built.quads[3]);
    assert!(o_a.color[0] > 0.9 && o_a.color[1] < 0.5, "「あ」はタグの赤");
    assert_eq!(&o_i.color[..3], &[0.0, 0.0, 0.0], "「い」は既定の黒");
    // タグの太さ(0.2)は既定(0.08)より太い
    assert!(o_a.size[0] - built.quads[4].size[0] > o_i.size[0] - built.quads[5].size[0]);
}

#[test]
fn ルビと圏点にも親文字の縁取りが付く() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("[outline][ruby めい]迷[/ruby][em]宮[/em][/outline]", &font);
    reveal.advance(1.0);
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    // 文字2・ルビ2・圏点1 のそれぞれに縁取り
    assert_eq!(built.quads.len(), 10);
}

#[test]
fn 縦書きで寝かせた字と縦中横の縁取りは字と同じ中心と回転() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup_vertical("[outline]A[tcy]12[/tcy]あ[/outline]", &font);
    reveal.advance(1.0);
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &vertical_place());
    let n = built.quads.len() / 2;
    assert_eq!(n, 4);
    for i in 0..n {
        let (o, g) = (built.quads[i], built.quads[n + i]);
        let (co, cg) = (quad_center(o), quad_center(g));
        assert!(
            (co[0] - cg[0]).abs() < 0.5 && (co[1] - cg[1]).abs() < 0.5,
            "{i}"
        );
        assert_eq!(o.extra, g.extra);
        assert!(o.size[0] > g.size[0] && o.size[1] > g.size[1]);
    }
    // 寝かせた「A」は回す
    assert!(built.quads[n].extra[0] > 1.0);
}

/// 字面(四角形)が、組版の字の枠(行の中の向き)からはみ出さないか。
fn ink_inside(quads: &[nekomoji_wgpu::Quad], layout: &Layout, axis: usize, origin: f32) {
    let page = &layout.pages[0];
    for (q, g) in quads.iter().zip(page.glyphs.iter()) {
        let (from, to) = (origin + g.x * 32.0, origin + (g.x + g.width) * 32.0);
        let (a, b) = (q.pos[axis], q.pos[axis] + q.size[axis]);
        assert!(
            a >= from - 2.0 && b <= to + 2.0,
            "{}: 字面 {a:.1}〜{b:.1} が字の枠 {from:.1}〜{to:.1} の外",
            g.ch
        );
    }
}

#[test]
fn 仮名を詰めると送りが狭くなり_字面は詰めた字の枠に収まる() {
    let Some(font) = load_font() else { return };
    let font = font.with_proportional();
    if !font.has_proportional() {
        return;
    }
    let src = "ちょっとトット";
    let full = load_font().unwrap();
    let (wide, _) = setup(src, &full);
    let (layout, mut reveal) = setup(src, &font);
    let width = |l: &Layout| l.pages[0].lines[0].width;
    assert!(width(&layout) < width(&wide) - 0.5, "詰めた行は狭い");
    reveal.advance(10.0);
    let mut atlas = Atlas::new(1024);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    ink_inside(&built.quads, &layout, 0, Placement::default().origin[0]);
}

#[test]
fn 縦書きでも仮名を詰めると送りが狭くなり_字面は詰めた字の枠に収まる() {
    let Some(font) = load_font() else { return };
    let font = font.with_proportional();
    if !font.has_proportional() {
        return;
    }
    let src = "とってもトット";
    let full = load_font().unwrap();
    let (wide, _) = setup_vertical(src, &full);
    let (layout, mut reveal) = setup_vertical(src, &font);
    let width = |l: &Layout| l.pages[0].glyphs.iter().map(|g| g.width).sum::<f32>();
    assert!(width(&layout) < width(&wide) - 0.5, "詰めた行は短い");
    reveal.advance(10.0);
    let mut atlas = Atlas::new(1024);
    let place = vertical_place();
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
    ink_inside(&built.quads, &layout, 1, place.origin[1]);
}

#[test]
fn 詰めても約物と漢字の送りは変わらない() {
    let Some(font) = load_font() else { return };
    let src = "「漢字、。」";
    let (a, _) = setup(src, &font);
    let (b, _) = setup(src, &font.with_proportional());
    let xs = |l: &Layout| l.pages[0].glyphs.iter().map(|g| g.x).collect::<Vec<_>>();
    assert_eq!(xs(&a), xs(&b));
}

#[test]
fn 使い回すbuiltへの四角形は新しく作るものと同じで_前の中身を残さない() {
    let Some(font) = load_font() else { return };
    let src =
        "[shadow][outline][glow]あ[ruby めい]迷[/ruby][/glow][/outline][/shadow][s]宮へ[/s][p]か";
    let mut atlas = Atlas::new(1024);
    let mut built = Built::default();
    for writing in [Writing::Horizontal, Writing::Vertical] {
        let script = parse(src).unwrap();
        let opts = LayoutOptions::default().writing(writing);
        let layout = Layout::build(&script, &font, &opts, &Vars::new());
        let mut reveal = Reveal::new(&layout, RevealOptions::default().chars_per_sec(10.0));
        let place = match writing {
            Writing::Vertical => vertical_place(),
            _ => Placement::default(),
        };
        for _ in 0..15 {
            reveal.advance(0.05);
            build_quads_into(&layout, &reveal, &font, &mut atlas, &place, &mut built);
            let fresh = build_quads(&layout, &reveal, &font, &mut atlas, &place);
            assert_eq!(built.quads, fresh.quads);
            assert_eq!(built.overflow, fresh.overflow);
        }
        reveal.skip();
        build_quads_into(&layout, &reveal, &font, &mut atlas, &place, &mut built);
        let cap = built.quads.capacity();
        // 次のページ(文字が少ない)へ進んでも、前のページの四角形を残さず、領域は使い回す
        reveal.proceed();
        reveal.advance(1.0);
        build_quads_into(&layout, &reveal, &font, &mut atlas, &place, &mut built);
        assert_eq!(
            built.quads,
            build_quads(&layout, &reveal, &font, &mut atlas, &place).quads
        );
        assert_eq!(built.quads.capacity(), cap);
    }
    // 溢れも前のフレームから持ち越さない
    let (layout, mut reveal) = setup("あいうえおかきくけこ", &font);
    reveal.advance(2.0);
    build_quads_into(
        &layout,
        &reveal,
        &font,
        &mut Atlas::new(64),
        &Placement::default(),
        &mut built,
    );
    assert!(built.overflow);
    build_quads_into(
        &layout,
        &reveal,
        &font,
        &mut atlas,
        &Placement::default(),
        &mut built,
    );
    assert!(!built.overflow);
}

#[test]
fn 猫モードでも使い回すbuiltへの四角形は新しく作るものと同じ() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("あ[ruby めい]迷[/ruby]", &font);
    let neko = Neko::new(
        &layout,
        NekoMode::default()
            .knock_chance(1.0)
            .burst_chance(0.0)
            .punch_chance(0.0)
            .walk_speed(100.0),
    );
    let mut atlas = Atlas::new(1024);
    let place = Placement::default();
    let mut built = Built::default();
    let mut knocked = false;
    for _ in 0..60 {
        reveal.advance(0.1);
        knocked |= !neko.knocked(&layout, &reveal).is_empty();
        build_neko_quads_into(
            &layout, &reveal, &neko, &font, &mut atlas, &place, &mut built,
        );
        let fresh = build_neko_quads(&layout, &reveal, &neko, &font, &mut atlas, &place);
        assert_eq!(built.quads, fresh.quads);
    }
    // 文字が飛ばされている間も比べた
    assert!(knocked);
}

/// 一辺 `w`×`h` の単色の RGBA。
fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    rgba.iter()
        .copied()
        .cycle()
        .take((w * h * 4) as usize)
        .collect()
}

#[test]
fn 登録したアイコンは字形ではなく絵の四角形になる() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("[icon ok 2]", &font);
    let mut atlas = Atlas::new(1024);
    atlas
        .icons_mut()
        .insert("ok", 16, 8, &solid(16, 8, [255, 0, 0, 255]))
        .unwrap();
    reveal.advance(1.0);
    let place = Placement::default().origin([10.0, 20.0]);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
    assert_eq!(built.quads.len(), 1);
    assert!(built.missing_icons.is_empty());
    let q = built.quads[0];
    // 種類はアイコン。幅 2 の枠(64 × 32 px)に 2:1 の絵がちょうど収まる
    assert_eq!(q.extra[1], 1.0);
    assert_eq!(q.size, [64.0, 32.0]);
    assert_eq!(q.pos[0], 10.0);
    // 絵のアトラスの位置を指す(余白 1px の内側)
    let tex = atlas.icons().size() as f32;
    assert_eq!(q.uv_min, [1.0 / tex, 1.0 / tex]);
    assert_eq!(q.uv_max, [17.0 / tex, 9.0 / tex]);
    // 色は白(絵のまま)、不透明度は文字と同じ
    assert_eq!(q.color, [1.0, 1.0, 1.0, 1.0]);
}

#[test]
fn アイコンは縦横比を保って字の枠の中央に収める() {
    let Some(font) = load_font() else { return };
    // 幅 2(64 × 32 px の枠)に正方形の絵
    let (layout, mut reveal) = setup("[icon sq 2]", &font);
    let mut atlas = Atlas::new(1024);
    atlas
        .icons_mut()
        .insert("sq", 8, 8, &solid(8, 8, [0, 255, 0, 255]))
        .unwrap();
    reveal.advance(1.0);
    let place = Placement::default().origin([0.0, 0.0]);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
    let q = built.quads[0];
    assert_eq!(q.size, [32.0, 32.0]);
    // 枠の横の中央(64 - 32) / 2
    assert_eq!(q.pos[0], 16.0);
}

#[test]
fn 縦書きのアイコンは行方向が幅_行と直角が字の大きさ() {
    let Some(font) = load_font() else { return };
    let script = parse("[icon ok 2]").unwrap();
    let opts = LayoutOptions::default().vertical();
    let layout = Layout::build(&script, &font, &opts, &Vars::new());
    let mut reveal = Reveal::new(&layout, RevealOptions::default());
    reveal.advance(1.0);
    let mut atlas = Atlas::new(1024);
    // 1:2(縦長)の絵が、32 × 64 px の枠にちょうど収まる
    atlas
        .icons_mut()
        .insert("ok", 8, 16, &solid(8, 16, [0, 0, 255, 255]))
        .unwrap();
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    assert_eq!(built.quads.len(), 1);
    assert_eq!(built.quads[0].size, [32.0, 64.0]);
    assert_eq!(built.quads[0].extra[1], 1.0);
}

#[test]
fn 登録していないアイコンは描かず_idを一度だけ知らせる() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("[icon a][icon a][icon b]あ", &font);
    let mut atlas = Atlas::new(1024);
    reveal.advance(1.0);
    let mut built = Built::default();
    build_quads_into(
        &layout,
        &reveal,
        &font,
        &mut atlas,
        &Placement::default(),
        &mut built,
    );
    // 字形の「あ」だけが四角形になる
    assert_eq!(built.quads.len(), 1);
    assert_eq!(built.quads[0].extra[1], 0.0);
    assert_eq!(built.missing_icons, vec!["a".to_string(), "b".to_string()]);
    // 登録して作り直せば、知らせは消える
    for id in ["a", "b"] {
        atlas
            .icons_mut()
            .insert(id, 4, 4, &solid(4, 4, [255; 4]))
            .unwrap();
    }
    build_quads_into(
        &layout,
        &reveal,
        &font,
        &mut atlas,
        &Placement::default(),
        &mut built,
    );
    assert!(built.missing_icons.is_empty());
    assert_eq!(built.quads.len(), 4);
}

#[test]
fn アイコンの絵は字形のアトラスが作り直されても残る() {
    let mut atlas = Atlas::new(64);
    atlas
        .icons_mut()
        .insert("ok", 4, 4, &solid(4, 4, [255; 4]))
        .unwrap();
    atlas.clear();
    assert!(atlas.make_room(64) || atlas.icons().contains("ok"));
    assert!(atlas.icons().contains("ok"));
}

#[test]
fn 文字送りの途中ではアイコンも出たものだけ描く() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("あ[icon ok]い", &font);
    let mut atlas = Atlas::new(1024);
    atlas
        .icons_mut()
        .insert("ok", 4, 4, &solid(4, 4, [255; 4]))
        .unwrap();
    reveal.advance(0.0); // 1文字目だけ
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    assert_eq!(built.quads.len(), 1);
    reveal.advance(0.1); // アイコンまで
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    assert_eq!(built.quads.len(), 2);
    assert_eq!(built.quads[1].extra[1], 1.0);
}

fn arrow(atlas: &mut Atlas, id: &str) {
    atlas
        .icons_mut()
        .insert(id, 8, 8, &solid(8, 8, [255, 255, 255, 255]))
        .unwrap();
}

#[test]
fn アイコンを決めれば_猫モードでなくても入力待ちのカーソルを最後に積む() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("あい", &font);
    reveal.advance(5.0);
    let mut atlas = Atlas::new(1024);
    arrow(&mut atlas, "next");
    let place = Placement::default().cursor_icon("next");
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
    // あ・い・カーソル
    assert_eq!(built.quads.len(), 3);
    let (last_glyph, cursor) = (built.quads[1], built.quads[2]);
    assert_eq!(cursor.extra[1], 1.0, "アイコンの絵");
    // 一辺は字の高さ(32px)の 0.8 倍で、最後の文字より右
    assert_eq!(cursor.size, [25.6, 25.6]);
    assert!(cursor.pos[0] > last_glyph.pos[0] + 16.0, "{cursor:?}");
    assert!(built.missing_icons.is_empty());
}

#[test]
fn アイコンの大きさは_cursor_size_で変えられる() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("あ", &font);
    reveal.advance(5.0);
    let mut atlas = Atlas::new(1024);
    arrow(&mut atlas, "next");
    let place = Placement::default().cursor_icon("next").cursor_size(0.5);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
    assert_eq!(built.quads.last().unwrap().size, [16.0, 16.0]);
}

#[test]
fn 動きなしではアイコンのカーソルは明滅も上下もしない() {
    let Some(font) = load_font() else { return };
    let script = parse("あ").unwrap();
    let layout = Layout::build(&script, &font, &LayoutOptions::default(), &Vars::new());
    let mut reveal = Reveal::new(
        &layout,
        RevealOptions::default().motion(nekomoji::Motion::Off),
    );
    let mut atlas = Atlas::new(1024);
    arrow(&mut atlas, "next");
    let place = Placement::default().cursor_icon("next");
    let mut ys = Vec::new();
    for _ in 0..3 {
        reveal.advance(2.0);
        let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
        let q = *built.quads.last().unwrap();
        assert_eq!(q.color[3], 0.9);
        ys.push(q.pos[1]);
    }
    assert!(ys.windows(2).all(|w| w[0] == w[1]), "{ys:?}");
}

#[test]
fn 入力待ちの種類ごとにアイコンを変えられ_決めていない種類は出ない() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("あ[l]い", &font);
    let mut atlas = Atlas::new(1024);
    arrow(&mut atlas, "mid");
    // ページの途中(`[l]`)だけアイコン
    let place =
        Placement::default().cursor_icon_for(nekomoji::WaitKind::Click, Some("mid".to_string()));
    reveal.advance(1.0);
    assert_eq!(reveal.wait_kind(), Some(nekomoji::WaitKind::Click));
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
    assert_eq!(built.quads.len(), 2, "あ + アイコン");
    assert_eq!(built.quads[1].extra[1], 1.0);
    // 最後まで出し終えた(End)ときは、決めていないので出ない
    reveal.proceed();
    reveal.advance(5.0);
    assert_eq!(reveal.wait_kind(), Some(nekomoji::WaitKind::End));
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
    assert_eq!(built.quads.len(), 2, "あ・い だけ");
    assert!(built.quads.iter().all(|q| q.extra[1] == 0.0));
}

#[test]
fn 猫モードでもアイコンを決めれば肉球の代わりに描く() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("あい", &font);
    reveal.advance(5.0);
    let neko = Neko::new(&layout, NekoMode::default().knock_chance(0.0));
    let mut atlas = Atlas::new(1024);
    arrow(&mut atlas, "next");
    // アイコンなし: 肉球(字形のカバレッジの四角形)
    let paw = build_neko_quads(
        &layout,
        &reveal,
        &neko,
        &font,
        &mut atlas,
        &Placement::default(),
    );
    assert_eq!(paw.quads.len(), 3);
    assert_eq!(paw.quads[2].extra[1], 0.0);
    // アイコンあり: 最後の 1 つがアイコンで、肉球は積まない
    let place = Placement::default().cursor_icon("next");
    let icon = build_neko_quads(&layout, &reveal, &neko, &font, &mut atlas, &place);
    assert_eq!(icon.quads.len(), 3);
    assert_eq!(icon.quads[2].extra[1], 1.0);
}

#[test]
fn 絵を登録していないカーソルのアイコンは描かず_idを知らせる() {
    let Some(font) = load_font() else { return };
    let (layout, mut reveal) = setup("あい", &font);
    reveal.advance(5.0);
    let mut atlas = Atlas::new(1024);
    let place = Placement::default().cursor_icon("nope");
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
    assert_eq!(built.quads.len(), 2);
    assert_eq!(built.missing_icons, vec!["nope".to_string()]);
}

#[test]
fn 縦書きのアイコンのカーソルは最後の文字の下に出る() {
    let Some(font) = load_font() else { return };
    let (layout, reveal) = setup_vertical("あい", &font);
    let mut atlas = Atlas::new(1024);
    arrow(&mut atlas, "next");
    let place = Placement::default()
        .origin([200.0, 20.0])
        .cursor_icon("next");
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
    assert_eq!(built.quads.len(), 3);
    let (last_glyph, cursor) = (built.quads[1], built.quads[2]);
    assert_eq!(cursor.extra[1], 1.0);
    assert!(cursor.pos[1] > last_glyph.pos[1] + 16.0, "{cursor:?}");
}
