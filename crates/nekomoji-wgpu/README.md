# nekomoji-wgpu

[![crates.io](https://img.shields.io/crates/v/nekomoji-wgpu.svg)](https://crates.io/crates/nekomoji-wgpu)
[![docs.rs](https://docs.rs/nekomoji-wgpu/badge.svg)](https://docs.rs/nekomoji-wgpu)

*A wgpu rendering adapter for [nekomoji](https://crates.io/crates/nekomoji), a Japanese text layout and reveal-effects library for games. The documentation is in Japanese; the API docs are on docs.rs.*

[nekomoji](https://crates.io/crates/nekomoji)(日本語テキストの配置と文字送りを計算するライブラリ)の結果を、wgpu で画面に描くためのアダプタです。

コアは「どの文字をどこに、どの色・大きさ・動きで置くか」だけを計算します。このアダプタは、それを動的なグリフのアトラスとインスタンス描画で画面に出します。フォントは同梱しません。お使いの日本語フォントを渡してください。

## 全体の流れ

```text
台本(タグ付きの文章)
   │  nekomoji      parse → Layout::build → Reveal   … 何をどこに・いつ出すか(計算だけ)
   ▼
Layout と Reveal
   │  nekomoji-wgpu  build_quads_into                … 画面の四角形にする(GPU に触らない)
   ▼
Vec<Quad>
   │  nekomoji-wgpu  Renderer::prepare → draw        … GPU へ送って描く
   ▼
画面(あなたが開いた wgpu の描画パスの中)
```

| 仕事 | 誰がやるか |
|---|---|
| wgpu の初期化・ウィンドウ・描画パスを開く/閉じる・画面に出す | あなた(お使いのエンジン) |
| 窓の板・背景・立ち絵を描く | あなた |
| 入力(クリック・キー)を受ける | あなた |
| ボイス・効果音の再生、場面の進行 | あなた(`TextEvent` で知らせます) |
| 文字の配置・禁則・ルビ・文字送り・エフェクト | nekomoji |
| グリフのラスタライズ・アトラス・四角形・描画 | nekomoji-wgpu |

## 機能

- **動的なグリフのアトラス**: 画面に出てきた文字だけを置きます。満杯になったら大きくして置き直します(GPU に依存しない部分は単体でテストできます)
- **フォントの連鎖**: 本命のフォントに無い字形を、足した順に代わりのフォントで探します。縦書き用の字形(GSUB の `vert` / `vrt2`)と仮名のプロポーショナル詰め(GPOS の `palt` / `vpal`)に対応します
- **ルビ・圏点・縦書き・縦中横・上付き/下付き・取り消し線/下線**
- **縁取り・影・発光**、揺れ・波・フェード・回転・不透明度
- **文中のアイコン `[icon]`**: RGBA の絵を `Atlas::icons_mut()` に登録すると、字の枠に収めて描きます(色・縁取り・影・発光は効きません)。未登録の ID は何も描かず、`Built::missing_icons` で知らせます
- **入力待ちのカーソル**: 登録したアイコンで描けます(`Placement::cursor_icon`。種類ごとに変えることも)。猫モードでは肉球も描けます
- **猫モード**: 飛ばされた文字と、入力待ちの肉球のカーソル
- **窓の補助**: 画面の矩形から、幅・行数・起点・シザーを作ります(`TextWindow`)
- **確認用の重ね描き**: 行・字の枠・ルビ・カーソル・猫の位置(`build_overlay`)

## 使い方の手順

次の 7 つの手順で組みます。手順ごとに、いつ・何をするかが決まっています。

| 手順 | いつ | やること |
|---|---|---|
| 1 | 開発の最初 | 依存を足し、日本語フォントを用意する |
| 2 | 起動のとき(1 回) | フォント・アトラス・描画器を作る |
| 3 | 起動のとき(1 回) | 窓(メッセージ枠)の位置と大きさを決める |
| 4 | 場面を切り替えるとき | 台本を解析して配置し、文字送りを始める |
| 5 | 毎フレーム | 時間を進め、四角形にして描く |
| 6 | 入力があったとき | クリックで進める・飛ばす |
| 7 | 画面の大きさが変わったとき | 窓と配置を作り直す |

窓を開いて動かす全体の例は、リポジトリの
[`examples/minimal.rs`](https://github.com/lovelygenerator/nekomoji/blob/main/crates/nekomoji-wgpu/examples/minimal.rs)
(winit で窓を開き、手順 1〜7 の順に書いた短い例)にあります。

```sh
cargo run -p nekomoji-wgpu --example minimal -- <日本語フォント>
```

以下の断片は、`Game` という 1 つの構造体にまとめたものです。つなげた全体は、リポジトリの
[`tests/guide.rs`](https://github.com/lovelygenerator/nekomoji/blob/main/crates/nekomoji-wgpu/tests/guide.rs)
にあり、画面外のテクスチャへ描く確認つきで動かしています。

### 手順 1: 準備

```toml
[dependencies]
nekomoji = "0.1"
nekomoji-wgpu = "0.1"
wgpu = "30"
```

`nekomoji-wgpu` は wgpu 30 を使います。お使いの wgpu の版を合わせてください(Rust 1.87 以上)。

フォント(`.ttf`・`.otf`・`.ttc`)は、バイト列として読み込んで渡します。同梱はしません。ゲームに入れるときは、使う文字だけのサブセットにすると小さくなります(リポジトリの CLI に `nekomoji subset` があります)。

### 手順 2: 起動のとき(フォント・アトラス・描画器)

```rust,ignore
// 字の大きさ(px)。フォントはこの大きさで作り、窓にも同じ値を渡す
let px = 32.0;
let font = FontSet::from_bytes(font_bytes, 0, px)?; // 0 は .ttc の中の番号
let atlas = Atlas::new(DEFAULT_ATLAS_SIZE);
let renderer = Renderer::new(device, format, DEFAULT_ATLAS_SIZE);
```

- `format` は描画先(サーフェス)の形式です。サーフェスの設定は `surface_config(&surface, &adapter, [幅, 高さ])` が作ります。色が明るくならない形式を選ぶので、そのまま `surface.configure(&device, &config)` に渡し、`config.format` を `Renderer::new` に渡してください。
- 本命のフォントに無い文字(絵文字・記号など)を描くなら、`font.with_fallback(代わりのフォントのバイト列, 0)?` を足した順に探します。
- 仮名を詰めたいときは `font.with_proportional()` を足します(フォントに `palt` / `vpal` があるとき)。
- アトラスの大きさ(`DEFAULT_ATLAS_SIZE` = 1024)は最初の値です。足りなくなれば、手順 5 のとおり大きくします。

### 手順 3: 窓を決める

```rust,ignore
// 画面の下に、高さ 200px のメッセージ窓。文字は 32px、余白は 16px
let window = TextWindow::bottom([1280.0, 720.0], 200.0, 24.0, font.base_px).padding(16.0);
let opts = window.layout_options(LayoutOptions::default()); // 1 行の幅・行数・書字方向
let place = window.placement(Placement::default());          // 本文の起点・行の間隔
```

`TextWindow` は画面の矩形(px、左上が原点)から、組版の設定(`opts`)と描画の設定(`place`)をそろえて作ります。「何文字ぶんの幅か」と「画面の何 px か」を別々に計算しなくて済みます。

| やりたいこと | 書き方 |
|---|---|
| 画面の下にメッセージ窓 | `TextWindow::bottom(画面, 高さ, 余白, 字の大きさ)` |
| 画面の中央に | `TextWindow::centered(画面, [幅, 高さ], 字の大きさ)` |
| 好きな矩形に | `TextWindow::new([左, 上, 幅, 高さ], 字の大きさ)` |
| 余白 | `.padding(16.0)`(四方)・`.padding_each([上, 右, 下, 左])` |
| 縦書き | `.vertical()`(起点が右上になる) |
| 行の間隔 | `.line_pitch(1.8)`(ルビの分を含めた倍率) |
| 縁取り・影・色 | `Placement::default().outline(Some(Outline::default()))` などを `placement(...)` に渡す |

窓の板や背景の絵そのものは描きません。お使いのエンジンで、`window.rect` に合わせて描いてください。文字に合わせた枠の大きさが要るときは `Placement::extent_rect` を使います。

### 手順 4: 場面を切り替える

```rust,ignore
let script = parse("[ruby めいきゅう]迷宮[/ruby]の奥で、[var name]は振り返った。[l]何かが動いた。[p]")?;
let mut vars = Vars::new();
vars.insert("name".into(), "ミケ".into());

let layout = Layout::build(&script, &font, &opts, &vars);
let reveal = Reveal::new(&layout, RevealOptions::default());
```

台本の書き方(タグ)は、[nekomoji のマークアップ](https://github.com/lovelygenerator/nekomoji/blob/main/docs/markup.md)を見てください。`parse` のエラーは行と桁を持ちます。場面が変わるたびに、解析と配置をやり直し、`Reveal` を作り直します(変数の値だけ変えるときも `Layout::build` からやり直します)。

文字送りの速さ・句読点の間などは `RevealOptions::default().chars_per_sec(30.0)` のように builder で決めます。

### 手順 5: 毎フレーム

時間を進め、出てきた出来事をゲーム側で処理します。

```rust,ignore
for event in reveal.advance(dt) {
    match event {
        TextEvent::Voice(id) => { /* ボイスを鳴らす */ }
        TextEvent::Se(id) => { /* 効果音を鳴らす */ }
        TextEvent::Blip(id) => { /* 文字送りの音を鳴らす */ }
        TextEvent::Custom { name, args } => { /* 台本の [event ...] */ }
        _ => {}
    }
}
```

次に、四角形にして描きます。`Built` は毎フレーム使い回します。

```rust,ignore
// 四角形にする。アトラスが満杯なら大きくして(または空にして)組み立て直す
for _ in 0..2 {
    build_quads_into(&layout, &reveal, &font, &mut atlas, &place, &mut built);
    if !built.overflow {
        break;
    }
    atlas.make_room(Renderer::max_atlas_size(device));
}
// GPU へ送って、開いてある描画パスに描く
renderer.prepare(device, queue, &mut atlas, &built.quads, [画面の幅, 画面の高さ]);
window.apply_scissor(&mut pass, [幅, 高さ]); // 窓の外へはみ出す字を切る(省いてもよい)
renderer.draw(&mut pass);
```

- 描画パス(`wgpu::RenderPass`)は、あなたが開きます。背景や窓の板を先に描き、そのあとにこの 2 行を呼ぶと、文字が上に重なります。
- `renderer.draw` は 1 回の描画呼び出しです。アイコンも文字も、順序・回転・不透明度がそろいます。

### 手順 6: 入力

```rust,ignore
// クリック(決定ボタン)のとき
if reveal.is_waiting() {
    reveal.proceed(); // 入力待ちなら次へ。`[l]` なら同じページの続き、`[p]` なら次のページ
} else {
    reveal.skip();    // 文字送りの途中なら、残りを一度に出す
}
```

何の入力待ちかは `reveal.wait_kind()`(`Click`・`Page`・`End`)で分かります。オートモードの待ち時間の目安は `reveal.auto_wait(ページ, |ボイスの ID| ボイスの長さ)` で得られます(進めるのはあなたです)。

### 手順 7: 画面の大きさが変わったとき

窓と、組版・描画の設定を作り直し、場面を配置し直します。

```rust,ignore
let window = TextWindow::bottom([新しい幅, 新しい高さ], 200.0, 24.0, font.base_px).padding(16.0);
let opts = window.layout_options(LayoutOptions::default());
let place = window.placement(Placement::default());
let state = reveal.state();                       // 進み具合を控える
let layout = Layout::build(&script, &font, &opts, &vars);
let mut reveal = Reveal::new(&layout, RevealOptions::default());
reveal.restore(state);                            // 同じところから続ける
```

高解像度の画面(`scale_factor` が 1 でない)では、窓の矩形・余白・字の大きさに倍率を掛け(`window.scaled(scale)` が使えます)、フォントも `base_px * scale` で作り直します。

## もう少し進んだ使い方

### アイコン(`[icon]`)

台本に `[icon heart]` と書くと、字の枠にアイコンの場所が空きます。絵は、RGBA のバイト列で登録します(PNG などはお好きな方法で RGBA に直してください)。

```rust,ignore
atlas.icons_mut().insert("heart", 幅, 高さ, &rgba)?; // rgba は 幅×高さ×4 バイト
```

- 登録していない ID は何も描かれず、`built.missing_icons` に ID が入ります。
- 絵は字の枠に、縦横比を保って収まります。`[color]`・縁取り・影・発光は効きません(絵のまま)。透明度と回転・文字送りには従います。

### 入力待ちのカーソルをアイコンにする

```rust,ignore
let place = place.cursor_icon("next"); // 絵 "next" を登録しておく
// ページの途中・ページの終わり・終了で変えるなら
let place = place.cursor_icon_for(WaitKind::Click, Some("mid".to_string()));
```

大きさは `cursor_size`(字の高さの比、既定 0.8)で変えます。決めなければ、通常は何も描かず、猫モードでは肉球を描きます。

### クリックした文字を調べる

```rust,ignore
let [x, line] = place.to_logical(&layout, reveal.page(), font.base_px, [マウスの x, マウスの y]);
if let Some(i) = layout.pages[reveal.page()].hit_test(x, line) {
    let glyph = &layout.pages[reveal.page()].glyphs[i];
    let marks = layout.marks_of(glyph); // 台本の [mark id] で付けた名前
}
```

台本に `[mark item]剣[/mark]` と書いておくと、クリックされた語の名前が分かります。

### 猫モード

```rust,ignore
let neko = Neko::new(&layout, NekoMode::default());
build_neko_quads_into(&layout, &reveal, &neko, &font, &mut atlas, &place, &mut built);
```

`build_quads_into` の代わりに呼びます。見えない猫が文字をはじき、入力待ちでは肉球のカーソルが出ます(文字は戻ってきます。テキストは変わりません)。動きの強さは `reveal.set_motion(Motion::Reduced)` で弱め、`Motion::Off` で止められます。光や動きに弱い人のために、設定画面から選べるようにしておくことをおすすめします。

### セーブとロード

`reveal.state()` が返す `RevealState` は、`to_string()` で文字列に、`parse()` で元に戻せます。ページと時刻、クリック待ちから進んだ時刻まで戻るので、見た目まで同じ状態から再開できます。

## うまくいかないとき

| 症状 | 見るところ |
|---|---|
| 何も出ない | 描画パスの中で `renderer.draw` を呼んでいるか。`built.quads` が空でないか。`reveal.advance` で時間を進めているか |
| 一部の文字が出ない | フォントにその字形があるか。`with_fallback` で代わりのフォントを足す。`built.overflow` が真ならアトラスが満杯(手順 5 の `make_room`) |
| 色が明るく薄い | サーフェスの形式が sRGB になっている。`surface_config` か `surface_format` で選ぶ |
| アイコンが出ない | `built.missing_icons` に ID が入っていないか。絵を `atlas.icons_mut()` に登録したか |
| 文字が窓からはみ出す | `window.layout_options(...)` を `Layout::build` に渡したか。字の大きさ(`px`)がフォントの `base_px` と同じか |
| 縦書きの位置が合わない | `TextWindow` に `.vertical()` を付けると、起点が右上になる |
| 高解像度の画面で小さい | 矩形・余白・フォントの大きさに倍率を掛けたか(手順 7) |

## 動作の確認

リポジトリの `examples/` に、組み込みの出発点になる最小の例 `minimal`、窓なしで PNG に書き出す `snapshot`、窓で確かめる `viewer` があります。[nekomoji のリポジトリ](https://github.com/lovelygenerator/nekomoji)で試せます。

```sh
cargo run -p nekomoji-wgpu --example snapshot -- <日本語フォント> <シナリオ.neko> -o out.png
```

## 対応する Rust

wgpu 30 に合わせて、Rust 1.87 以上が必要です(コアの `nekomoji` は 1.75 以上)。

## ライセンス

次のどちらかを選べます(crates.io では相対リンクが切れるので、URL で書いています)。

- Apache License, Version 2.0([LICENSE-APACHE](https://github.com/lovelygenerator/nekomoji/blob/main/crates/nekomoji-wgpu/LICENSE-APACHE))
- MIT license([LICENSE-MIT](https://github.com/lovelygenerator/nekomoji/blob/main/crates/nekomoji-wgpu/LICENSE-MIT))
