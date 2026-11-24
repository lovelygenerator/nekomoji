# 変更履歴

このプロジェクトの主な変更を記録します。

書き方は [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に、版の付け方は [セマンティック バージョニング](https://semver.org/lang/ja/) に従います。

## [0.1.1] - 2026-11-24

`nekomoji-wgpu` を crates.io で公開する版です。コアは互換のまま、`nekomoji-wgpu` が使う項目を 1 つ足しました。

### 追加

- **`nekomoji-wgpu`: 最小の例 `examples/minimal`**。winit で窓を開き、画面の下のメッセージ窓に文字を出す。README の手順 1〜7 の順に書いてあり、クリック(Space・Enter)で進み、窓の大きさ・倍率の変更にも追従する。組み込みの出発点にする短い例(確認の道具は `viewer`)
- **`nekomoji-wgpu`: README を段階的な使い方の説明書にした**。全体の流れ・役割分担の表・7 つの手順(準備・起動・窓・場面・毎フレーム・入力・画面の大きさの変更)・アイコン・カーソル・クリックした文字の判定・猫モード・セーブ・うまくいかないときの表。docs.rs のトップにも同じ内容が出る。断片をつなげた全体は `tests/guide.rs`(画面外へ描いて確かめる)
- **`nekomoji-wgpu`: 入力待ちのカーソルをアイコンにできる**。`Placement::cursor_icon(id)`(すべての種類)・`cursor_icon_for(WaitKind, id)`(ページの途中・ページの終わり・終了で別々に)で、登録したアイコンを最後の文字の後ろ(縦書きは下)に描く。大きさは `cursor_size`(既定は字の高さの 0.8 倍)。猫モードでは肉球の代わりに描き、猫モードでなくても描く(決めなければ今までどおり、通常は何も描かず、猫モードは肉球)。位置と明滅(動きなしでは止める)は肉球と同じ。`examples/snapshot` に `--cursor-icon id` を足した
- **`nekomoji-wgpu`: 窓の補助 `TextWindow`**。画面の矩形・余白・字の大きさから、`LayoutOptions` の幅・行数・書字方向、`Placement` の起点・行の間隔、シザー矩形を作る(`bottom`・`centered`・`scaled`・`contains` つき)。サーフェスの設定を作る `surface_config` と、アトラスの既定の大きさ `DEFAULT_ATLAS_SIZE` も足した
- **`nekomoji-wgpu` を crates.io で公開**(`nekomoji` と同じ版 0.1.1。MSRV は wgpu 30 に合わせて 1.87)。`nekomoji-cli` は今までどおり出さない
- **`nekomoji-wgpu`: 文中のアイコン(`[icon]`)の絵を描けるようにした**。`Atlas::icons_mut().insert(id, 幅, 高さ, RGBA)` で絵を登録すると(`IconSet`)、`build_quads`・`build_neko_quads` が字の枠に収めた絵の四角形を出し、`Renderer` が RGBA のテクスチャで描く。文字送り・揺れ・波・回転・不透明度・縦書きに文字と同じように従う。登録していない ID は何も描かず、`Built::missing_icons` で知らせる。`examples/snapshot`・`examples/viewer` に `--icon id=絵.png` を足した
- **`nekomoji`**: `GlyphInstance::width`(送り幅)を足した。アイコンの絵を置く枠の大きさに使う

## [0.1.0] - 2026-11-23

最初の公開版です。

### 追加

#### `nekomoji`(コア。crates.io で公開)

- **マークアップ**: ゲームのシナリオ向けの書き方。範囲タグ `ruby`・`color`・`size`・`b`・`speed`・`shake`・`wave`・`fade`・`glow`・`outline`・`shadow`・`sup`・`sub`・`s`・`u`・`em`(圏点)・`tcy`(縦中横)・`align`・`mark`(名前付きの範囲)・`blip`(文字送りの音)・`neko`(猫モード)。単独タグ `wait`・`l`(クリック待ち)・`p`・`br`・`icon`・`voice`・`se`・`event`。ルビの短い書き方 `｜親文字《よみ》`(`[ruby]` と同じ。縦線は半角 `|` も可)。変数 `var`(`[var 名前]`)と、`[` `]` のエスケープ(`[[` `]]`)。エラーは行・桁つきの日本語で出す(`parse_all` ですべてのエラーをまとめて得られる)。引数の書き方と範囲は `docs/markup.md` で決めてあり、色は色名か `#rgb`・`#rrggbb`、名前・ID は 1〜64 文字、強さは 0 より大きい値、範囲の外や知らない色名はエラーになる。バックログ用のテキストに出すルビは、いつでもマークアップとして読み戻せる
- **組版**: 禁則(行頭禁則・行末禁則・分離禁止。UAX #14 の分類を JIS X 4051 の和文の約物に合わせた表で判定)、句読点のぶら下げ、英単語・数を途中で改行しない(どの行にも収まらない語だけ文字の間で折り返す)、ルビ(はみ出しを抑えて配置)、改ページ・自動改ページ、約物の詰め(始め括弧の連続 `「『` も)、和欧間のアキ、両端揃え、地の文・会話の字下げ、行頭・中央・行末の揃え
- **縦書き**: 文字ごとの置き方(立てる・寝かせる・句読点・小書きの仮名)と縦中横
- **1文字の単位**: 見た目の1文字(書記素クラスタ)で数える。異体字セレクタ(IVS)・結合文字・ZWJ でつながる絵文字・国旗に対応
- **文字送り**: 1秒あたりの文字数・速さの倍率・句読点の間・待機・クリック待ち・ページ送り待ち、出来事(ボイス・効果音・独自のイベント・文字送りの音)、スキップ・次へ、オートモードの待ち時間、`RevealState` での保存と復元。揺れ・波・フェード・発光は時刻の関数で、同じ入力なら必ず同じ結果になる。描画命令は `glyphs_into`・`decorations_into` で毎フレーム同じバッファに書ける
- **動きの強さ**(`Full` / `Reduced` / `Off`): 光過敏・動きに弱い人への配慮。点滅は 1 秒に 3 度を超えない
- **猫モード**: 猫が文字の上を歩き回り、猫パンチで文字を飛ばしたり、最後の文字を押して落としたりする。シードで決定的
- **元の文章との対応と当たり判定**: 文字ごとの元の文章のバイト範囲、変数、名前付きの範囲、行ごとの矩形、当たり判定、ページの外寸
- **バックログ用のテキスト**と、ルビを読みに置き換えた**読み上げ用のテキスト**
- **文中のアイコン**: U+FFFC で場所を空け、描画側が絵を描く
- 外部の依存なし

#### `nekomoji-cli`(このリポジトリの中で使う。crates.io には出さない)

- `log`・`check`(エラーをすべて出す。複数のファイル、`--json` で機械で読める JSON)・`subset`(TrueType・CFF のフォントのサブセット化)のコマンドと REPL

#### `nekomoji-wgpu`(このリポジトリの中で使う。crates.io には出さない)

- wgpu による描画: 動的なグリフのアトラス、発光・縁取り・影、縦書き用の字形(`vert` / `vrt2`)、仮名のプロポーショナル詰め(`palt` / `vpal`。`FontSet::with_proportional`)、異体字の字形、フォントの代わりの連鎖、猫モードの肉球のカーソル、論理座標の重ね描きと座標の変換。四角形は `build_quads_into`・`build_neko_quads_into` で毎フレーム同じ `Built` に書ける
- `viewer`・`snapshot` の例と、ブラウザのデモ(WebGPU。使えなければ WebGL2)

[0.1.1]: https://github.com/lovelygenerator/nekomoji/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/lovelygenerator/nekomoji/releases/tag/v0.1.0
