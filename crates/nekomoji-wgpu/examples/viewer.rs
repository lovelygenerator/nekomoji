//! viewer: シナリオのファイルと nekomoji の出力を確かめるための道具。
//! 窓で文字送りを確かめ、その場で大きさ・幅・行数・書字方向を変え、論理座標を重ねて見られる。
//! シナリオのファイルは保存すると自動で読み直す。マウスの下の文字の元の位置(行・桁)・変数・
//! 名前付きの範囲(`[mark id]`)をタイトルに出し、範囲を強調する。ゲームを進める機能は持たない。
//!
//! ```sh
//! cargo run -p nekomoji-wgpu --example viewer -- [<フォント>] [<.neko>] \
//!     [--vertical] [--width N] [--lines N] [--px N] [--index N] [--fallback <フォント>] [--grid] [--no-neko] [--palt] [--set 変数=値] [--icon id=絵.png]...
//! ```
//!
//! フォント(拡張子 ttf / otf / ttc / otc)を省くと、リポジトリの `assets/fonts/` の Zen Maru Gothic を使う。
//! 無ければ初回に取ってくる(`NEKOMOJI_NO_FETCH=1` で取らず、OS の日本語フォントを使う)。
//! `--fallback <フォント>`(繰り返し可。`system` で OS の日本語フォント)で、本命に字形が無い文字を描く
//! 代わりのフォントを足す。変数の値(`--set`)をサブセットのフォントで確かめるときに使う。
//!
//! キー操作(起動時に端末へも出す):
//!
//! | キー | 動作 |
//! |---|---|
//! | Space / Enter / クリック | 入力待ちなら次へ、文字送り中なら全部出す(名前付きの範囲の上でクリックするとその範囲を選ぶ) |
//! | ← / → | 前 / 次のページ(ページの始めから) |
//! | Backspace | 最初のページへ |
//! | + / - | 文字の大きさ ±4px |
//! | ] / [ | 行の長さの上限 ±1 |
//! | L | 1ページの行数の上限(なし → 2 → 3 → 4 → 5 → なし) |
//! | V | 横書き / 縦書き |
//! | G | 論理座標の重ね描き |
//! | F | 文字に合わせたテキストボックスの枠(`Page::extent`) |
//! | M | 名前付きの範囲をすべて強調 |
//! | N | 猫モード(見えない猫のちょっかいと肉球のカーソル) |
//! | O | 動きの強さ(標準 → 控えめ → なし) |
//! | A | 揃えの既定(行頭 → 中央 → 行末。`[align]` の無い段落) |
//! | I | 字下げ(なし → 小説 → 会話 → 小説+会話) |
//! | E | 縁取りと影の既定(`Placement::outline`・`shadow`。タグの無い文字にも付ける) |
//! | K | 仮名のプロポーショナル詰め(`FontSet::with_proportional`。フォントの `palt`・`vpal`) |
//! | U | オートモード(入力待ちから `Reveal::auto_wait` の秒数で自動で次へ) |
//! | P | 一時停止(時刻を止めてエフェクトや猫を観察する) |
//! | R | シナリオを読み直す(保存したときも自動で読み直す) |
//! | H | キー操作を端末に出す |
//! | Esc | 終了 |
//!
//! 窓のタイトルに今の設定とページ、マウスの下の論理座標(`x` と行)と、そこにある文字
//! (`Page::hit_test`)の元の位置・変数・名前付きの範囲を出す。範囲の上では範囲を薄く強調する。
//!
//! 環境変数: `NEKOMOJI_VERTICAL=1` で縦書き、`NEKOMOJI_SEED`
//! で乱数のシード、`NEKOMOJI_KNOCK`・`NEKOMOJI_BURST`・`NEKOMOJI_PUNCH` で猫のちょっかいの確率
//! (既定 0.15・0.05・0.12)、`NEKOMOJI_FONT` でフォントのパス、`NEKOMOJI_FONT_INDEX` で ttc の番号、
//! `NEKOMOJI_FALLBACK` で代わりのフォント(パスの並び)。

mod common {
    pub mod icons;
    pub mod sysfont;
}

use common::{icons, sysfont};
use nekomoji::{
    parse, Align, Glyph, Indent, Layout, LayoutOptions, Motion, Neko, NekoMode, Outline, Reveal,
    RevealOptions, RevealState, Script, Shadow, TextEvent, Vars, Writing,
};
use nekomoji_wgpu::{
    atlas::Atlas, build_neko_quads, build_overlay, build_quads, build_rects, surface_format,
    FontSet, Overlay, Placement, Renderer,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

const ATLAS_SIZE: u32 = 2048;
const MARGIN: f32 = 32.0;
const DEFAULT_SRC: &str = "[mark 用語:迷宮][ruby めいきゅう]迷宮[/ruby][/mark]の奥で、[color red]何か[/color]が動いた。[wait 0.5][br][shake 2 1.5]――誰だっ！[/shake][p]\
[speed 0.5]……そこにいるのは、[neko hop][wave]ミケ[/wave][/neko]か。[/speed][p]\
[fade 1]ふわりと、[color #66ccff][glow breathe]青い光[/glow][/color]が灯った。[/fade]";
const HELP: &str = "\
キー操作:
  Space / Enter / クリック  次へ(文字送り中は全部出す。範囲の上のクリックは範囲を選ぶ)
  ← / →                     前 / 次のページ
  Backspace                  最初のページへ
  + / -                      文字の大きさ ±4px
  ] / [                      行の長さの上限 ±1
  L                          行数の上限(なし → 2 → 3 → 4 → 5 → なし)
  V                          横書き / 縦書き
  G                          論理座標の重ね描き
  F                          文字に合わせたテキストボックスの枠
  M                          名前付きの範囲をすべて強調
  N                          猫モード
  O                          動きの強さ(標準 → 控えめ → なし)
  A                          揃えの既定(行頭 → 中央 → 行末)
  I                          字下げ(なし → 小説 → 会話 → 小説+会話)
  E                          縁取りと影の既定(タグの無い文字にも付ける)
  K                          仮名のプロポーショナル詰め(palt・vpal)
  U                          オートモード(入力待ちから自動で次へ)
  P                          一時停止
  R                          シナリオを読み直す(保存すると自動でも読み直す)
  H                          この一覧
  Esc                        終了";

/// 字下げの名前と作り方
type IndentPreset = (&'static str, fn() -> Indent);

/// `I` で切り替える字下げ
const INDENTS: [IndentPreset; 4] = [
    ("なし", Indent::default),
    ("小説", Indent::novel),
    ("会話", Indent::dialogue),
    ("小説+会話", || Indent::novel().bracket_rest(1.0)),
];

/// 今の字下げが `INDENTS` の何番目か(どれでもなければ 0)。
fn indent_index(indent: Indent) -> usize {
    INDENTS.iter().position(|(_, f)| f() == indent).unwrap_or(0)
}

/// 文字の大きさの範囲(px)
const PX_RANGE: (f32, f32) = (12.0, 128.0);
/// シナリオのファイルが変わったかを見る間隔(秒)
const WATCH_SECS: f32 = 0.5;

/// 強調の色(RGBA)
mod color {
    /// マウスの下の文字
    pub const HOVER: [f32; 4] = [1.0, 1.0, 1.0, 0.12];
    /// マウスの下の名前付きの範囲
    pub const MARK_HOVER: [f32; 4] = [1.0, 0.8, 0.3, 0.18];
    /// 選んだ範囲
    pub const MARK_SELECTED: [f32; 4] = [1.0, 0.8, 0.3, 0.9];
    /// すべての範囲(M)
    pub const MARK_ALL: [f32; 4] = [0.5, 0.8, 1.0, 0.15];
    /// 文字に合わせたテキストボックスの枠(F)
    pub const BOX: [f32; 4] = [0.85, 0.85, 0.95, 0.8];
    /// 文中のアイコン([icon])の場所(絵の代わり)
    pub const ICON: [f32; 4] = [0.6, 0.9, 0.6, 0.9];
}

/// テキストボックスの枠の余白(文字の大きさに対する比)
const BOX_PADDING: f32 = 0.5;

struct Gfx {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
}

/// その場で変えられる設定。
struct Settings {
    px: f32,
    opts: LayoutOptions,
    vars: Vars,
    neko: NekoMode,
    neko_on: bool,
    /// 動きの強さ
    motion: Motion,
    grid: bool,
    paused: bool,
    /// オートモード
    auto: bool,
    /// 文字に合わせたテキストボックスの枠を描くか
    fit_box: bool,
    /// 名前付きの範囲をすべて強調するか
    all_marks: bool,
    /// 仮名をプロポーショナルに詰めるか
    palt: bool,
}

struct App {
    /// 本命のフォントと代わりのフォント(大きさを変えたら連鎖を作り直す)
    main_font: sysfont::Font,
    fallbacks: Vec<sysfont::Font>,
    font: FontSet,
    /// シナリオのファイル(無ければ既定の本文)と、最後に読んだときの更新時刻
    path: Option<PathBuf>,
    modified: Option<SystemTime>,
    watch_timer: f32,
    /// 元の文章(行・桁を数えるため)
    src: String,
    script: Script,
    /// 選んだ名前付きの範囲(ページと ID)
    selected: Option<(usize, String)>,
    set: Settings,
    layout: Layout,
    reveal: Reveal,
    neko: Neko,
    atlas: Atlas,
    place: Placement,
    last: Instant,
    mouse: Option<[f32; 2]>,
    title: String,
    gfx: Option<Gfx>,
}

impl App {
    fn init(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("nekomoji viewer")
                        .with_inner_size(LogicalSize::new(960.0, 600.0)),
                )
                .expect("ウィンドウを作れない"),
        );
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(event_loop.owned_display_handle()),
        ));
        let surface = instance.create_surface(window.clone()).expect("サーフェス");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("アダプタ");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("デバイス");
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("サーフェスの設定");
        // sRGB の形式だと色が明るく薄くなる
        if let Some(f) = surface_format(&surface.get_capabilities(&adapter).formats) {
            config.format = f;
        }
        surface.configure(&device, &config);
        let renderer = Renderer::new(&device, config.format, ATLAS_SIZE);
        self.gfx = Some(Gfx {
            window,
            surface,
            config,
            device,
            queue,
            renderer,
        });
        self.fit_origin();
    }

    /// 縦書きでは本文の右上を窓の右端に、横書きでは左上を余白の位置に合わせる。
    fn fit_origin(&mut self) {
        let width = self.gfx.as_ref().map_or(960.0, |g| g.config.width as f32);
        self.place.origin = match self.layout.writing {
            Writing::Vertical => [width - MARGIN, MARGIN],
            _ => [MARGIN, MARGIN],
        };
    }

    /// 今の設定でレイアウトを組み直す。ページと時刻はできるだけ保つ。
    fn relayout(&mut self) {
        let keep = self.reveal.state();
        self.layout = Layout::build(&self.script, &self.font, &self.set.opts, &self.set.vars);
        warn(&self.layout, &self.font);
        self.reveal = Reveal::new(
            &self.layout,
            RevealOptions::default().motion(self.set.motion),
        );
        let page = keep.page.min(self.layout.pages.len().saturating_sub(1));
        let time = if page == keep.page { keep.time } else { 0.0 };
        self.reveal.restore(RevealState::new(
            page,
            time,
            keep.skipped && page == keep.page,
        ));
        self.neko = Neko::new(&self.layout, self.set.neko.clone());
        self.atlas.clear();
        self.fit_origin();
    }

    /// 文字の大きさを変える(フォントを作り直す)。
    fn set_px(&mut self, px: f32) {
        let px = px.clamp(PX_RANGE.0, PX_RANGE.1);
        if px == self.set.px {
            return;
        }
        match sysfont::font_set(&self.main_font, &self.fallbacks, px, self.set.palt) {
            Ok(f) => {
                self.font = f;
                self.set.px = px;
                self.relayout();
            }
            Err(e) => eprintln!("フォントを作り直せない: {e}"),
        }
    }

    /// シナリオを読み直す。解析に失敗したら前のまま。
    fn reload(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("読めない {}: {e}", path.display());
                return;
            }
        };
        self.modified = modified(&path);
        match parse(src.trim_end()) {
            Ok(script) => {
                self.script = script;
                self.src = src.trim_end().to_string();
                self.relayout();
                eprintln!("読み直した: {}", path.display());
            }
            Err(e) => eprintln!("{e}(前のシナリオのまま)"),
        }
    }

    /// シナリオのファイルが更新されていたら読み直す。
    fn watch(&mut self, dt: f32) {
        self.watch_timer += dt;
        if self.watch_timer < WATCH_SECS {
            return;
        }
        self.watch_timer = 0.0;
        if let Some(path) = &self.path {
            let m = modified(path);
            if m.is_some() && m != self.modified {
                self.reload();
            }
        }
    }

    /// ページ `page` の始めへ移る。
    fn goto_page(&mut self, page: usize) {
        if page < self.layout.pages.len() {
            self.reveal.restore(RevealState::new(page, 0.0, false));
        }
    }

    fn update_title(&mut self) {
        let s = &self.set;
        let mut t = format!(
            "nekomoji viewer — {}px 幅{} 行数{} {} 揃え{} 字下げ{} 詰め{} 猫{} 動き{} 縁{} 格子{}{} | {}/{}",
            s.px,
            s.opts.max_width,
            s.opts
                .max_lines
                .map_or("なし".to_string(), |n| n.to_string()),
            if self.layout.writing == Writing::Vertical {
                "縦書き"
            } else {
                "横書き"
            },
            match s.opts.align {
                Align::Center => "中央",
                Align::End => "行末",
                _ => "行頭",
            },
            INDENTS[indent_index(s.opts.indent)].0,
            if s.palt { "あり" } else { "なし" },
            if s.neko_on { "あり" } else { "なし" },
            match s.motion {
                Motion::Full => "標準",
                Motion::Reduced => "控えめ",
                _ => "なし",
            },
            if self.place.outline.is_some() {
                "あり"
            } else {
                "なし"
            },
            if s.grid { "あり" } else { "なし" },
            if s.paused {
                " 一時停止"
            } else if s.auto {
                " オート"
            } else {
                ""
            },
            self.reveal.page() + 1,
            self.reveal.page_count(),
        );
        if s.fit_box {
            if let Some(p) = self.layout.pages.get(self.reveal.page()) {
                let e = p.extent();
                t.push_str(&format!(" | 外寸 {:.2}×{}行", e.width, e.lines));
            }
        }
        if let Some(m) = self.mouse {
            let [x, line] =
                self.place
                    .to_logical(&self.layout, self.reveal.page(), self.font.base_px, m);
            t.push_str(&format!(" | x={x:.2} 行={line:.2}"));
            if let Some(g) = self.hovered() {
                t.push_str(&format!(" | {}", self.describe(g)));
            }
        }
        if t != self.title {
            if let Some(g) = &self.gfx {
                g.window.set_title(&t);
            }
            self.title = t;
        }
    }

    /// マウスの下にある、表示済みの文字の添字(ページ内)。
    fn hovered_index(&self) -> Option<usize> {
        let m = self.mouse?;
        let [x, line] =
            self.place
                .to_logical(&self.layout, self.reveal.page(), self.font.base_px, m);
        let page = self.layout.pages.get(self.reveal.page())?;
        page.hit_test(x, line)
            .filter(|&i| i < self.reveal.visible_count())
    }

    fn hovered(&self) -> Option<&Glyph> {
        let i = self.hovered_index()?;
        self.layout.pages.get(self.reveal.page())?.glyphs.get(i)
    }

    /// 文字の説明: 文字・元の位置(行:桁)・変数・名前付きの範囲。
    fn describe(&self, g: &Glyph) -> String {
        let (line, col) = line_col(&self.src, g.source.start);
        let mut d = match &g.icon {
            Some(id) => format!("アイコン {id} {line}:{col}"),
            None => format!("「{}」{line}:{col}", g.text()),
        };
        if let Some(name) = g.var.and_then(|v| self.layout.vars.get(v)) {
            d.push_str(&format!(" 変数 [var {name}]"));
        }
        let marks = self.layout.marks_of(g);
        if !marks.is_empty() {
            d.push_str(&format!(" 範囲 {}", marks.join(" > ")));
        }
        d
    }

    /// 範囲の強調・テキストボックスの枠の四角形。
    fn highlight_quads(&mut self) -> Vec<nekomoji_wgpu::Quad> {
        let page = self.reveal.page();
        let base = self.font.base_px;
        let mut out = Vec::new();
        // 文中のアイコン([icon])のうち、絵を登録していないものは場所を枠で示す
        let p = &self.layout.pages[page];
        let icons: Vec<[f32; 4]> = p
            .glyphs
            .iter()
            .enumerate()
            .take(self.reveal.visible_count())
            .filter(|(_, g)| {
                g.icon
                    .as_deref()
                    .is_some_and(|id| !self.atlas.icons().contains(id))
            })
            .flat_map(|(i, _)| {
                self.place
                    .span_rects(&self.layout, page, base, &p.spans(i, 1))
            })
            .collect();
        out.extend(build_rects(&mut self.atlas, &icons, color::ICON, true));
        let rects_of = |app: &Self, id: &str| {
            app.place
                .span_rects(&app.layout, page, base, &app.layout.mark_spans(page, id))
        };
        if self.set.fit_box {
            let [x, y, w, h] = self.place.extent_rect(&self.layout, page, base);
            if w > 0.0 && h > 0.0 {
                let pad = BOX_PADDING * base;
                let r = [x - pad, y - pad, w + pad * 2.0, h + pad * 2.0];
                out.extend(build_rects(&mut self.atlas, &[r], color::BOX, true));
            }
        }
        if self.set.all_marks {
            let ids: Vec<String> = self.layout.marks(page).into_iter().map(|m| m.id).collect();
            for id in ids {
                let rects = rects_of(self, &id);
                out.extend(build_rects(&mut self.atlas, &rects, color::MARK_ALL, false));
            }
        }
        if let Some(i) = self.hovered_index() {
            let p = &self.layout.pages[page];
            let rects = self
                .place
                .span_rects(&self.layout, page, base, &p.spans(i, 1));
            out.extend(build_rects(&mut self.atlas, &rects, color::HOVER, false));
            // いちばん内側の範囲を強調する
            if let Some(id) = self.layout.marks_of(&p.glyphs[i]).last().cloned() {
                let rects = rects_of(self, &id);
                out.extend(build_rects(
                    &mut self.atlas,
                    &rects,
                    color::MARK_HOVER,
                    false,
                ));
            }
        }
        if let Some((p, id)) = self.selected.clone() {
            if p == page {
                let rects = rects_of(self, &id);
                out.extend(build_rects(
                    &mut self.atlas,
                    &rects,
                    color::MARK_SELECTED,
                    true,
                ));
            }
        }
        out
    }

    /// クリック: 名前付きの範囲の上ならその範囲を選んで端末に出し、それ以外は次へ進める。
    fn click(&mut self) {
        let page = self.reveal.page();
        let hit = self.hovered_index().and_then(|i| {
            let g = &self.layout.pages[page].glyphs[i];
            let id = self.layout.marks_of(g).last()?.clone();
            Some((i, id))
        });
        let Some((i, id)) = hit else {
            self.selected = None;
            self.advance_input();
            return;
        };
        let p = &self.layout.pages[page];
        let text: String = self
            .layout
            .marks(page)
            .iter()
            .filter(|m| m.id == id)
            .flat_map(|m| &p.glyphs[m.first_glyph..m.first_glyph + m.glyph_count])
            .map(Glyph::text)
            .collect();
        eprintln!("範囲 {id}「{text}」 / {}", self.describe(&p.glyphs[i]));
        self.selected = Some((page, id));
    }

    fn frame(&mut self) {
        let now = Instant::now();
        let real_dt = (now - self.last).as_secs_f32();
        self.last = now;
        self.watch(real_dt);
        let dt = if self.set.paused { 0.0 } else { real_dt };
        for e in self.reveal.advance(dt) {
            match e {
                TextEvent::Voice(id) => println!("voice {id}"),
                TextEvent::Se(id) => println!("se {id}"),
                other => println!("{other}"),
            }
        }
        // オートモード: ボイスの長さは分からないので読む時間だけで待つ
        if self.set.auto && !self.set.paused && self.reveal.auto_due(|_: &str| None) {
            self.reveal.proceed();
        }
        self.update_title();
        if self.gfx.is_none() {
            return;
        }
        let mut quads = Vec::new();
        let max = self
            .gfx
            .as_ref()
            .map_or(ATLAS_SIZE, |g| Renderer::max_atlas_size(&g.device));
        // 満杯なら大きくして(上限なら空にして)やり直す。2048 → 4096 → 8192 の後、もう一度空にして試す
        for attempt in 0..4 {
            let built = if self.set.neko_on {
                build_neko_quads(
                    &self.layout,
                    &self.reveal,
                    &self.neko,
                    &self.font,
                    &mut self.atlas,
                    &self.place,
                )
            } else {
                build_quads(
                    &self.layout,
                    &self.reveal,
                    &self.font,
                    &mut self.atlas,
                    &self.place,
                )
            };
            if built.overflow && attempt < 3 {
                self.atlas.make_room(max);
                continue;
            }
            quads = built.quads;
            break;
        }
        let highlight = self.highlight_quads();
        // 強調は文字の後ろに敷く(枠と選んだ範囲の枠線は前でもよいが、まとめて後ろに置く)
        quads.splice(0..0, highlight);
        if self.set.grid {
            let opts = Overlay::default()
                .max_width(self.set.opts.max_width)
                .max_lines(self.set.opts.max_lines);
            quads.extend(build_overlay(
                &self.layout,
                &self.reveal,
                self.set.neko_on.then_some(&self.neko),
                &self.font,
                &mut self.atlas,
                &self.place,
                &opts,
            ));
        }
        let Some(g) = self.gfx.as_mut() else { return };
        g.renderer.prepare(
            &g.device,
            &g.queue,
            &mut self.atlas,
            &quads,
            [g.config.width as f32, g.config.height as f32],
        );
        let frame = match g.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => {
                g.surface.configure(&g.device, &g.config);
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = g
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("nekomoji"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.07,
                            g: 0.07,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            g.renderer.draw(&mut pass);
        }
        g.queue.submit([enc.finish()]);
        g.window.pre_present_notify();
        g.queue.present(frame);
        g.window.request_redraw();
    }

    /// 入力待ちなら次へ、文字送り中なら全部出す。
    fn advance_input(&mut self) {
        if self.reveal.is_waiting() {
            self.reveal.proceed();
        } else {
            self.reveal.skip();
        }
    }

    fn key(&mut self, key: Key, event_loop: &ActiveEventLoop) {
        match key {
            Key::Named(NamedKey::Escape) => event_loop.exit(),
            Key::Named(NamedKey::Space | NamedKey::Enter) => self.advance_input(),
            Key::Named(NamedKey::ArrowRight) => self.goto_page(self.reveal.page() + 1),
            Key::Named(NamedKey::ArrowLeft) => self.goto_page(self.reveal.page().saturating_sub(1)),
            Key::Named(NamedKey::Backspace) => self.goto_page(0),
            Key::Character(c) => match c.to_lowercase().as_str() {
                "+" | "=" | ";" => self.set_px(self.set.px + 4.0),
                "-" => self.set_px(self.set.px - 4.0),
                "]" => {
                    self.set.opts.max_width += 1.0;
                    self.relayout();
                }
                "[" => {
                    self.set.opts.max_width = (self.set.opts.max_width - 1.0).max(2.0);
                    self.relayout();
                }
                "l" => {
                    self.set.opts.max_lines = match self.set.opts.max_lines {
                        None => Some(2),
                        Some(n) if n < 5 => Some(n + 1),
                        Some(_) => None,
                    };
                    self.relayout();
                }
                "v" => {
                    self.set.opts.writing = match self.set.opts.writing {
                        Writing::Vertical => Writing::Horizontal,
                        _ => Writing::Vertical,
                    };
                    self.relayout();
                }
                "g" => self.set.grid = !self.set.grid,
                "f" => self.set.fit_box = !self.set.fit_box,
                "m" => self.set.all_marks = !self.set.all_marks,
                "n" => self.set.neko_on = !self.set.neko_on,
                "o" => {
                    self.set.motion = match self.set.motion {
                        Motion::Full => Motion::Reduced,
                        Motion::Reduced => Motion::Off,
                        _ => Motion::Full,
                    };
                    self.reveal.set_motion(self.set.motion);
                }
                "a" => {
                    self.set.opts.align = match self.set.opts.align {
                        Align::Start => Align::Center,
                        Align::Center => Align::End,
                        _ => Align::Start,
                    };
                    self.relayout();
                }
                "i" => {
                    let next = (indent_index(self.set.opts.indent) + 1) % INDENTS.len();
                    self.set.opts.indent = INDENTS[next].1();
                    self.relayout();
                }
                "e" => {
                    let on = self.place.outline.is_none();
                    self.place.outline = on.then(Outline::default);
                    self.place.shadow = on.then(Shadow::default);
                }
                "k" => {
                    match sysfont::font_set(
                        &self.main_font,
                        &self.fallbacks,
                        self.set.px,
                        !self.set.palt,
                    ) {
                        Ok(f) => {
                            self.font = f;
                            self.set.palt = !self.set.palt;
                            self.relayout();
                        }
                        Err(e) => eprintln!("フォントを作り直せない: {e}"),
                    }
                }
                "p" => self.set.paused = !self.set.paused,
                "u" => self.set.auto = !self.set.auto,
                "r" => self.reload(),
                "h" => eprintln!("{HELP}"),
                _ => {}
            },
            _ => {}
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_none() {
            self.init(event_loop);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(g) = self.gfx.as_mut() {
                    g.config.width = size.width.max(1);
                    g.config.height = size.height.max(1);
                    g.surface.configure(&g.device, &g.config);
                }
                self.fit_origin();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.key(event.logical_key, event_loop)
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse = Some([position.x as f32, position.y as f32]);
            }
            WindowEvent::CursorLeft { .. } => self.mouse = None,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.click(),
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }
}

/// 値の無い変数と、フォントに字形の無い文字(代わりの文字も無く、豆腐や空白になる)を端末に出す。
/// 代わりのフォントで描く文字も知らせる。
fn warn(layout: &Layout, font: &FontSet) {
    for name in &layout.missing_vars {
        eprintln!("警告: 変数 [var {name}] に値がない(--set {name}=値)");
    }
    let mut missing: Vec<char> = Vec::new();
    let mut borrowed: Vec<char> = Vec::new();
    let chars = layout.pages.iter().flat_map(|p| {
        p.glyphs
            .iter()
            .filter(|g| g.icon.is_none())
            .map(|g| g.ch)
            .chain(p.rubies.iter().flat_map(|r| r.text.chars()))
    });
    for ch in chars {
        if ch.is_whitespace() || missing.contains(&ch) || borrowed.contains(&ch) {
            continue;
        }
        match font.glyph(ch) {
            None => missing.push(ch),
            Some(g) if g.font > 0 => borrowed.push(ch),
            Some(_) => {}
        }
    }
    if !borrowed.is_empty() {
        let list: String = borrowed.iter().collect();
        eprintln!("代わりのフォントで描く文字 {} 字: {list}", borrowed.len());
    }
    if !missing.is_empty() {
        let list: String = missing.iter().collect();
        eprintln!(
            "警告: フォントに字形が無い文字 {} 字: {list}",
            missing.len()
        );
    }
}

/// 元の文章のバイト位置を行・桁(どちらも 1 始まり、桁は文字単位)にする。
fn line_col(src: &str, byte: usize) -> (usize, usize) {
    let before = src.get(..byte).unwrap_or(src);
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, col)
}

fn modified(path: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn usage() -> ! {
    eprintln!(
        "使い方: viewer [<フォント(ttf/otf/ttc)>] [<.neko>] [--vertical] [--width N] [--lines N] \
         [--px N] [--index N] [--fallback <フォント>] [--grid] [--no-neko] [--palt] [--set 変数=値] [--icon id=絵.png]..."
    );
    std::process::exit(2);
}

fn main() {
    let env = |k: &str| std::env::var(k).ok();
    let vertical_env = env("NEKOMOJI_VERTICAL").is_some_and(|v| v == "1");
    let mut set = Settings {
        px: 36.0,
        opts: LayoutOptions::default().writing(if vertical_env {
            Writing::Vertical
        } else {
            Writing::Horizontal
        }),
        vars: Vars::new(),
        neko: NekoMode::default()
            .seed(
                env("NEKOMOJI_SEED")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
            )
            .knock_chance(
                env("NEKOMOJI_KNOCK")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.15),
            )
            .burst_chance(
                env("NEKOMOJI_BURST")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.05),
            )
            .punch_chance(
                env("NEKOMOJI_PUNCH")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.12),
            ),
        neko_on: true,
        motion: Motion::Full,
        grid: false,
        paused: false,
        auto: false,
        fit_box: false,
        all_marks: false,
        palt: false,
    };
    // ttc は 0 番が中国語向けの並びのことがある。日本語の字形が要るなら JP の番号を渡す想定
    let mut font_index = sysfont::font_index_env();
    let mut width = None;
    let mut fallback_args = Vec::new();
    let mut icon_args = Vec::new();
    let mut pos = Vec::new();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| usage());
        match arg.as_str() {
            "--vertical" => set.opts.writing = Writing::Vertical,
            "--width" => width = Some(val().parse().unwrap_or_else(|_| usage())),
            "--lines" => set.opts.max_lines = Some(val().parse().unwrap_or_else(|_| usage())),
            "--px" => set.px = val().parse().unwrap_or_else(|_| usage()),
            "--index" => font_index = Some(val().parse().unwrap_or_else(|_| usage())),
            "--fallback" => fallback_args.push(val()),
            "--icon" => icon_args.push(val()),
            "--grid" => set.grid = true,
            "--no-neko" => set.neko_on = false,
            "--palt" => set.palt = true,
            "--set" => {
                let kv = val();
                let (k, v) = kv.split_once('=').unwrap_or_else(|| usage());
                set.vars.insert(k.into(), v.into());
            }
            "-h" | "--help" => usage(),
            s if s.starts_with('-') => usage(),
            _ => pos.push(arg),
        }
    }
    // 縦書きでは窓の高さ(600px 前後)に収まる長さにする
    set.opts.max_width = width.unwrap_or(if set.opts.writing == Writing::Vertical {
        12.0
    } else {
        18.0
    });
    // 拡張子がフォントのものはフォント、それ以外はシナリオ
    let (fonts, scripts): (Vec<_>, Vec<_>) =
        pos.into_iter().partition(|p| sysfont::is_font_path(p));
    if fonts.len() > 1 || scripts.len() > 1 {
        usage();
    }
    let path = scripts.first().map(PathBuf::from);
    let src = match &path {
        Some(p) => std::fs::read_to_string(p).unwrap_or_else(|e| {
            eprintln!("読めない {}: {e}", p.display());
            std::process::exit(2)
        }),
        None => DEFAULT_SRC.to_string(),
    };
    let found =
        sysfont::resolve(fonts.first().map(String::as_str), font_index).unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(2)
        });
    let fallbacks = sysfont::resolve_fallbacks(&fallback_args).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    sysfont::report(&found, &fallbacks);
    let font = sysfont::font_set(&found, &fallbacks, set.px, set.palt).unwrap_or_else(|e| {
        eprintln!("フォントを解釈できない: {e}");
        std::process::exit(2)
    });
    let src = src.trim_end().to_string();
    let script = parse(&src).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    let layout = Layout::build(&script, &font, &set.opts, &set.vars);
    warn(&layout, &font);
    let reveal = Reveal::new(&layout, RevealOptions::default().motion(set.motion));
    let neko = Neko::new(&layout, set.neko.clone());
    eprintln!("{HELP}");
    let mut atlas = Atlas::new(ATLAS_SIZE);
    if let Err(e) = icons::register(&mut atlas, &icon_args) {
        eprintln!("{e}");
        std::process::exit(2);
    }

    let event_loop = EventLoop::new().expect("イベントループ");
    event_loop.set_control_flow(ControlFlow::Poll);
    let modified = path.as_ref().and_then(modified);
    let mut app = App {
        main_font: found,
        fallbacks,
        font,
        path,
        modified,
        watch_timer: 0.0,
        src,
        script,
        selected: None,
        set,
        layout,
        reveal,
        neko,
        atlas,
        place: Placement::default(),
        last: Instant::now(),
        mouse: None,
        title: String::new(),
        gfx: None,
    };
    event_loop.run_app(&mut app).expect("実行");
}
