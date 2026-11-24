//! ブラウザ向けデモ。`trunk serve` で確認する(examples/web-demo/README.md の手順を参照)。
//!
//! URL の問い合わせで確認用の切り替えができる: `?tate=1`(縦書き)・`?neko=1`(猫モード)・`?seed=数`(猫のシード)・
//! `?gl=1`(WebGPU を使わず WebGL2 に固定)・`?outline=1`(縁取りと影の既定)・
//! `?motion=reduced|off`(動きの強さ)。タブの題名に使っている描き方(WebGPU / WebGL2)と fps を出す。
//!
//! 窓の大きさ(と拡大率)が変わったら、キャンバスの画素数・折り返し幅・字の大きさを作り直す。
//! 文字送りの状態は `Reveal::state` / `restore` で引き継ぐ。
//!
//! 画面は左右に分かれる。左がテキスト(`.neko` を開く・手で書く)、右が結果。「プレビュー」(Ctrl/⌘+Enter)で
//! 左のテキストを解析し直して右に出す。エラーは行・桁つきで左の下に出し、右は前の結果のまま。
//! 「フォントを開く」で任意の ttf/otf を読み込める(既定は demo.neko の文字だけのサブセット)。
//!
//! ネイティブでビルドしたときは使い方を表示するだけ。

// メッセージの組み立てはブラウザに依存しないので、ネイティブでもテストできる
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod editor;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod icons;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "これはブラウザ向けのデモです。examples/web-demo で `trunk serve` を実行してください。"
    );
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(web::run());
}

#[cfg(target_arch = "wasm32")]
mod web {
    use nekomoji::{
        parse, parse_all, Layout, LayoutOptions, Motion, Neko, NekoMode, Outline, Reveal,
        RevealOptions, Script, Shadow, Vars, Writing,
    };
    use nekomoji_wgpu::{
        atlas::Atlas, build_neko_quads, surface_format, FontSet, Placement, Renderer,
    };
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_time::Instant;

    use crate::editor;
    use crate::icons;

    const ATLAS_SIZE: u32 = 2048;
    /// デモのシナリオ。`font.ttf` はこのファイルから `nekomoji subset` で作る
    const SRC: &str = include_str!("../demo.neko");

    /// requestAnimationFrame で自分自身を呼び直すクロージャの入れ物
    type FrameLoop = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

    /// URL の問い合わせで決まる、作り直しても変わらない設定
    struct Settings {
        vertical: bool,
        motion: Motion,
        seed: u64,
        /// 猫モード(見えない猫のちょっかい)。URL の ?neko=1 で有効。既定はオフ(`[neko hop]` と肉球のカーソルだけ)
        neko: bool,
        outline: bool,
    }

    /// 設定から猫モードを決める
    fn neko_mode(settings: &Settings) -> NekoMode {
        NekoMode::default().walk(settings.neko).seed(settings.seed)
    }

    struct State {
        /// フォントのデータ(拡大率が変わったら字の大きさを変えて作り直す)
        font_data: Vec<u8>,
        /// 後から読み込んだ全文字入りのフォント(本命に無い文字の代わり。読み込めるまでは `None`)
        full_font: Option<Vec<u8>>,
        script: Script,
        settings: Settings,
        /// 今のキャンバスの画素数と devicePixelRatio
        size: (u32, u32),
        dpr: f64,
        font: FontSet,
        layout: Layout,
        reveal: Reveal,
        neko: Neko,
        atlas: Atlas,
        place: Placement,
        last: Instant,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        device: wgpu::Device,
        queue: wgpu::Queue,
        renderer: Renderer,
        canvas: web_sys::HtmlCanvasElement,
        /// 使っている描き方(`WebGPU` / `WebGL2`)
        backend: String,
        /// fps を数える区切りの始まりと、そこからのフレーム数
        fps_since: Instant,
        frames: u32,
        /// 左側の部品(テキスト・メッセージ・フォント名)
        ui: Ui,
    }

    /// 左側の部品。
    struct Ui {
        textarea: web_sys::HtmlTextAreaElement,
        messages: web_sys::Element,
        font_name: web_sys::Element,
    }

    /// メッセージの種類(CSS のクラス名と同じ)
    const ERROR: &str = "error";
    const WARN: &str = "warn";
    const INFO: &str = "info";

    /// 種類を付けたメッセージの並び
    fn notes(kind: &'static str, lines: Vec<String>) -> Vec<(&'static str, String)> {
        lines.into_iter().map(|l| (kind, l)).collect()
    }

    impl Ui {
        /// メッセージ欄を入れ替える(空なら欄を空にする)。文字として入れる(HTML としては読まない)。
        fn show(&self, notes: &[(&'static str, String)]) {
            self.messages.set_text_content(None);
            let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
                return;
            };
            for (kind, text) in notes {
                if let Ok(row) = doc.create_element("div") {
                    row.set_class_name(kind);
                    row.set_text_content(Some(text));
                    let _ = self.messages.append_child(&row);
                }
            }
        }
    }

    /// 要素 `id` を探す。
    fn by_id(doc: &web_sys::Document, id: &str) -> Result<web_sys::Element, JsValue> {
        doc.get_element_by_id(id)
            .ok_or_else(|| JsValue::from_str(&format!("#{id} が無い")))
    }

    /// 引数なしのイベントの受け口を付ける(クロージャは付けたまま残す)。
    fn listen(
        target: &web_sys::EventTarget,
        event: &str,
        f: impl FnMut() + 'static,
    ) -> Result<(), JsValue> {
        let closure = Closure::<dyn FnMut()>::new(f);
        target.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())?;
        closure.forget();
        Ok(())
    }

    /// 左のテキストを解析して右へ出す(エラーなら右は前の結果のまま)。成功時は注意の一覧を返す。
    fn apply_script(s: &mut State, src: &str) -> Result<Vec<String>, Vec<String>> {
        let src = src.trim_end();
        if src.trim().is_empty() {
            return Err(vec!["テキストが空です".to_string()]);
        }
        s.script = parse_all(src).map_err(|e| editor::format_errors(&e))?;
        Ok(rebuild(s))
    }

    /// 今の台本・フォント・画面の大きさでレイアウト・文字送り・猫を作り直し、最初のページから出す。
    /// フォントの字形に関する注意を返す。
    fn rebuild(s: &mut State) -> Vec<String> {
        s.layout = make_layout(&s.script, &s.font, s.size, s.dpr, s.settings.vertical);
        s.reveal = Reveal::new(
            &s.layout,
            RevealOptions::default().motion(s.settings.motion),
        );
        s.neko = Neko::new(&s.layout, neko_mode(&s.settings));
        editor::warnings(&s.layout, &s.font, s.atlas.icons())
    }

    /// フォントを読み込み直して、今の台本を組み直す。読めなければ今のフォントのまま。
    fn apply_font(s: &mut State, name: &str, data: Vec<u8>) -> Result<Vec<String>, String> {
        let font = make_font_chain(&data, s.full_font.as_deref(), s.dpr)?;
        s.font = font;
        s.font_data = data;
        // 字の形が変わったのでアトラスの字形は使えない
        s.atlas.clear();
        s.ui.font_name.set_text_content(Some(name));
        Ok(rebuild(s))
    }

    /// 「プレビュー」。
    fn preview(state: &Rc<RefCell<State>>) {
        let mut s = state.borrow_mut();
        let src = s.ui.textarea.value();
        let shown = match apply_script(&mut s, &src) {
            Ok(w) => notes(WARN, w),
            Err(e) => notes(ERROR, e),
        };
        s.ui.show(&shown);
    }

    /// `<input type=file>` で選ばれた最初のファイル。選び直せるように入力欄は空に戻す。
    fn picked(input: &web_sys::HtmlInputElement) -> Option<web_sys::File> {
        let file = input.files().and_then(|f| f.get(0));
        input.set_value("");
        file
    }

    async fn read_promise(promise: js_sys::Promise) -> Result<JsValue, JsValue> {
        wasm_bindgen_futures::JsFuture::from(promise).await
    }

    /// 「スクリプトを開く」: 選んだファイルを左のテキストに入れる(プレビューは押してから)。
    async fn open_script(state: Rc<RefCell<State>>, file: web_sys::File) {
        let name = file.name();
        let text = read_promise(file.text())
            .await
            .ok()
            .and_then(|v| v.as_string());
        let s = state.borrow();
        match text {
            Some(text) => {
                s.ui.textarea.set_value(&text);
                s.ui.show(&[(
                    INFO,
                    format!("{name} を開きました。「プレビュー」で右に出します"),
                )]);
            }
            None => s.ui.show(&[(ERROR, format!("{name} を読めません"))]),
        }
    }

    /// 「フォントを開く」: 選んだ ttf/otf に替えて、今の台本を組み直す。
    async fn open_font(state: Rc<RefCell<State>>, file: web_sys::File) {
        let name = file.name();
        let data = read_promise(file.array_buffer())
            .await
            .ok()
            .map(|b| js_sys::Uint8Array::new(&b).to_vec());
        let mut s = state.borrow_mut();
        let shown = match data {
            Some(data) => match apply_font(&mut s, &name, data) {
                Ok(w) => {
                    let mut n = vec![(INFO, format!("フォント {name} を読み込みました"))];
                    n.extend(notes(WARN, w));
                    n
                }
                Err(e) => vec![(ERROR, format!("フォント {name} を読めません: {e}"))],
            },
            None => vec![(ERROR, format!("{name} を読めません"))],
        };
        s.ui.show(&shown);
    }

    /// 左側の部品(プレビュー・ファイルを開く・ショートカット)をつなぐ。
    fn wire_ui(state: &Rc<RefCell<State>>, doc: &web_sys::Document) -> Result<(), JsValue> {
        let ui_textarea = state.borrow().ui.textarea.clone();

        let button = by_id(doc, "preview")?;
        listen(&button, "click", {
            let state = state.clone();
            move || preview(&state)
        })?;

        // テキストの上で Ctrl/⌘+Enter
        let on_key = {
            let state = state.clone();
            Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
                if e.key() == "Enter" && (e.ctrl_key() || e.meta_key()) {
                    e.prevent_default();
                    preview(&state);
                }
            })
        };
        ui_textarea.add_event_listener_with_callback("keydown", on_key.as_ref().unchecked_ref())?;
        on_key.forget();

        let script_input: web_sys::HtmlInputElement = by_id(doc, "open-script")?.dyn_into()?;
        listen(&script_input.clone(), "change", {
            let state = state.clone();
            move || {
                if let Some(file) = picked(&script_input) {
                    wasm_bindgen_futures::spawn_local(open_script(state.clone(), file));
                }
            }
        })?;

        let font_input: web_sys::HtmlInputElement = by_id(doc, "open-font")?.dyn_into()?;
        listen(&font_input.clone(), "change", {
            let state = state.clone();
            move || {
                if let Some(file) = picked(&font_input) {
                    wasm_bindgen_futures::spawn_local(open_font(state.clone(), file));
                }
            }
        })?;
        Ok(())
    }

    /// 1 秒ごとにタブの題名へ描き方と fps を出す(描画の重さの確認用)。
    fn report_fps(s: &mut State, now: Instant) {
        s.frames += 1;
        let secs = (now - s.fps_since).as_secs_f32();
        if secs < 1.0 {
            return;
        }
        let fps = s.frames as f32 / secs;
        s.frames = 0;
        s.fps_since = now;
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            doc.set_title(&format!("nekomoji web demo — {} — {fps:.0} fps", s.backend));
        }
    }

    async fn fetch_bytes(url: &str) -> Result<Vec<u8>, JsValue> {
        let window = web_sys::window().ok_or("window が無い")?;
        let resp: web_sys::Response =
            wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url))
                .await?
                .dyn_into()?;
        if !resp.ok() {
            return Err(JsValue::from_str(&format!(
                "{url} を読めない({})",
                resp.status()
            )));
        }
        let buf = wasm_bindgen_futures::JsFuture::from(resp.array_buffer()?).await?;
        Ok(js_sys::Uint8Array::new(&buf).to_vec())
    }

    /// 本命のフォントに、全文字入りのフォントを代わりとして足す。足せなければ本命だけにする。
    fn make_font_chain(data: &[u8], full: Option<&[u8]>, dpr: f64) -> Result<FontSet, String> {
        if let Some(full) = full {
            match make_font(data, dpr).and_then(|f| f.with_fallback(full, 0)) {
                Ok(font) => return Ok(font),
                Err(e) => web_sys::console::warn_1(&JsValue::from_str(&e)),
            }
        }
        make_font(data, dpr)
    }

    /// 起動の後に全文字入りのフォント(`font-full.ttf`)を読み込み、サブセットに無い文字の代わりにする。
    /// 無い(`trunk serve` など)・読めないときは、サブセットのまま使う。表示中の台本は組み直さない
    /// (今の台本はサブセットの文字だけで組んであり、字形の番号も変わらない)。次の「プレビュー」から効く。
    async fn load_full_font(state: Rc<RefCell<State>>) {
        let data = match fetch_bytes("font-full.ttf").await {
            Ok(d) => d,
            Err(e) => {
                web_sys::console::info_2(&JsValue::from_str("font-full.ttf を使わない:"), &e);
                return;
            }
        };
        let mut s = state.borrow_mut();
        match make_font_chain(&s.font_data, Some(&data), s.dpr) {
            Ok(font) if font.font_count() > 1 => {
                s.font = font;
                s.full_font = Some(data);
                s.ui.font_name
                    .set_text_content(Some("font.ttf(デモ用サブセット)+ 全文字のフォント(代わり)"));
            }
            Ok(_) => {}
            Err(e) => web_sys::console::error_1(&JsValue::from_str(&e)),
        }
    }

    /// URL の問い合わせ文字列(`?a=1&b=2`)から値を引く。
    fn query(key: &str) -> Option<String> {
        let q = web_sys::window()?.location().search().ok()?;
        q.trim_start_matches('?').split('&').find_map(|kv| {
            kv.split_once('=')
                .filter(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        })
    }

    pub async fn run() {
        if let Err(e) = start().await {
            web_sys::console::error_1(&e);
        }
    }

    /// キャンバスの見た目の大きさ × devicePixelRatio を画素数にする(GPU の上限で抑える)。
    fn canvas_size(canvas: &web_sys::HtmlCanvasElement, dpr: f64, max: u32) -> (u32, u32) {
        let px = |v: i32| ((v as f64 * dpr).round() as u32).clamp(1, max);
        (px(canvas.client_width()), px(canvas.client_height()))
    }

    fn make_font(data: &[u8], dpr: f64) -> Result<FontSet, String> {
        FontSet::from_bytes(data, 0, 28.0 * dpr as f32)
    }

    /// 画面幅(縦書きでは高さ)が全角何文字ぶんかから折り返し幅を決めてレイアウトする。スマホでも収まるように
    fn make_layout(
        script: &Script,
        font: &FontSet,
        size: (u32, u32),
        dpr: f64,
        vertical: bool,
    ) -> Layout {
        let room = if vertical { size.1 } else { size.0 };
        let cols = ((room as f32 - 64.0 * dpr as f32) / font.base_px)
            .floor()
            .clamp(8.0, 30.0);
        let opts = LayoutOptions::default()
            .writing(if vertical {
                Writing::Vertical
            } else {
                Writing::Horizontal
            })
            .max_width(cols);
        Layout::build(script, font, &opts, &Vars::new())
    }

    fn make_place(size: (u32, u32), dpr: f64, settings: &Settings) -> Placement {
        // 縦書きでは本文の右上を画面の右端に合わせる
        let margin = 32.0 * dpr as f32;
        let left = if settings.vertical {
            size.0 as f32 - margin
        } else {
            margin
        };
        // URL の ?outline=1 で縁取りと影の既定を付ける(描画の重さを見る)
        Placement::default()
            .origin([left, margin])
            .outline(settings.outline.then(Outline::default))
            .shadow(settings.outline.then(Shadow::default))
    }

    async fn start() -> Result<(), JsValue> {
        let window = web_sys::window().ok_or("window が無い")?;
        let document = window.document().ok_or("document が無い")?;
        let canvas: web_sys::HtmlCanvasElement = document
            .get_element_by_id("nekomoji")
            .ok_or("canvas が無い")?
            .dyn_into()?;
        let dpr = window.device_pixel_ratio();
        // 上限は GPU の設定の後で確かめ直す。ここでは WebGL2 の最低限で抑えておく
        let (w, h) = canvas_size(&canvas, dpr, 2048);
        canvas.set_width(w);
        canvas.set_height(h);

        let font_data = fetch_bytes("font.ttf").await?;
        let font = make_font(&font_data, dpr).map_err(|e| JsValue::from_str(&e))?;
        let script = parse(SRC.trim_end()).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let settings = Settings {
            // URL の ?tate=1 で縦書き
            vertical: query("tate").is_some_and(|v| v == "1"),
            // URL の ?motion=reduced|off で動きの強さ
            motion: match query("motion").as_deref() {
                Some("reduced") => Motion::Reduced,
                Some("off") => Motion::Off,
                _ => Motion::Full,
            },
            // 猫モード。URL の ?seed=数 で乱数のシードを変えられる
            seed: query("seed").and_then(|v| v.parse().ok()).unwrap_or(0),
            // URL の ?neko=1 で猫モード(猫のちょっかい)をオンにする
            neko: query("neko").is_some_and(|v| v == "1"),
            outline: query("outline").is_some_and(|v| v == "1"),
        };
        let layout = make_layout(&script, &font, (w, h), dpr, settings.vertical);
        let reveal = Reveal::new(&layout, RevealOptions::default().motion(settings.motion));
        let neko = Neko::new(&layout, neko_mode(&settings));

        // WebGPU があればそれ、無ければ WebGL2(feature "webgl")。
        // navigator.gpu があってもアダプタを作れないブラウザがあるので、実際に作れるかを確かめてから選ぶ。
        // URL の ?gl=1 で WebGPU を外し、WebGL2 のフォールバックを確かめられる
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        if query("gl").is_some_and(|v| v == "1") {
            desc.backends.remove(wgpu::Backends::BROWSER_WEBGPU);
        }
        let instance = wgpu::util::new_instance_with_webgpu_detection(desc).await;
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let info = adapter.get_info();
        let backend = match info.backend {
            wgpu::Backend::BrowserWebGpu => "WebGPU".to_string(),
            wgpu::Backend::Gl => "WebGL2".to_string(),
            other => format!("{other:?}"),
        };
        web_sys::console::log_1(&JsValue::from_str(&format!(
            "nekomoji: 描き方 {backend}({} / {:?})",
            info.name, info.device_type
        )));
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let mut config = surface
            .get_default_config(&adapter, w, h)
            .ok_or("サーフェスの設定を作れない")?;
        // WebGL2 の既定は sRGB の形式で、そのままだと WebGPU より色が明るく薄くなる
        if let Some(f) = surface_format(&surface.get_capabilities(&adapter).formats) {
            config.format = f;
        }
        surface.configure(&device, &config);
        let renderer = Renderer::new(&device, config.format, ATLAS_SIZE);

        // 文中のアイコン(`[icon ok]`・`[icon heart]`)の絵。字形のアトラスが作り直されても残る
        let mut atlas = Atlas::new(ATLAS_SIZE);
        icons::register(&mut atlas).map_err(|e| JsValue::from_str(&e))?;

        let place = make_place((w, h), dpr, &settings);
        // 左側の部品。テキストには最初にデモのシナリオを入れておく
        let textarea: web_sys::HtmlTextAreaElement = by_id(&document, "src")?.dyn_into()?;
        textarea.set_value(SRC.trim_end());
        let ui = Ui {
            textarea,
            messages: by_id(&document, "messages")?,
            font_name: by_id(&document, "font-name")?,
        };
        ui.font_name
            .set_text_content(Some("font.ttf(デモ用サブセット)"));
        let state = Rc::new(RefCell::new(State {
            font_data,
            full_font: None,
            script,
            settings,
            size: (w, h),
            dpr,
            font,
            layout,
            reveal,
            neko,
            atlas,
            place,
            last: Instant::now(),
            surface,
            config,
            device,
            queue,
            renderer,
            canvas: canvas.clone(),
            backend,
            fps_since: Instant::now(),
            frames: 0,
            ui,
        }));
        wire_ui(&state, &document)?;
        wasm_bindgen_futures::spawn_local(load_full_font(state.clone()));

        // タップ・クリック・キーで進める
        let on_input = {
            let state = state.clone();
            Closure::<dyn FnMut()>::new(move || {
                let mut s = state.borrow_mut();
                if s.reveal.is_waiting() {
                    s.reveal.proceed();
                } else {
                    s.reveal.skip();
                }
            })
        };
        canvas
            .add_event_listener_with_callback("pointerdown", on_input.as_ref().unchecked_ref())?;
        on_input.forget();

        // requestAnimationFrame のループ
        let tick: FrameLoop = Rc::new(RefCell::new(None));
        let tick2 = tick.clone();
        *tick2.borrow_mut() = Some(Closure::new(move || {
            frame(&mut state.borrow_mut());
            request_frame(tick.borrow().as_ref().unwrap());
        }));
        request_frame(tick2.borrow().as_ref().unwrap());
        Ok(())
    }

    fn request_frame(f: &Closure<dyn FnMut()>) {
        web_sys::window()
            .unwrap()
            .request_animation_frame(f.as_ref().unchecked_ref())
            .unwrap();
    }

    /// 窓の大きさ・拡大率が変わっていたら、サーフェス・字の大きさ・レイアウトを作り直す。
    /// 文字送りは同じページの同じ時刻から続ける(ページ数が減って戻せなければ最後のページの頭から)。
    fn follow_resize(s: &mut State) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let dpr = window.device_pixel_ratio();
        let max = s.device.limits().max_texture_dimension_2d;
        let size = canvas_size(&s.canvas, dpr, max);
        if size == s.size && dpr == s.dpr {
            return;
        }
        if dpr != s.dpr {
            match make_font_chain(&s.font_data, s.full_font.as_deref(), dpr) {
                Ok(font) => s.font = font,
                Err(e) => {
                    web_sys::console::error_1(&JsValue::from_str(&e));
                    return;
                }
            }
            // 字の大きさが変わったのでアトラスの字形は使えない
            s.atlas.clear();
        }
        s.size = size;
        s.dpr = dpr;
        s.canvas.set_width(size.0);
        s.canvas.set_height(size.1);
        s.config.width = size.0;
        s.config.height = size.1;
        s.surface.configure(&s.device, &s.config);

        let saved = s.reveal.state();
        s.layout = make_layout(&s.script, &s.font, size, dpr, s.settings.vertical);
        let mut reveal = Reveal::new(
            &s.layout,
            RevealOptions::default().motion(s.settings.motion),
        );
        if !reveal.restore(saved.clone()) {
            let last = s.layout.pages.len().saturating_sub(1);
            reveal.restore(nekomoji::RevealState::new(saved.page.min(last), 0.0, false));
        }
        s.reveal = reveal;
        s.neko = Neko::new(&s.layout, neko_mode(&s.settings));
        s.place = make_place(size, dpr, &s.settings);
    }

    fn frame(s: &mut State) {
        follow_resize(s);
        let now = Instant::now();
        let dt = (now - s.last).as_secs_f32();
        s.last = now;
        report_fps(s, now);
        for e in s.reveal.advance(dt) {
            web_sys::console::log_1(&JsValue::from_str(&format!("{e:?}")));
        }
        // 満杯なら大きくして(上限なら空にして)やり直す
        let max = Renderer::max_atlas_size(&s.device);
        let mut built = build_neko_quads(
            &s.layout,
            &s.reveal,
            &s.neko,
            &s.font,
            &mut s.atlas,
            &s.place,
        );
        for _ in 0..3 {
            if !built.overflow {
                break;
            }
            s.atlas.make_room(max);
            built = build_neko_quads(
                &s.layout,
                &s.reveal,
                &s.neko,
                &s.font,
                &mut s.atlas,
                &s.place,
            );
        }
        s.renderer.prepare(
            &s.device,
            &s.queue,
            &mut s.atlas,
            &built.quads,
            [s.config.width as f32, s.config.height as f32],
        );
        let frame = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => {
                s.surface.configure(&s.device, &s.config);
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = s
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
            s.renderer.draw(&mut pass);
        }
        s.queue.submit([enc.finish()]);
        s.queue.present(frame);
        let _ = &s.canvas;
    }
}
