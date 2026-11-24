//! 窓を開いて、画面の下のメッセージ窓に文字を出す最小の例(README の「使い方の手順」と同じ順)。
//!
//! ```text
//! cargo run -p nekomoji-wgpu --example minimal [-- <日本語フォント>]
//! ```
//!
//! クリック(か Space・Enter)で、文字送りの途中なら残りを出し、入力待ちなら次へ進む。Esc で終わる。
//! フォントを省くと `assets/fonts/` の Zen Maru Gothic を使う(無ければ初回に取ってくる)。
//!
//! viewer のような確認の道具ではなく、お使いのゲームに組み込むときの出発点にするための
//! 短い例なので、シナリオの読み直し・オーバーレイ・猫モードなどは入れていない。

mod common {
    // 他の例と共有するので、この例で使わない関数がある
    #[allow(dead_code)]
    pub mod sysfont;
}

use common::sysfont;
use nekomoji::{
    parse, Layout, LayoutOptions, ParseError, Reveal, RevealOptions, Script, TextEvent, Vars,
};
use nekomoji_wgpu::{
    atlas::Atlas, build_quads_into, surface_config, Built, FontSet, Placement, Renderer,
    TextWindow, DEFAULT_ATLAS_SIZE,
};
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// 字の大きさ(論理ピクセル。高解像度の画面では倍率を掛ける)
const FONT_PX: f32 = 32.0;

const SCENE: &str = "[ruby めいきゅう]迷宮[/ruby]の奥で、[var name]は振り返った。[l]\
[shake 2 1.5]――誰だっ！[/shake][p]\
[speed 0.5]……そこにいるのは、[wave]ミケ[/wave]か。[/speed][p]\
[color #66ccff][glow breathe]青い光[/glow][/color]が灯った。クリックで終わり。";

/// 手順 2〜6: 文字を出すための状態。GPU の部分(`Renderer`)は持たない。
struct Game {
    font_data: sysfont::Font,
    font: FontSet,
    scale: f32,
    atlas: Atlas,
    script: Script,
    vars: Vars,
    window: TextWindow,
    opts: LayoutOptions,
    place: Placement,
    layout: Layout,
    reveal: Reveal,
    built: Built,
}

impl Game {
    fn new(font_data: sysfont::Font, screen: [u32; 2], scale: f32) -> Result<Self, String> {
        let font = sysfont::font_set(&font_data, &[], FONT_PX * scale, false)?;
        let script = parse(SCENE).map_err(|e: ParseError| e.to_string())?;
        let mut vars = Vars::new();
        vars.insert("name".into(), "ミケ".into());
        // 手順 3: 窓から組版と描画の設定を作る
        let window = window_for(screen, scale, font.base_px);
        let opts = window.layout_options(LayoutOptions::default());
        let place = window.placement(Placement::default());
        // 手順 4: 場面を配置して、文字送りを始める
        let layout = Layout::build(&script, &font, &opts, &vars);
        let reveal = Reveal::new(&layout, RevealOptions::default());
        Ok(Self {
            font_data,
            font,
            scale,
            atlas: Atlas::new(DEFAULT_ATLAS_SIZE),
            script,
            vars,
            window,
            opts,
            place,
            layout,
            reveal,
            built: Built::default(),
        })
    }

    /// 手順 7: 画面の大きさ(か倍率)が変わったとき。窓と配置を作り直し、同じところから続ける。
    fn resize(&mut self, screen: [u32; 2], scale: f32) -> Result<(), String> {
        if scale != self.scale {
            // 字の大きさが変わるので、フォントとアトラスも作り直す
            self.font = sysfont::font_set(&self.font_data, &[], FONT_PX * scale, false)?;
            self.atlas = Atlas::new(DEFAULT_ATLAS_SIZE);
            self.scale = scale;
        }
        self.window = window_for(screen, scale, self.font.base_px);
        self.opts = self.window.layout_options(self.opts.clone());
        self.place = self.window.placement(self.place.clone());
        let state = self.reveal.state(); // 進み具合を控える
        self.layout = Layout::build(&self.script, &self.font, &self.opts, &self.vars);
        self.reveal = Reveal::new(&self.layout, RevealOptions::default());
        self.reveal.restore(state);
        Ok(())
    }

    /// 手順 5: 毎フレーム、時間を進める。出てきた出来事はゲーム側で処理する(ここでは表示するだけ)。
    fn update(&mut self, dt: f32) {
        for event in self.reveal.advance(dt) {
            match event {
                TextEvent::Voice(id) | TextEvent::Se(id) | TextEvent::Blip(id) => {
                    eprintln!("音: {id}")
                }
                TextEvent::Custom { name, args } => eprintln!("イベント: {name} {args:?}"),
                _ => {}
            }
        }
    }

    /// 手順 6: クリック(決定ボタン)。
    fn on_click(&mut self) {
        if self.reveal.is_waiting() {
            self.reveal.proceed();
        } else {
            self.reveal.skip();
        }
    }

    /// 手順 5: 四角形にして GPU へ送る。アトラスが満杯なら大きくして組み立て直す。
    fn prepare(&mut self, gfx: &mut Gfx) {
        for _ in 0..2 {
            build_quads_into(
                &self.layout,
                &self.reveal,
                &self.font,
                &mut self.atlas,
                &self.place,
                &mut self.built,
            );
            if !self.built.overflow {
                break;
            }
            self.atlas.make_room(Renderer::max_atlas_size(&gfx.device));
        }
        let screen = [gfx.config.width as f32, gfx.config.height as f32];
        gfx.renderer.prepare(
            &gfx.device,
            &gfx.queue,
            &mut self.atlas,
            &self.built.quads,
            screen,
        );
    }
}

/// 画面の下に、高さ 200px(論理)のメッセージ窓を置く。
fn window_for(screen: [u32; 2], scale: f32, px: f32) -> TextWindow {
    TextWindow::bottom(
        [screen[0] as f32, screen[1] as f32],
        200.0 * scale,
        24.0 * scale,
        px,
    )
    .padding(16.0 * scale)
}

/// 手順 2: 窓・サーフェス・デバイスと描画器。
struct Gfx {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
}

struct App {
    font_data: Option<sysfont::Font>,
    game: Option<Game>,
    gfx: Option<Gfx>,
    last: Instant,
}

impl App {
    fn init(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("nekomoji minimal")
                        .with_inner_size(LogicalSize::new(960.0, 540.0)),
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
        // 色が明るくならない形式で、サーフェスの設定を作る
        let config = surface_config(&surface, &adapter, [size.width, size.height])
            .expect("サーフェスの設定");
        surface.configure(&device, &config);
        let renderer = Renderer::new(&device, config.format, DEFAULT_ATLAS_SIZE);
        let font_data = self.font_data.take().expect("フォント");
        let game = Game::new(
            font_data,
            [config.width, config.height],
            window.scale_factor() as f32,
        )
        .unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(1)
        });
        self.game = Some(game);
        self.gfx = Some(Gfx {
            window,
            surface,
            config,
            device,
            queue,
            renderer,
        });
    }

    fn frame(&mut self) {
        let (Some(game), Some(gfx)) = (self.game.as_mut(), self.gfx.as_mut()) else {
            return;
        };
        let now = Instant::now();
        game.update((now - self.last).as_secs_f32());
        self.last = now;
        game.prepare(gfx);

        let frame = match gfx.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => {
                gfx.surface.configure(&gfx.device, &gfx.config);
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut enc = gfx.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("nekomoji minimal"),
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
            // ここで背景や窓の板を描く(この例は背景の色で塗りつぶすだけ。窓の板は `game.window.rect` に合わせて描く)
            let size = [gfx.config.width, gfx.config.height];
            game.window.apply_scissor(&mut pass, size); // 窓の外へはみ出す字を切る
            gfx.renderer.draw(&mut pass);
        }
        gfx.queue.submit([enc.finish()]);
        gfx.window.pre_present_notify();
        gfx.queue.present(frame);
        gfx.window.request_redraw();
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
                let (Some(game), Some(gfx)) = (self.game.as_mut(), self.gfx.as_mut()) else {
                    return;
                };
                gfx.config.width = size.width.max(1);
                gfx.config.height = size.height.max(1);
                gfx.surface.configure(&gfx.device, &gfx.config);
                let scale = gfx.window.scale_factor() as f32;
                if let Err(e) = game.resize([gfx.config.width, gfx.config.height], scale) {
                    eprintln!("{e}");
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(game) = self.game.as_mut() {
                    game.on_click();
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.logical_key {
                    Key::Named(NamedKey::Escape) => event_loop.exit(),
                    Key::Named(NamedKey::Space | NamedKey::Enter) => {
                        if let Some(game) = self.game.as_mut() {
                            game.on_click();
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }
}

fn main() {
    // 手順 1: フォントを用意する(引数で渡したもの、無ければ assets/fonts/ の Zen Maru Gothic)
    let explicit = std::env::args().nth(1);
    let font_data = sysfont::resolve(explicit.as_deref(), None).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let event_loop = EventLoop::new().expect("イベントループ");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        font_data: Some(font_data),
        game: None,
        gfx: None,
        last: Instant::now(),
    };
    event_loop.run_app(&mut app).expect("実行");
}
