# nekomoji web demo

`nekomoji-wgpu` を WASM でブラウザに出す確認用デモ。画面は左右に分かれたエディタで、左にスクリプト、右に結果が出る。

- **左**: テキスト欄(最初は `demo.neko`)。「スクリプトを開く」で `.neko`(UTF-8 のテキスト)を読み込める。手で書き換えてもよい
- **プレビュー**(Ctrl/⌘ + Enter): 左のテキストを解析し直して、右に最初のページから出す。エラーがあれば行・桁つきで左の下に出し、右は前の結果のまま。値の無い変数と、フォントに無い文字は注意として出る
- **フォントを開く**: 任意の ttf/otf を読み込んで、今のテキストを組み直す(ファイルはブラウザの中だけで使い、送信しない)。既定の `font.ttf` は `demo.neko` の文字だけのサブセットで、起動の後に全文字入りの `font-full.ttf`(Zen Maru Gothic。公開のとき `dist/` に置く。git には入れない)を読み込んで、サブセットに無い文字の代わりにする。読み込んだ後の「プレビュー」から効く。`trunk serve` では `font-full.ttf` が無いので、ほかの文字を書くときは字の多いフォントを読み込む
- **右**: 結果。タップ(クリック)で入力待ちを進める。猫モードは既定でオフ(`?neko=1` でオン)。画面が狭いとき(幅 700px 以下)は上下に並ぶ

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
cd examples/web-demo && trunk serve
```

- シナリオは `demo.neko`(全タグの説明書。猫の説明のページだけ猫口調)。`font.ttf` は [Zen Maru Gothic](https://github.com/googlefonts/zen-marugothic)(SIL OFL 1.1、`OFL.txt`)を `demo.neko` の文字だけに絞ったもの(約 100 KB)。Reserved Font Name が無いので名前は変えていない
- `demo.neko` を書き換えたら作り直す:
  `cargo run -p nekomoji-cli -- subset ZenMaruGothic-Regular.ttf examples/web-demo/demo.neko -o examples/web-demo/font.ttf`
- Zen Maru Gothic には `―`(U+2015)と `﹅`(圏点)が無いので、`—` と `、` で代わりに描く。`subset` は代わりの文字も残す
- 別のフォントで試すときは `font.ttf` を差し替える

- WebGPU があればそれを使い、無ければ WebGL2 に落ちる(`Limits::downlevel_webgl2_defaults`)
- 画面をタップすると、入力待ちを進める(文字送り中は全部出す)
- `font.ttf` は ttf/otf のみ(ttc は 0 番を読む)
- 猫モードは既定でオフ(`[neko hop]` と肉球のカーソルだけ)。URL に `?neko=1` を付けると、見えない猫が文字を叩いて飛ばし、入力待ちでは最後の文字を押して落とす。`?seed=数` でちょっかいの出方が変わる。`?tate=1` で縦書き
- 確認用の切り替え: `?gl=1`(WebGL2 に固定)・`?outline=1`(縁取りと影の既定)・`?motion=reduced|off`(動きの強さ)。タブの題名に描き方(WebGPU / WebGL2)と fps が出る
- Windows(Git Bash)では `./scripts/wasm-check.sh` で、型検査・ビルド・配信・ブラウザでの確認を順に行える
- Windows のブラウザで WebGPU・WebGL2 とも動くことを確認している
- 公開するときは `trunk build --release` でできる `dist/` を静的なファイルとして配信する。`.wasm` が `application/wasm` で配信されること、`font.ttf` も配信されることを確かめる。フォントを配るときは `OFL.txt` も一緒に置く

## フォントの読み込みを速くする

日本語フォントは数 MB〜十数 MB ある。ダウンロードと、fontdue がバイト列を解析する時間の両方が起動を遅くする。効く順に書く。

### 1. サブセット化(いちばん効く)

シナリオに出る文字だけを残したフォントを作る。数百 KB 以下になり、ダウンロードも解析も短くなる。`nekomoji` の CLI に専用のコマンドがある。シナリオを実際に組版して使う文字を集めるので、タグや変数名は含まれず、ルビの読みは含まれる。

```sh
# フォント(ttf / otf / ttc)とシナリオ(.neko、複数可)を渡す
cargo run -p nekomoji-cli -- subset full.ttf ../meikyu.neko -o font.ttf --set player_name=ミケ
# 例: IPA ゴシック 6.2 MB → 73 KB
```

- `--set 変数=値`: シナリオの `[var 変数]` の値。渡さないと、その値の文字が残らないので警告が出る
- `--chars 文字列`: シナリオ以外に残したい文字(後から足す台詞、UI の文字など)
- `--index N`: ttc のとき何番目のフォントか(既定 0)。取り出してからサブセットにする
- 半角の英数記号と全角スペースは自動で残る(`--no-basic` で止められる)
- フォントに字形が無い文字は、警告として一覧が出る
- シナリオを増やしたら作り直す。残っていない文字は表示されない
- 出力は ttf / otf(元の形式)。**WOFF2 は fontdue が読めない**
- **TrueType 形式(glyf)も CFF 形式(Noto Sans CJK など)も使える。** 例: Noto Sans CJK JP(ttc、19.5 MB)→ 80 KB
- CFF 形式のサブセットでは、GSUB/GPOS(`vert` などの置き換え表)が落ちる。今の nekomoji は使わないので問題ないが、縦書き対応のときは見直す
- ttc を渡すときは `--index` で日本語(JP)の番号を指定する。Noto Sans CJK なら JP が 0 番

別の道具が好みなら、fonttools の `pyftsubset` でも同じことができる。

```sh
pip install fonttools
cat ../meikyu.neko | grep -o . | sort -u | tr -d '\n' > chars.txt
pyftsubset full.otf --text-file=chars.txt --unicodes="U+0020-007E,U+3000" \
  --layout-features='*' --output-file=font.otf
```

### 2. 圧縮して配信する

ttf/otf は gzip や brotli でよく縮む。配信側(CDN・サーバー)でテキスト以外の `font/ttf` も圧縮対象にする。`trunk serve` のローカル確認では効果が見えにくい。

### 3. 並行してダウンロードを始める(preload)

WASM の起動が終わってから `fetch` を始めると遅れる。`index.html` の `<head>` に次を書くと、WASM の読み込みと同時に始まる(このデモの `index.html` には入れてある)。

```html
<link rel="preload" as="fetch" href="font.ttf" crossorigin>
```

### 4. キャッシュ

2回目以降は HTTP キャッシュ(`Cache-Control: max-age=…`)で足りる。オフラインでも動かすなら Service Worker か Cache API に入れる。

### 5. グリフの先読み(未実装)

今は、文字が初めて出るフレームでラスタライズしてアトラスに入れる。1フレームに新しい文字が大量に出ると一瞬引っかかる。読み込み後に `Layout` の全文字を先にアトラスへ入れておけば避けられる。数が多いときは、1フレームに数十文字ずつ分けて入れる。
