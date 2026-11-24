#!/usr/bin/env bash
# Windows(Git Bash)での確認: Web デモの WASM
#
# nekomoji のクローンの中で実行する:
#   ./scripts/wasm-check.sh            # 全部(型検査 → ビルド → ローカルで配信してブラウザで目で確認)
#   ./scripts/wasm-check.sh check      # wasm32 での型検査と clippy だけ(自動で合否を出す)
#   ./scripts/wasm-check.sh build      # 型検査 + trunk build --release(dist/ の中身と大きさ)
#   ./scripts/wasm-check.sh serve      # trunk serve で配信してブラウザで目で確認(ビルドも含む)
#
# 環境変数(任意):
#   PORT=8080         trunk serve のポート(既定 8080)
#   YES=1             目で確認する項目を飛ばす(自動の確認だけ)
#   NO_OPEN=1         ブラウザを自動で開かない(表示された URL を自分で開く)
#
# 確かめること:
#   1. wasm32-unknown-unknown で型検査と clippy が通る(wasm32 の部分はこれまで未検査)
#   2. trunk build --release で dist/ に index.html・.wasm・.js・font.ttf ができる。.wasm の大きさ
#   3. trunk serve で .wasm が application/wasm、font.ttf が取れる
#   4. ブラウザで、WebGPU と WebGL2(?gl=1)の両方で、横書き・縦書き・猫モード・発光が描ける
#   5. 縁取りと影(?outline=1)を付けても fps が落ちない。動きの強さ(?motion=reduced|off)が効く
#
# タブの題名に「描き方(WebGPU / WebGL2)と fps」が出る。ブラウザの開発者ツール(F12)の
# コンソールにも「nekomoji: 描き方 …」と、文字送りの出来事が出る。

set -uo pipefail

MODE="${1:-all}"
PORT="${PORT:-8080}"
TARGET="wasm32-unknown-unknown"

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

# ブラウザで開く
open_url() {
  echo "  → $1"
  [ -n "${NO_OPEN:-}" ] && return
  if command -v start >/dev/null; then start "" "$1"            # Git Bash
  elif command -v cmd.exe >/dev/null; then cmd.exe //c start "" "${1//&/^&}"   # cmd では & を逃がす
  elif command -v xdg-open >/dev/null; then xdg-open "$1" >/dev/null 2>&1 &
  elif command -v open >/dev/null; then open "$1"
  else echo "  (ブラウザで上の URL を開いてください)"; fi
}

# ---- 場所 ----
DIR=$(git rev-parse --show-toplevel 2>/dev/null) || die "nekomoji のクローンの中で実行してください"
DEMO="$DIR/examples/web-demo"
[ -f "$DEMO/Trunk.toml" ] || die "nekomoji のクローンの中で実行してください($DIR)"
cd "$DIR" || die "$DIR に入れません"
WORK="$DIR/target/wasm-check"               # target/ は .gitignore 済み
LOG="$WORK/check.log"
mkdir -p "$WORK"

SERVE_PID=""
stop_serve() {
  [ -z "$SERVE_PID" ] && return
  # Git Bash では Windows のプロセス番号で子(cargo・rustc)ごと止める
  if [ -r "/proc/$SERVE_PID/winpid" ] && command -v taskkill >/dev/null; then
    taskkill //F //T //PID "$(cat "/proc/$SERVE_PID/winpid")" >/dev/null 2>&1 || true
  fi
  kill "$SERVE_PID" 2>/dev/null || true
  wait "$SERVE_PID" 2>/dev/null || true
  SERVE_PID=""
}

# 終わるとき(途中で止めたときも): 配信を止め、結果をまとめて待つ
finish() {
  stop_serve
  echo
  say "結果"
  if [ ${#RESULTS[@]} -gt 0 ]; then printf '  %s\n' "${RESULTS[@]}"; fi
  local ng=0 r
  for r in "${RESULTS[@]+"${RESULTS[@]}"}"; do case "$r" in NG*) ng=$((ng + 1)) ;; esac; done
  if [ "$ng" -gt 0 ]; then warn "NG が $ng 件あります"; else echo "NG はありません"; fi
  echo "ログ: $LOG"
  read -r -p "Enter で閉じる " _ || true
}
trap finish EXIT

exec > >(tee -a "$LOG") 2>&1
echo "---- $(date) mode=$MODE ----"
echo "repo:   $DIR"
echo "branch: $(git branch --show-current)  commit: $(git log -1 --format='%h %s')"

# ---- 道具 ----
tools() {
  say "道具の確認"
  command -v cargo >/dev/null || die "cargo がありません(https://rustup.rs)"
  local rustc_ver
  rustc_ver=$(rustc --version | awk '{print $2}')
  if [ "$(printf '%s\n1.87.0\n' "$rustc_ver" | sort -V | head -1)" != "1.87.0" ]; then
    die "rustc $rustc_ver は古いです。1.87 以上が要ります(rustup update)"
  fi
  echo "rustc $rustc_ver"

  if rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
    echo "target $TARGET: あり"
  else
    echo "target $TARGET が無いので入れます"
    rustup target add "$TARGET" || die "rustup target add $TARGET に失敗"
  fi

  if command -v trunk >/dev/null; then
    echo "trunk: $(trunk --version)"
  else
    warn "trunk がありません"
    if command -v cargo-binstall >/dev/null; then
      echo "cargo binstall trunk で入れます(ビルド済みのものを取ってくるので速い)"
      cargo binstall -y trunk || die "trunk を入れられません"
    else
      local a=y
      [ -z "${YES:-}" ] && read -r -p "  cargo install trunk --locked で入れますか(数分かかります) [y/n] " a
      [ "$a" = y ] || [ "$a" = Y ] || die "trunk を入れてから実行してください(cargo install trunk --locked)"
      cargo install trunk --locked || die "trunk を入れられません"
    fi
    echo "trunk: $(trunk --version)"
  fi
  command -v curl >/dev/null || die "curl がありません"
}

# ---- 1. 型検査 ----
check_types() {
  say "wasm32 での型検査と clippy"
  if cargo check -p nekomoji-web-demo --target "$TARGET"; then pass "cargo check --target $TARGET"
  else fail "cargo check --target $TARGET"; return 1; fi
  if cargo clippy -p nekomoji-web-demo --target "$TARGET" -- -D warnings; then pass "cargo clippy --target $TARGET -D warnings"
  else fail "cargo clippy --target $TARGET -D warnings"; fi
}

# ---- 2. ビルド ----
check_build() {
  say "trunk build --release"
  local dist="$DEMO/dist"
  rm -rf "$dist"
  if (cd "$DEMO" && trunk build --release); then pass "trunk build --release"
  else fail "trunk build --release"; return 1; fi
  [ -f "$dist/index.html" ] && pass "dist/index.html" || fail "dist/index.html"
  [ -f "$dist/font.ttf" ] && pass "dist/font.ttf(copy-file)" || fail "dist/font.ttf(copy-file)"
  local wasm js
  wasm=$(ls "$dist"/*.wasm 2>/dev/null | head -1)
  js=$(ls "$dist"/*.js 2>/dev/null | head -1)
  [ -n "$js" ] && pass "dist/$(basename "$js")" || fail "dist/*.js"
  if [ -n "$wasm" ]; then
    local size gz
    size=$(wc -c < "$wasm" | tr -d ' ')
    gz=$(gzip -9 -c "$wasm" | wc -c | tr -d ' ')
    pass "dist/$(basename "$wasm")($((size / 1024)) KB、gzip で $((gz / 1024)) KB)"
  else
    fail "dist/*.wasm"
  fi
  echo "  dist の中身:"
  (cd "$dist" && ls -l) | sed 's/^/    /'
}

# ---- 3〜5. 配信してブラウザで確認 ----
check_serve() {
  say "trunk serve(ポート $PORT)"
  local base="http://127.0.0.1:$PORT"
  if curl -s -o /dev/null --max-time 2 "$base/"; then
    die "ポート $PORT はもう使われています(PORT=8081 ./scripts/wasm-check.sh serve などで変える)"
  fi
  (cd "$DEMO" && exec trunk serve --release --address 127.0.0.1 --port "$PORT") > "$WORK/serve.log" 2>&1 &
  SERVE_PID=$!
  echo "  起動を待ちます(ビルドを含むので初回は数分かかります。ログ: $WORK/serve.log)"
  local i ok=""
  for i in $(seq 1 600); do
    if curl -s -o /dev/null --max-time 2 "$base/"; then ok=1; break; fi
    if ! kill -0 "$SERVE_PID" 2>/dev/null; then break; fi
    sleep 1
  done
  if [ -z "$ok" ]; then
    tail -n 30 "$WORK/serve.log" | sed 's/^/    /'
    fail "trunk serve が応答する"; return 1
  fi
  pass "trunk serve が応答する"

  # 配信の中身(.wasm の Content-Type は、公開先でも同じことを確かめる)
  local html wasm ct code
  html=$(curl -s "$base/")
  wasm=$(printf '%s' "$html" | grep -o '[^"'"'"' ]*\.wasm' | head -1)
  if [ -n "$wasm" ]; then
    ct=$(curl -s -o /dev/null -w '%{content_type}' "$base/${wasm#/}")
    case "$ct" in application/wasm*) pass ".wasm が application/wasm で配信される" ;;
      *) fail ".wasm が application/wasm で配信される(実際: ${ct:-なし})" ;; esac
  else
    fail "index.html に .wasm の参照がある"
  fi
  code=$(curl -s -o /dev/null -w '%{http_code}' "$base/font.ttf")
  [ "$code" = 200 ] && pass "font.ttf が取れる" || fail "font.ttf が取れる(HTTP $code)"

  if [ -n "${YES:-}" ]; then RESULTS+=("--   ブラウザでの確認(飛ばした)"); return; fi

  cat <<EOF

  これからブラウザでページを順に開きます。各ページで F12 を押してコンソールも見てください。
  ・タブの題名に「nekomoji web demo — 描き方 — 〇〇 fps」が出ます
  ・画面をクリック(タップ)すると、文字送り中は全部出し、入力待ちなら次へ進みます
  ・ページは 4 つ(迷宮と揺れ → ミケ(跳ねる・波) → 青い光(発光) → 縦中横・圏点)
  別のブラウザ(Edge・Chrome・Firefox)でも見るときは、表示された URL を貼り付けてください。
EOF

  echo
  echo "-- (a) 標準: WebGPU で横書き"
  open_url "$base/"
  ask "タブの題名が「WebGPU」になった(Firefox などで WebGPU が無ければ WebGL2 で OK)"
  ask "コンソールに赤いエラー(panic・wgpu のエラー)が無い"
  ask "丸ゴシックで、豆腐(□)や欠けが無い。ルビ・揺れ・跳ねる「ミケ」・波が見えた"
  ask "「青い光」がふわっと明滅して光った(発光)"
  ask "最後のページで縦中横「12」・圏点「ここ」が正しく、入力待ちで肉球のカーソルが明滅した"
  ask "ときどき猫に文字が飛ばされ、数秒後に元の位置へ戻った(出なければ ?seed=1 などで)"
  ask "fps がおおむね画面のリフレッシュレート(60 など)で、カクつかない"

  echo
  echo "-- (b) WebGL2 に固定(?gl=1)"
  open_url "$base/?gl=1"
  ask "タブの題名が「WebGL2」になり、(a) と同じ見た目で描けた(コンソールに赤いエラーが無い)"
  ask "WebGL2 でも発光がにじまず、fps が落ちない"

  echo
  echo "-- (c) 縦書き(?tate=1 と ?tate=1&gl=1)"
  open_url "$base/?tate=1"
  ask "WebGPU の縦書き: 右端から始まり、句読点が右上・括弧と長音が縦向き・ルビが右に付いた"
  open_url "$base/?tate=1&gl=1"
  ask "WebGL2 の縦書きも同じ見た目"

  echo
  echo "-- (d) 縁取りと影(?outline=1 と ?outline=1&gl=1)"
  open_url "$base/?outline=1"
  ask "WebGPU: 文字に黒い縁取りと右下の影が付き、fps が (a) とほぼ変わらない"
  open_url "$base/?outline=1&gl=1"
  ask "WebGL2: 縁取りと影が付き、fps が (b) とほぼ変わらない"

  echo
  echo "-- (e) 動きの強さ(?motion=reduced と ?motion=off)"
  open_url "$base/?motion=reduced"
  ask "控えめ: 揺れ・波が小さく早く止まり、猫パンチと爆散が出ない(入力待ちで最後の1文字がゆっくり落ちるのは出てよい)"
  open_url "$base/?motion=off"
  ask "なし: 揺れ・波・跳ねが動かず、発光は一定の明るさ、肉球が明滅しない"

  echo
  echo "-- (f) 画面の大きさ"
  ask "窓を狭く(またはスマホ幅に。F12 のデバイス表示)して再読み込みすると、折り返しが狭くなり画面に収まる"
}

case "$MODE" in
  all)    tools; check_types; check_build; check_serve ;;
  check)  tools; check_types ;;
  build)  tools; check_types; check_build ;;
  serve)  tools; check_serve ;;
  *)      die "知らないモード: $MODE(all / check / build / serve)" ;;
esac
