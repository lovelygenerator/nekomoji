# nekomoji(ねこもじ)

[![crates.io](https://img.shields.io/crates/v/nekomoji.svg)](https://crates.io/crates/nekomoji)
[![docs.rs](https://docs.rs/nekomoji/badge.svg)](https://docs.rs/nekomoji)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/lovelygenerator/nekomoji/blob/main/LICENSE-MIT)
[![license: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/lovelygenerator/nekomoji/blob/main/LICENSE-APACHE)
[![MSRV](https://img.shields.io/badge/MSRV-1.75-blue.svg)](https://github.com/lovelygenerator/nekomoji/blob/main/crates/nekomoji/README.md)
[![CI](https://github.com/lovelygenerator/nekomoji/actions/workflows/ci.yml/badge.svg)](https://github.com/lovelygenerator/nekomoji/actions/workflows/ci.yml)

*Japanese text layout and reveal effects for games, in Rust. The documentation is in Japanese; see the [English README](https://github.com/lovelygenerator/nekomoji/blob/main/README.en.md) for an overview.*

ゲーム向けの日本語テキストの「見せ方」を計算するライブラリです。

ノベルゲームや RPG で、日本語の文字をどこに置き、いつ出し、どう動かすかを計算します。描画は、お使いのエンジンやライブラリに任せられます。**外部の依存はなく**、`unsafe` も使いません。結果はすべて入力と経過時間だけで決まるので、決定的でテストしやすくなっています(エフェクトの三角関数などは、環境によって小数の最後の桁が変わることがあります)。ネイティブでも WASM でも動きます。ブラウザで試せる [Web デモ](https://nekomoji-demo.vercel.app/)があります。

cosmic-text や parley のようなテキストの crate は、フォントと基本的なレイアウトまでを扱います。nekomoji は、その上でゲームに必要になる層(ルビ・禁則・縦書き・文字送り)を受け持ちます。

## 機能

- **マークアップ**: シナリオの本文の書き方。ルビ `[ruby]`(短い書き方 `｜親文字《よみ》` も可)・圏点 `[em]`・色・大きさ・太字・速さ・上付き/下付き・取り消し線/下線・縁取りと影・待機 `[wait]`・クリック待ち `[l]`・改ページ `[p]`・文中のアイコン `[icon]`・ボイス/効果音/独自のイベント・変数 `[var name]`。エラーは行・桁つき(メッセージは日本語。`parse_all` ですべてのエラーをまとめて得られます)
- **組版**: 禁則(行頭禁則・行末禁則)、句読点のぶら下げ、ルビのはみ出し、約物の詰め、和欧間のアキ、両端揃え、字下げ(地の文・会話)、揃え、自動改ページ、書記素クラスタ(異体字セレクタ・ZWJ でつながる絵文字・国旗)
- **縦書き**: 立てる/寝かせるの分類と縦中横 `[tcy]`
- **文字送り**: 1秒あたりの文字数、範囲ごとの速さ、句読点の後の間、ちょうどよい時刻の出来事、スキップ/次へ、保存と復元、オートモードの待ち時間、文字送りの音
- **時刻の関数のエフェクト**: 揺れ・波・フェード・発光(`in` / `breathe` / `flicker`)。光過敏・動きに弱い人への配慮として動きの強さ(`Full` / `Reduced` / `Off`)。点滅は 1 秒に 3 度を超えない
- **猫モード**: 猫が文字の上を歩き回り、文字を猫パンチで飛ばし、入力待ちでは最後の文字を押して落とす。入力待ちには肉球のカーソル。飛んだ文字は戻ってくる
- **当たり判定と範囲**: 文字ごとの元の文章のバイト範囲、名前付きの範囲 `[mark id]`、行ごとの矩形、ページの外寸
- **バックログと読み上げ**: `Page::text()` はルビを `｜親文字《よみ》` の形で残し、`Page::speech_text()` はルビを読みに置き換える

## 例

```rust
use nekomoji::{parse, CellMetrics, Layout, LayoutOptions, Reveal, RevealOptions, TextEvent, Vars};

let script = parse("[ruby めいきゅう]迷宮[/ruby]の奥で、何かが動いた。[voice v1][p][var name]は振り返った。")?;

let mut vars = Vars::new();
vars.insert("name".into(), "ミケ".into());

// 設定は Default から builder のメソッドで組み立てる
let opts = LayoutOptions::default().max_width(18.0).max_lines(3);
// CellMetrics は端末の桁で測る。描画側は代わりに FontMetrics を実装する
let layout = Layout::build(&script, &CellMetrics, &opts, &vars);
let mut reveal = Reveal::new(&layout, RevealOptions::default());

// 描画命令のバッファ(フレームをまたいで使い回す)
let mut glyphs = Vec::new();

// 毎フレーム
let dt = 1.0 / 60.0;
for event in reveal.advance(dt) {
    match event {
        TextEvent::Voice(id) => { /* ボイスを鳴らす */ let _ = id; }
        TextEvent::WaitInput => { /* カーソルを出して、クリックを待つ */ }
        _ => {}
    }
}
reveal.glyphs_into(&layout, &mut glyphs); // 新しい Vec が欲しければ reveal.glyphs(&layout)
for glyph in &glyphs {
    // `glyph.ch` を (`glyph.x`, `glyph.line`) に `glyph.size`・`glyph.alpha`・
    // `glyph.offset`・`glyph.rotation`・`glyph.color` などで描く
    let _ = glyph;
}

// クリックしたら: 文字送りの途中なら全部出し、入力待ちなら次へ
if reveal.is_waiting() {
    reveal.proceed();
} else {
    reveal.skip();
}
# Ok::<(), nekomoji::ParseError>(())
```

位置は論理座標です。`x` は行の中の位置(字の大きさを 1 とする単位)、`line` は行の番号です。縦書き(`LayoutOptions::default().vertical()`)では `x` が行の中を下へ進み、行は右から左へ並びます。論理座標を画面にどう写すかは、描画側で自由に決められます。

### 猫モード

```rust
use nekomoji::{parse, CellMetrics, Layout, LayoutOptions, Neko, NekoMode, Reveal, RevealOptions, Vars};

let script = parse("猫が通りかかった。")?;
let layout = Layout::build(&script, &CellMetrics, &LayoutOptions::default(), &Vars::new());
let mut reveal = Reveal::new(&layout, RevealOptions::default());
let neko = Neko::new(&layout, NekoMode::default().seed(42));

reveal.advance(0.5);
// reveal.glyphs の代わりにこれを描く。飛ばされた文字は飛んでいき、また戻ってくる
let glyphs = neko.glyphs(&layout, &reveal);
// 入力待ちの間は、ここに肉球を描く
let cursor = reveal.cursor(&layout);
# let _ = (glyphs, cursor);
# Ok::<(), nekomoji::ParseError>(())
```

## 描画

nekomoji 自身はグリフをラスタライズしないので、実際のフォントで組むときは、使うフォントの型に `FontMetrics`(送り幅と、必要なら縦の送り・線の位置)を実装して `Layout::build` に渡してください。リポジトリには wgpu の描画アダプタ(`nekomoji-wgpu`。動的なグリフのアトラス、フォントの `vert` による縦書き用の字形、発光・縁取り・影)があるので、参考にしてください。

## マークアップ

マークアップの仕様は [`docs/markup.md`](https://github.com/lovelygenerator/nekomoji/blob/main/docs/markup.md) にあります。

## 対応する Rust の版(MSRV)

Rust 1.75(リポジトリの CLI・wgpu の描画アダプタ・例は 1.87 以上)

## ライセンス

次のどちらかを選べます。

- Apache License, Version 2.0([LICENSE-APACHE](LICENSE-APACHE))
- MIT license([LICENSE-MIT](LICENSE-MIT))
