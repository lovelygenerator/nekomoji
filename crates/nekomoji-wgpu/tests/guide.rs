//! README の「使い方の手順」の全体。README の断片はここから抜き出したもので、ここが通れば
//! 手順どおりに組んだものが動く(GPU が無い環境・フォントが無い環境では抜ける)。
//! README を変えたら、ここも同じに直す。

use nekomoji::{parse, Layout, LayoutOptions, Reveal, RevealOptions, TextEvent, Vars, WaitKind};
use nekomoji_wgpu::{
    atlas::Atlas, build_quads_into, Built, FontSet, Placement, Renderer, TextWindow,
    DEFAULT_ATLAS_SIZE,
};

// ---- 手順 2: 起動のとき ----

struct Game {
    font: FontSet,
    atlas: Atlas,
    renderer: Renderer,
    window: TextWindow,
    opts: LayoutOptions,
    place: Placement,
    layout: Layout,
    reveal: Reveal,
    built: Built,
    screen: [u32; 2],
}

impl Game {
    /// `format` は描画先(サーフェス)の形式、`screen` は画面の大きさ(px)、
    /// `scale` は高解像度の画面の倍率(使わなければ 1.0)。
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        font_bytes: &[u8],
        screen: [u32; 2],
        scale: f32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // 字の大きさ(px)。フォントはこの大きさで作り、窓にも同じ値を渡す
        let px = 32.0 * scale;
        let font = FontSet::from_bytes(font_bytes, 0, px)?;
        let atlas = Atlas::new(DEFAULT_ATLAS_SIZE);
        let renderer = Renderer::new(device, format, DEFAULT_ATLAS_SIZE);

        // ---- 手順 3: 窓を決める ----
        let window = window_for(screen, scale, px);
        let opts = window.layout_options(LayoutOptions::default());
        let place = window.placement(Placement::default());

        // ---- 手順 4: 場面を用意する(空の場面から始める) ----
        let layout = Layout::build(&parse("")?, &font, &opts, &Vars::new());
        let reveal = Reveal::new(&layout, RevealOptions::default());

        Ok(Self {
            font,
            atlas,
            renderer,
            window,
            opts,
            place,
            layout,
            reveal,
            built: Built::default(),
            screen,
        })
    }

    /// 手順 4: 場面を切り替える。台本を解析して配置し、文字送りを最初から始める。
    fn set_scene(&mut self, src: &str, vars: &Vars) -> Result<(), nekomoji::ParseError> {
        let script = parse(src)?;
        self.layout = Layout::build(&script, &self.font, &self.opts, vars);
        self.reveal = Reveal::new(&self.layout, RevealOptions::default());
        Ok(())
    }

    /// 手順 5: 毎フレーム、時間を進める。出てきた出来事はゲーム側で処理する。
    fn update(&mut self, dt: f32) {
        for event in self.reveal.advance(dt) {
            match event {
                TextEvent::Voice(id) => {
                    /* ボイスを鳴らす */
                    let _ = id;
                }
                TextEvent::Se(id) => {
                    /* 効果音を鳴らす */
                    let _ = id;
                }
                TextEvent::Blip(id) => {
                    /* 文字送りの音を鳴らす */
                    let _ = id;
                }
                TextEvent::Custom { name, args } => {
                    /* 独自のイベント */
                    let _ = (name, args);
                }
                _ => {}
            }
        }
    }

    /// 手順 6: クリック(決定ボタン)の扱い。
    fn on_click(&mut self) {
        if self.reveal.is_waiting() {
            // 入力待ちなら次へ(ページの途中ならその続き、ページの終わりなら次のページ)
            self.reveal.proceed();
        } else {
            // 文字送りの途中なら、残りを一度に出す
            self.reveal.skip();
        }
    }

    /// 手順 5: 毎フレーム、描く。`pass` は呼び出し側が開いた描画パス。
    fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        // 四角形にする。アトラスが満杯なら大きくして(または空にして)組み立て直す
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
            self.atlas.make_room(Renderer::max_atlas_size(device));
        }
        let screen = [self.screen[0] as f32, self.screen[1] as f32];
        self.renderer
            .prepare(device, queue, &mut self.atlas, &self.built.quads, screen);
        self.window.apply_scissor(pass, self.screen); // 窓の外へはみ出す字を切る
        self.renderer.draw(pass);
    }

    /// 手順 7: 画面の大きさが変わったとき。窓を作り直し、場面を配置し直す。
    fn resize(&mut self, screen: [u32; 2], scale: f32) {
        self.screen = screen;
        self.window = window_for(screen, scale, self.font.base_px);
        self.opts = self.window.layout_options(self.opts.clone());
        self.place = self.window.placement(self.place.clone());
        // 配置が変わるので、同じ場面を組み直す(進み具合は state / restore で引き継げる)
    }
}

/// 画面の下に、高さ 200px のメッセージ窓を置く(倍率つき)。
fn window_for(screen: [u32; 2], scale: f32, px: f32) -> TextWindow {
    TextWindow::bottom(
        [screen[0] as f32, screen[1] as f32],
        200.0 * scale,
        24.0 * scale,
        px,
    )
    .padding(16.0 * scale)
}

// ---- 動作の確認: 画面外のテクスチャへ描いて、文字が出ているか調べる ----

fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
    pollster::block_on(adapter.request_device(&Default::default())).ok()
}

fn font_bytes() -> Option<Vec<u8>> {
    [
        "NEKOMOJI_FONT",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ]
    .iter()
    .find_map(|p| std::fs::read(std::env::var(p).unwrap_or_else(|_| (*p).into())).ok())
}

/// 描いて、色のついた画素の数(全体, 窓より上)を返す。
fn frame(
    game: &mut Game,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: [u32; 2],
) -> (usize, usize) {
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let row = (size[0] * 4).div_ceil(256) * 256;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (row * size[1]) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        game.draw(device, queue, &mut pass);
    }
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(size[1]),
            },
        },
        wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
    );
    queue.submit([enc.finish()]);
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let data = slice.get_mapped_range().unwrap();
    // 窓は画面の下 24px 余白・高さ 200px。その上の行は黒のまま
    let above = (size[1] - 24 - 200) as usize;
    let lit = |p: &[u8]| p[0] > 32 || p[1] > 32 || p[2] > 32;
    let all = data.chunks(4).filter(|p| lit(p)).count();
    let mut out = 0;
    for y in 0..above {
        let r = &data[y * row as usize..y * row as usize + size[0] as usize * 4];
        out += r.chunks(4).filter(|p| lit(p)).count();
    }
    (all, out)
}

#[test]
fn 手順どおりに組むと窓の中だけに文字が出て_クリックで進む() {
    let (Some((device, queue)), Some(bytes)) = (gpu(), font_bytes()) else {
        return;
    };
    let size = [640, 360];
    let mut game = Game::new(&device, wgpu::TextureFormat::Rgba8Unorm, &bytes, size, 1.0).unwrap();
    let mut vars = Vars::new();
    vars.insert("name".into(), "ミケ".into());
    game.set_scene(
        "[var name]は振り返った。[l]誰かが呼んでいる。[p]つぎのページ。",
        &vars,
    )
    .unwrap();

    // 文字送りの途中: 何か出ているが、まだ全部ではない
    game.update(0.3);
    let (part, outside) = frame(&mut game, &device, &queue, size);
    assert!(part > 0, "文字が出ていない");
    assert_eq!(outside, 0, "窓の外に描いている");

    // クリックで残りが出て、`[l]` の入力待ちになる
    game.on_click();
    game.update(0.0);
    assert_eq!(game.reveal.wait_kind(), Some(WaitKind::Click));
    let (all, _) = frame(&mut game, &device, &queue, size);
    assert!(all > part, "{all} <= {part}");

    // もう一度クリックで続き、ページの終わりでクリックすると次のページへ
    game.on_click();
    game.update(10.0);
    assert_eq!(game.reveal.wait_kind(), Some(WaitKind::Page));
    game.on_click();
    assert_eq!(game.reveal.page(), 1);

    // 画面の大きさを変えても動く
    game.resize([800, 450], 1.0);
    let _ = game.opts.max_width;
}
