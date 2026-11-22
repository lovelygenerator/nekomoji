# assets/fonts

examples(`viewer` と `snapshot`)が使う既定のフォントの置き場所です。

フォントを指定せずに `cargo run -p nekomoji-wgpu --example viewer` などを実行すると、ここに
**Zen Maru Gothic**(SIL Open Font License 1.1)の Regular と `OFL.txt` が無ければ、公式のリポジトリ
(googlefonts/zen-marugothic)の決まったコミットから `curl` で取ってきます。どの環境でも同じフォントで描くので、
見た目が揃います。

- ここに置いたフォントは git に入りません(`.gitignore`)。リポジトリや crate にフォントは含めません
- 取ってきたくないときは `NEKOMOJI_NO_FETCH=1`(OS の日本語フォントを使います。見た目は環境で変わります)
- 別のフォントを使うときは、引数にパスを渡すか `NEKOMOJI_FONT` に入れます
- 取り直すときは、このフォルダのフォントを消してから実行します
