#!/usr/bin/env bash
# Windows(Git Bash)での確認: フォントの自動取得と viewer
#
# nekomoji のクローンの中で実行する:
#   ./scripts/windows-check.sh            # 全部(自動の確認 → 画像 → viewer を目で確認)
#   ./scripts/windows-check.sh font       # フォントの自動取得だけ(自動で合否を出す)
#   ./scripts/windows-check.sh snap       # 画像を書き出してフォルダを開く
#   ./scripts/windows-check.sh viewer     # viewer を目で確認(キー操作・自動読み直し)
#
# 環境変数(任意):
#   SKIP_BUILD=1      release ビルドを飛ばす(ビルド済みのとき)
#   YES=1             目で確認する項目を飛ばす(自動の確認だけ)
#
# 確かめること:
#   1. assets/fonts/ が空のとき、初回の実行で Zen Maru Gothic を取ってくる(大きさと SHA-256 が一致する)
#   2. 2回目は取り直さない。リポジトリの外・crates/ の中から実行しても同じ assets/fonts/ を使う
#   3. NEKOMOJI_NO_FETCH=1 で取らず、Windows のフォント(游ゴシック・メイリオなど)に切り替わる
#   4. 引数で渡したフォントが優先される
#   5. 画像(横書き・縦書き・論理座標の重ね描き)が Zen Maru Gothic で描ける
#   6. viewer のキー操作と、シナリオを保存したときの自動読み直し
#
# 元から assets/fonts/ にあったフォントは、確認のあいだ退避して最後に戻す。

set -uo pipefail

MODE="${1:-all}"
ZEN="ZenMaruGothic-Regular.ttf"
ZEN_SIZE=3832756
ZEN_SHA256="a0c0b53543e0993ae2225e629c833f3d51495ad31720694ff112ce4ce11111ef"

say()  { printf '\n\033[1;36m== %s ==\033[0m\n' "$*"; }
warn() { printf '\033[1;33m%s\033[0m\n' "$*"; }
die()  { printf '\033[1;31m%s\033[0m\n' "$*" >&2; exit 1; }

RESULTS=()
pass() { RESULTS+=("OK   $*"); printf '\033[1;32m  OK\033[0m %s\n' "$*"; }
fail() { RESULTS+=("NG   $*"); printf '\033[1;31m  NG\033[0m %s\n' "$*"; }
# 目で確認する項目: y/n を聞いて記録する
ask() {
  if [ -n "${YES:-}" ]; then RESULTS+=("--   $*(飛ばした)"); return; fi
  local a
  read -r -p "  $* [y/n] " a
  if [ "$a" = y ] || [ "$a" = Y ]; then RESULTS+=("OK   $*(目視)"); else RESULTS+=("NG   $*(目視)"); fi
}

# Windows のプログラムに渡すパス(Git Bash の /c/... を C:\... に)
winpath() { if command -v cygpath >/dev/null; then cygpath -w "$1"; else printf '%s' "$1"; fi; }
# フォルダを開く
open_dir() {
  if command -v explorer.exe >/dev/null; then explorer.exe "$(winpath "$1")" || true
  else echo "  (フォルダ: $1)"; fi
}

# ---- 場所 ----
DIR=$(git rev-parse --show-toplevel 2>/dev/null) || die "nekomoji のクローンの中で実行してください"
[ -d "$DIR/crates/nekomoji-wgpu" ] || die "nekomoji のクローンの中で実行してください($DIR)"
cd "$DIR" || die "$DIR に入れません"
FONTS="$DIR/assets/fonts"
WORK="$DIR/target/windows-check"            # target/ は .gitignore 済み
OUT="$WORK/out"
BACKUP="$WORK/font-backup"
LOG="$WORK/check.log"
mkdir -p "$OUT" "$BACKUP"

EXE_SUFFIX=""
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) EXE_SUFFIX=".exe" ;; esac
SNAP="$DIR/target/release/examples/snapshot$EXE_SUFFIX"
NATIVE="$DIR/target/release/examples/viewer$EXE_SUFFIX"

# 終わるとき(途中で止めたときも): 退避したフォントを戻し、結果をまとめて待つ
finish() {
  if [ -f "$BACKUP/$ZEN" ]; then
    mkdir -p "$FONTS"
    mv -f "$BACKUP/$ZEN" "$FONTS/$ZEN"
    [ -f "$BACKUP/OFL.txt" ] && mv -f "$BACKUP/OFL.txt" "$FONTS/OFL.txt"
    echo "退避していたフォントを戻しました"
  fi
  rm -f "$FONTS/$ZEN.part"
  echo
  say "結果"
  if [ ${#RESULTS[@]} -gt 0 ]; then printf '  %s\n' "${RESULTS[@]}"; fi
  local ng=0 r
  for r in "${RESULTS[@]+"${RESULTS[@]}"}"; do case "$r" in NG*) ng=$((ng + 1)) ;; esac; done
  if [ "$ng" -gt 0 ]; then warn "NG が $ng 件あります"; else echo "NG はありません"; fi
  echo "ログ: $LOG"
  echo "画像: $OUT"
  read -r -p "Enter で閉じる " _ || true
}
trap finish EXIT

exec > >(tee -a "$LOG") 2>&1
echo "---- $(date) mode=$MODE ----"
echo "repo:   $DIR"
echo "branch: $(git branch --show-current)  commit: $(git log -1 --format='%h %s')"

# ---- 道具 ----
say "道具の確認"
command -v cargo >/dev/null || die "cargo がありません(https://rustup.rs)"
rustc_ver=$(rustc --version | awk '{print $2}')
if [ "$(printf '%s\n1.87.0\n' "$rustc_ver" | sort -V | head -1)" != "1.87.0" ]; then
  die "rustc $rustc_ver は古いです。1.87 以上が要ります(rustup update)"
fi
echo "rustc $rustc_ver"
if command -v curl >/dev/null; then echo "curl: $(command -v curl)"; else warn "curl が見つかりません(フォントの取得は失敗するはず)"; fi
if [ -n "$EXE_SUFFIX" ]; then
  # examples は Windows のプログラムとして curl を探す(Git Bash の外でも動くか)
  if [ -f "/c/Windows/System32/curl.exe" ]; then echo "Windows 標準の curl.exe: あり"
  else warn "C:\\Windows\\System32\\curl.exe がありません(Git Bash の外から実行すると取得に失敗します)"; fi
fi
if [ "$(git config --get core.autocrlf)" = true ]; then
  warn "core.autocrlf=true です。.gitattributes で LF にそろえていますが、テストが落ちたら false にしてください"
fi

# ---- ビルド ----
build() {
  [ -n "${SKIP_BUILD:-}" ] && [ -f "$SNAP" ] && [ -f "$NATIVE" ] && return
  say "ビルド(release)"
  cargo build --release -p nekomoji-wgpu --example snapshot --example viewer || die "ビルドに失敗"
}

# snapshot を実行して出力を返す。$1=実行する場所、残り=引数
run_snap() {
  local where="$1"; shift
  (cd "$where" && "$SNAP" "$@" 2>&1)
}

sha256_of() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | awk '{print $1}'
  else certutil -hashfile "$(winpath "$1")" SHA256 | sed -n 2p | tr -d ' \r'; fi
}

# ---- 1〜4. フォントの自動取得 ----
check_font() {
  say "フォントの自動取得"
  local neko out tmp
  neko=$(winpath "$DIR/examples/meikyu.neko")
  tmp=$(mktemp -d)

  # 元からあるフォントを退避して、空の状態にする
  [ -f "$FONTS/$ZEN" ] && mv -f "$FONTS/$ZEN" "$BACKUP/$ZEN"
  [ -f "$FONTS/OFL.txt" ] && mv -f "$FONTS/OFL.txt" "$BACKUP/OFL.txt"

  echo "-- 1. 初回(リポジトリの外 $tmp から実行)"
  out=$(run_snap "$tmp" "$neko" -o "$(winpath "$OUT/fetch.png")" --set player_name=ミケ)
  echo "$out" | sed 's/^/    /'
  if echo "$out" | grep -q "取得します"; then pass "初回に取得を始める"; else fail "初回に取得を始める"; fi
  if [ -f "$FONTS/$ZEN" ]; then
    pass "assets/fonts/$ZEN ができた"
    local size sum
    size=$(wc -c < "$FONTS/$ZEN" | tr -d ' ')
    [ "$size" = "$ZEN_SIZE" ] && pass "大きさが一致($size バイト)" || fail "大きさが一致(期待 $ZEN_SIZE、実際 $size)"
    sum=$(sha256_of "$FONTS/$ZEN")
    [ "$sum" = "$ZEN_SHA256" ] && pass "SHA-256 が一致" || fail "SHA-256 が一致(実際 $sum)"
  else
    fail "assets/fonts/$ZEN ができた"
  fi
  [ -f "$FONTS/OFL.txt" ] && pass "OFL.txt も置いた" || fail "OFL.txt も置いた"
  [ -f "$FONTS/$ZEN.part" ] && fail ".part が残っていない" || pass ".part が残っていない"
  echo "$out" | grep -q "フォント: .*$ZEN" && pass "初回の描画に Zen Maru Gothic を使う" || fail "初回の描画に Zen Maru Gothic を使う"
  # 取得できたら退避分は要らない(最後に古いものを戻さない)
  [ -f "$FONTS/$ZEN" ] && rm -f "$BACKUP/$ZEN" "$BACKUP/OFL.txt"

  echo "-- 2. 2回目(リポジトリの外・crates/ の中から)"
  out=$(run_snap "$tmp" "$neko" -o "$(winpath "$OUT/second.png")")
  echo "$out" | sed 's/^/    /'
  if echo "$out" | grep -q "取得します"; then fail "2回目は取り直さない(リポジトリの外)"
  elif echo "$out" | grep -q "フォント: .*$ZEN"; then pass "2回目は取り直さない(リポジトリの外)"
  else fail "2回目も assets/fonts/ を使う(リポジトリの外)"; fi
  out=$( (cd "$DIR/crates" && cargo run -q --release -p nekomoji-wgpu --example snapshot -- ../examples/meikyu.neko -o "$(winpath "$OUT/crates.png")" 2>&1) )
  echo "$out" | sed 's/^/    /'
  echo "$out" | grep -q "フォント: .*$ZEN" && ! echo "$out" | grep -q "取得します" \
    && pass "crates/ から cargo run しても同じ assets/fonts/ を使う" \
    || fail "crates/ から cargo run しても同じ assets/fonts/ を使う"
  if echo "$out" | grep -qF '\\?\'; then fail 'パスに \\?\ が付かない'; else pass 'パスに \\?\ が付かない'; fi

  echo "-- 3. NEKOMOJI_NO_FETCH=1(フォントを一時的によけて)"
  mv -f "$FONTS/$ZEN" "$BACKUP/$ZEN"
  out=$(cd "$tmp" && NEKOMOJI_NO_FETCH=1 "$SNAP" "$neko" -o "$(winpath "$OUT/os-font.png")" 2>&1)
  mv -f "$BACKUP/$ZEN" "$FONTS/$ZEN"
  echo "$out" | sed 's/^/    /'
  echo "$out" | grep -q "取得します" && fail "NO_FETCH では取得しない" || pass "NO_FETCH では取得しない"
  if echo "$out" | grep -q "OS のフォント"; then
    pass "OS の日本語フォントに切り替わる($(echo "$out" | sed -n 's/^フォント: //p'))"
  else
    fail "OS の日本語フォントに切り替わる"
  fi

  echo "-- 4. 引数で渡したフォントが優先される(web-demo のサブセット)"
  out=$(run_snap "$tmp" "$(winpath "$DIR/examples/web-demo/font.ttf")" "$(winpath "$DIR/examples/web-demo/demo.neko")" -o "$(winpath "$OUT/explicit.png")")
  echo "$out" | sed 's/^/    /'
  echo "$out" | grep -q "フォント: .*web-demo.font\.ttf" && pass "引数のフォントを使う" || fail "引数のフォントを使う"

  rm -rf "$tmp"
}

# ---- 5. 画像 ----
check_snap() {
  say "画像(横書き・縦書き・重ね描き)"
  local neko
  neko=$(winpath "$DIR/examples/meikyu.neko")
  run_snap "$DIR" "$neko" -o "$(winpath "$OUT/yoko.png")" --set player_name=ミケ --at 1,end | sed 's/^/    /'
  run_snap "$DIR" "$neko" -o "$(winpath "$OUT/tate.png")" --set player_name=ミケ --vertical --at 1,end | sed 's/^/    /'
  run_snap "$DIR" "$neko" -o "$(winpath "$OUT/grid.png")" --set player_name=ミケ --grid --neko --at end | sed 's/^/    /'
  run_snap "$DIR" "$neko" -o "$(winpath "$OUT/grid-tate.png")" --set player_name=ミケ --grid --vertical --at end | sed 's/^/    /'
  local f
  for f in yoko tate grid grid-tate; do
    [ -s "$OUT/$f.png" ] && pass "$f.png を書き出した" || fail "$f.png を書き出した"
  done
  echo "  画像のフォルダを開きます: $OUT"
  open_dir "$OUT"
  ask "画像が丸ゴシック(Zen Maru Gothic)で、豆腐(□)や欠けが無い"
  ask "grid.png・grid-tate.png で、字の枠(緑)が文字に重なり、行の上限(赤)が正しい位置にある"
}

# ---- 6. viewer ----
check_player() {
  say "viewer(目で確認)"
  if [ -n "${YES:-}" ]; then RESULTS+=("--   viewer(飛ばした)"); return; fi
  local scen
  scen="$WORK/player.neko"
  cp "$DIR/examples/meikyu.neko" "$scen"
  cat <<'EOF'
  窓が開いたら次を試してください(H で端末にキー一覧):
    + / -   文字の大きさ        ] / [  行の長さ       L  行数の上限
    V       縦書き/横書き       G      論理座標       N  猫モード
    P       一時停止            ← / →  ページ         Backspace 最初へ
    マウス  窓のタイトルに論理座標(x と行)が出る
  5 秒後にシナリオの末尾へページを書き足します(自動で読み直されるか見てください)。
  確かめ終わったら窓を閉じてください(Esc)。
EOF
  "$NATIVE" "$(winpath "$scen")" --grid --set player_name=ミケ &
  local pid=$!
  sleep 5
  printf '\n[p][color yellow]自動で読み直しました[/color]\n' >> "$scen"
  echo "  → 書き足しました(タイトルのページ数が増えれば OK)"
  wait "$pid"
  ask "起動時に端末へ「フォント: …$ZEN」と出て、丸ゴシックで描かれた"
  ask "+ / - で大きさ、] / [ で行の長さ、L で行数が変わり、表示中のページが保たれた"
  ask "V で縦書き・横書きが切り替わり、縦書きが窓の右端から始まった"
  ask "G で論理座標が出た・消えた。マウスを動かすとタイトルの x と行が変わった"
  ask "書き足した「自動で読み直しました」のページが増えた(→ で最後まで送ると見える)"
  ask "N・P・← → ・Backspace が効いた"
}

case "$MODE" in
  all)    build; check_font; check_snap; check_player ;;
  font)   build; check_font ;;
  snap)   build; check_snap ;;
  viewer|player) build; check_player ;;
  *)      die "知らないモード: $MODE(all / font / snap / viewer)" ;;
esac
