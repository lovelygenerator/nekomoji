//! GPU(ソフトウェア実装でも可)があれば、画面外のテクスチャへ描いて読み戻す。無ければ抜ける。

use nekomoji::{parse, Layout, LayoutOptions, Neko, NekoMode, Reveal, RevealOptions, Vars};
use nekomoji_wgpu::{
    atlas::Atlas, build_neko_quads, build_quads, FontSet, Placement, Quad, Renderer,
};

const W: u32 = 128;
const H: u32 = 128;

fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        force_fallback_adapter: false,
        ..Default::default()
    })) else {
        eprintln!("GPU が無いので省略");
        return None;
    };
    Some(pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap())
}

fn font() -> Option<FontSet> {
    let data = std::fs::read("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc").ok()?;
    Some(FontSet::from_bytes(&data, 0, 32.0).unwrap())
}

/// 四角形を W×H の画面外テクスチャへ描き、RGBA を読み戻す。
#[allow(clippy::let_and_return)]
fn draw(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut Renderer,
    atlas: &mut Atlas,
    quads: &[Quad],
) -> Vec<u8> {
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    renderer.prepare(device, queue, atlas, quads, [W as f32, H as f32]);
    let view = target.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (W * H * 4) as u64,
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
        renderer.draw(&mut pass);
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
                bytes_per_row: Some(W * 4),
                rows_per_image: Some(H),
            },
        },
        wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([enc.finish()]);
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let data = slice.get_mapped_range().unwrap().to_vec();
    data
}

#[test]
fn 画面外に描くと文字の部分だけ色が付く() {
    let Some(font) = font() else { return };
    let Some((device, queue)) = gpu() else { return };
    let script = parse("あ").unwrap();
    let layout = Layout::build(&script, &font, &LayoutOptions::default(), &Vars::new());
    let mut reveal = Reveal::new(&layout, RevealOptions::default());
    reveal.advance(1.0);
    let mut atlas = Atlas::new(256);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &Placement::default());
    assert_eq!(built.quads.len(), 1);
    let mut renderer = Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, 256);
    let data = draw(&device, &queue, &mut renderer, &mut atlas, &built.quads);
    let lit = data.chunks(4).filter(|p| p[0] > 128).count();
    assert!(lit > 50, "文字のピクセルが描かれていない: {lit}");
    assert!(lit < (W * H) as usize / 2, "塗りつぶしになっている: {lit}");
}

#[test]
fn 入力待ちの肉球のカーソルを描ける() {
    let Some(font) = font() else { return };
    let Some((device, queue)) = gpu() else { return };
    let script = parse("あ").unwrap();
    let layout = Layout::build(&script, &font, &LayoutOptions::default(), &Vars::new());
    let mut reveal = Reveal::new(&layout, RevealOptions::default());
    reveal.advance(1.0);
    let neko = Neko::new(&layout, NekoMode::default().walk(false));
    let mut atlas = Atlas::new(256);
    let place = Placement::default().origin([48.0, 48.0]);
    let built = build_neko_quads(&layout, &reveal, &neko, &font, &mut atlas, &place);
    // 文字1つと肉球
    assert_eq!(built.quads.len(), 2);
    let mut renderer = Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, 256);
    let data = draw(
        &device,
        &queue,
        &mut renderer,
        &mut atlas,
        &built.quads[1..],
    );
    // 肉球の部分だけ色が付く
    let lit = data.chunks(4).filter(|p| p[3] > 100).count();
    assert!(lit > 30, "肉球が描かれていない: {lit}");
}

#[test]
fn アイコンの絵が色のまま画面に出る() {
    let Some(font) = font() else { return };
    let Some((device, queue)) = gpu() else { return };
    let script = parse("[icon red 2]").unwrap();
    let layout = Layout::build(&script, &font, &LayoutOptions::default(), &Vars::new());
    let mut reveal = Reveal::new(&layout, RevealOptions::default());
    reveal.advance(1.0);
    let mut atlas = Atlas::new(256);
    // 赤と、半分透明な青の2枚
    let red: Vec<u8> = [255, 0, 0, 255].repeat(8 * 8);
    atlas.icons_mut().insert("red", 8, 8, &red).unwrap();
    let place = Placement::default().origin([16.0, 16.0]);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
    assert_eq!(built.quads.len(), 1);
    let mut renderer = Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, 256);
    let data = draw(&device, &queue, &mut renderer, &mut atlas, &built.quads);
    // 枠は幅 2 の 64×32 px で、正方形の絵は 32×32 px、枠の中央(x = 32..64)に出る
    let at = |x: u32, y: u32| &data[((y * W + x) * 4) as usize..][..4];
    assert_eq!(at(48, 32), &[255, 0, 0, 255], "絵の中央が赤でない");
    assert_eq!(at(20, 32), &[0, 0, 0, 255], "絵の外が塗られている");
    assert_eq!(at(100, 100), &[0, 0, 0, 255]);
}

#[test]
fn アイコンを後から登録するとテクスチャが大きくなって全部出る() {
    let Some(font) = font() else { return };
    let Some((device, queue)) = gpu() else { return };
    let script = parse("[icon a][icon b]").unwrap();
    let layout = Layout::build(&script, &font, &LayoutOptions::default(), &Vars::new());
    let mut reveal = Reveal::new(&layout, RevealOptions::default());
    reveal.advance(1.0);
    let mut atlas = Atlas::new(256);
    let mut renderer = Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, 256);
    let place = Placement::default().origin([0.0, 0.0]);
    // 1枚目を登録して描き、その後に2枚目を登録して(置き直しが起きても)両方が出る
    atlas
        .icons_mut()
        .insert("a", 4, 4, &[255, 0, 0, 255].repeat(16))
        .unwrap();
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
    let _ = draw(&device, &queue, &mut renderer, &mut atlas, &built.quads);
    atlas
        .icons_mut()
        .insert("b", 300, 300, &[0, 255, 0, 255].repeat(300 * 300))
        .unwrap();
    assert!(atlas.icons().size() >= 512);
    let built = build_quads(&layout, &reveal, &font, &mut atlas, &place);
    assert_eq!(built.quads.len(), 2);
    let data = draw(&device, &queue, &mut renderer, &mut atlas, &built.quads);
    let red = data.chunks(4).filter(|p| p[0] > 200 && p[1] < 50).count();
    let green = data.chunks(4).filter(|p| p[1] > 200 && p[0] < 50).count();
    assert!(red > 100, "1枚目が出ていない: {red}");
    assert!(green > 100, "2枚目が出ていない: {green}");
}
