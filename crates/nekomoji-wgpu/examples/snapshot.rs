//! 画面外に描いて PNG に書き出す(窓を開けない環境での見た目の確認・不具合の報告用)。
//!
//! ```sh
//! cargo run -p nekomoji-wgpu --example snapshot -- [<フォント>] <.neko> -o out.png \
//!     [--at 0.5,1,end] [--page 0] [--vertical] [--width 18] [--lines N] [--justify] \
//!     [--no-tsume] [--wao 0.25] [--indent novel|dialogue|both] [--align start|center|end] [--px 36] [--size 800x320] [--index N] [--fallback <フォント>] [--neko] [--seed N] [--knock P] [--burst P] [--punch P] \
//!     [--set 変数=値] [--grid] [--marks] [--box] [--motion full|reduced|off] \
//!     [--outline] [--shadow] [--bg #rrggbb] [--palt]
//! ```
//!
//! `--at` の時刻ごとに 1 コマ描き、縦に並べて 1 枚の PNG にする(既定は `end`)。
//! 時刻はページの始めからの秒で、`end` はページの最後(入力待ち)から 2 秒後。
//! ページの途中の入力待ち(`[l]`)は、すぐに進んだものとして描く。
//! フォント(拡張子 ttf / otf / ttc / otc)を省くと、`assets/fonts/` の Zen Maru Gothic を使う(無ければ取ってくる)。
//! `--fallback <フォント>`(繰り返し可。`system` で OS の日本語フォント)で、本命に字形が無い文字を描く代わりのフォントを足す。
//! `--grid` で論理座標(行・字の枠・ルビ・上限・カーソル・猫)を重ねて描く。
//! `--marks` で名前付きの範囲(`[mark id]`)を薄く塗り、`--box` で文字に合わせたテキストボックスの枠を描く。
//! `--outline`・`--shadow` で縁取り・影の既定(`Placement::outline`・`shadow`)を付け、`--bg` で背景の色を変える。
//! `--palt` で仮名をプロポーショナルに詰める(`FontSet::with_proportional`。フォントの `palt`・`vpal` を使う)。
//! GPU が無ければソフトウェア実装(lavapipe など)を使う。PNG は依存を足さずに無圧縮で書く。

mod common {
    pub mod sysfont;
}

use common::sysfont;
use nekomoji::{
    parse, Align, Indent, Layout, LayoutOptions, Motion, Neko, NekoMode, Outline, Reveal,
    RevealOptions, Shadow, Vars, WaitKind, Writing,
};
use nekomoji_wgpu::{
    atlas::Atlas, build_neko_quads, build_overlay, build_quads, build_rects, quads::parse_color,
    Overlay, Placement, Renderer,
};

const ATLAS_SIZE: u32 = 2048;
const MARGIN: f32 = 32.0;

struct Args {
    font: Option<String>,
    index: Option<u32>,
    /// 代わりのフォント
    fallbacks: Vec<String>,
    script: String,
    out: String,
    at: Vec<Option<f32>>,
    page: usize,
    layout: LayoutOptions,
    px: f32,
    size: (u32, u32),
    neko: Option<NekoMode>,
    vars: Vars,
    grid: bool,
    marks: bool,
    fit_box: bool,
    motion: Motion,
    outline: bool,
    shadow: bool,
    bg: [f32; 3],
    /// 仮名をプロポーショナルに詰めるか
    palt: bool,
}

fn usage() -> ! {
    eprintln!(
        "使い方: snapshot [<フォント>] <.neko> -o <出力.png> [--at 0.5,1,end] [--page N] [--vertical] \
         [--width N] [--lines N] [--justify] [--no-tsume] [--wao 比] [--indent novel|dialogue|both] [--align start|center|end] [--px N] [--size WxH] [--index N] \
         [--fallback <フォント>] [--neko] [--seed N] [--knock P] [--burst P] [--punch P] [--set 変数=値] [--grid] [--marks] [--box] [--motion full|reduced|off] \
         [--outline] [--shadow] [--bg #rrggbb] [--palt]"
    );
    std::process::exit(2);
}

fn parse_args() -> Args {
    let mut it = std::env::args().skip(1);
    let mut pos = Vec::new();
    let mut a = Args {
        font: None,
        index: sysfont::font_index_env(),
        fallbacks: Vec::new(),
        script: String::new(),
        out: "snapshot.png".into(),
        at: vec![None],
        page: 0,
        layout: LayoutOptions::default().max_width(18.0),
        px: 36.0,
        size: (0, 0),
        neko: None,
        vars: Vars::new(),
        grid: false,
        marks: false,
        fit_box: false,
        motion: Motion::Full,
        outline: false,
        shadow: false,
        palt: false,
        bg: [0.08, 0.08, 0.12],
    };
    let mut width_set = false;
    while let Some(arg) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| usage());
        match arg.as_str() {
            "-o" => a.out = val(),
            "--at" => {
                a.at = val()
                    .split(',')
                    .map(|t| {
                        if t == "end" {
                            None
                        } else {
                            Some(t.parse().unwrap_or_else(|_| usage()))
                        }
                    })
                    .collect()
            }
            "--page" => a.page = val().parse().unwrap_or_else(|_| usage()),
            "--vertical" => a.layout.writing = Writing::Vertical,
            "--width" => {
                a.layout.max_width = val().parse().unwrap_or_else(|_| usage());
                width_set = true;
            }
            "--lines" => a.layout.max_lines = Some(val().parse().unwrap_or_else(|_| usage())),
            "--justify" => a.layout.justify = true,
            "--grid" => a.grid = true,
            "--marks" => a.marks = true,
            "--box" => a.fit_box = true,
            "--outline" => a.outline = true,
            "--shadow" => a.shadow = true,
            "--palt" => a.palt = true,
            "--bg" => a.bg = parse_color(&val()).unwrap_or_else(|| usage()),
            "--motion" => {
                a.motion = match val().as_str() {
                    "full" => Motion::Full,
                    "reduced" => Motion::Reduced,
                    "off" => Motion::Off,
                    _ => usage(),
                }
            }
            "--no-tsume" => a.layout.punct_squeeze = false,
            "--wao" => a.layout.latin_space = val().parse().unwrap_or_else(|_| usage()),
            "--px" => a.px = val().parse().unwrap_or_else(|_| usage()),
            "--align" => a.layout.align = Align::from_name(&val()).unwrap_or_else(|| usage()),
            "--indent" => {
                a.layout.indent = match val().as_str() {
                    "novel" => Indent::novel(),
                    "dialogue" => Indent::dialogue(),
                    "both" => Indent::novel().bracket_rest(1.0),
                    _ => usage(),
                }
            }
            "--index" => a.index = Some(val().parse().unwrap_or_else(|_| usage())),
            "--fallback" => a.fallbacks.push(val()),
            "--size" => {
                let v = val();
                let (w, h) = v.split_once('x').unwrap_or_else(|| usage());
                a.size = (
                    w.parse().unwrap_or_else(|_| usage()),
                    h.parse().unwrap_or_else(|_| usage()),
                );
            }
            "--neko" => {
                a.neko.get_or_insert_with(NekoMode::default);
            }
            "--seed" => {
                a.neko.get_or_insert_with(NekoMode::default).seed =
                    val().parse().unwrap_or_else(|_| usage())
            }
            "--knock" => {
                a.neko.get_or_insert_with(NekoMode::default).knock_chance =
                    val().parse().unwrap_or_else(|_| usage())
            }
            "--burst" => {
                a.neko.get_or_insert_with(NekoMode::default).burst_chance =
                    val().parse().unwrap_or_else(|_| usage())
            }
            "--punch" => {
                a.neko.get_or_insert_with(NekoMode::default).punch_chance =
                    val().parse().unwrap_or_else(|_| usage())
            }
            "--set" => {
                let kv = val();
                let (k, v) = kv.split_once('=').unwrap_or_else(|| usage());
                a.vars.insert(k.into(), v.into());
            }
            s if s.starts_with('-') => usage(),
            _ => pos.push(arg),
        }
    }
    let (fonts, scripts): (Vec<_>, Vec<_>) =
        pos.into_iter().partition(|p| sysfont::is_font_path(p));
    let ([script], 0 | 1) = (scripts.as_slice(), fonts.len()) else {
        usage()
    };
    a.font = fonts.first().cloned();
    a.script = std::fs::read_to_string(script).unwrap_or_else(|e| {
        eprintln!("読めない {script}: {e}");
        std::process::exit(2)
    });
    if !width_set && a.layout.writing == Writing::Vertical {
        a.layout.max_width = 10.0;
    }
    a
}

/// 1 コマの大きさ(指定がなければレイアウトから見積もる)。
fn frame_size(a: &Args, layout: &Layout, place: &Placement) -> (u32, u32) {
    if a.size != (0, 0) {
        return a.size;
    }
    // 行の方向は、大きな字の行の広がりも含めて最も長いページに合わせる
    let lines = layout
        .pages
        .iter()
        .map(|p| place.line_offset(p, p.lines.len().max(1), a.px))
        .fold(place.line_pitch * a.px, f32::max);
    let along = a.layout.max_width * a.px + 2.0 * MARGIN + a.px;
    let across = lines + 2.0 * MARGIN + a.px;
    match a.layout.writing {
        Writing::Horizontal => (along as u32, across as u32),
        Writing::Vertical => (across as u32, along as u32),
    }
}

fn main() {
    let a = parse_args();
    let found = sysfont::resolve(a.font.as_deref(), a.index).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let fallbacks = sysfont::resolve_fallbacks(&a.fallbacks).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    sysfont::report(&found, &fallbacks);
    let font = sysfont::font_set(&found, &fallbacks, a.px, a.palt).unwrap_or_else(|e| {
        eprintln!("フォントを解釈できない: {e}");
        std::process::exit(2)
    });
    let script = parse(a.script.trim_end()).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let layout = Layout::build(&script, &font, &a.layout, &a.vars);
    let place0 = Placement::default()
        .outline(a.outline.then(Outline::default))
        .shadow(a.shadow.then(Shadow::default));
    let (w, h) = frame_size(&a, &layout, &place0);
    let place = place0.origin(match a.layout.writing {
        Writing::Horizontal => [MARGIN, MARGIN],
        Writing::Vertical => [w as f32 - MARGIN, MARGIN],
    });
    let neko = a.neko.clone().map(|m| Neko::new(&layout, m));

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .expect("GPU(ソフトウェア実装を含む)が見つからない");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("デバイス");
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = Renderer::new(&device, format, ATLAS_SIZE);
    let mut atlas = Atlas::new(ATLAS_SIZE);

    let mut sheet: Vec<u8> = Vec::new();
    for t in &a.at {
        let mut reveal = Reveal::new(&layout, RevealOptions::default().motion(a.motion));
        while reveal.page() < a.page {
            reveal.skip();
            if !reveal.proceed() {
                break;
            }
        }
        let t = t.unwrap_or_else(|| reveal.page_end(reveal.page()) + 2.0);
        // ページの途中の入力待ち([l])は、すぐに進んだものとして描く
        let mut left = t;
        while left > 0.0 {
            let dt = left.min(0.02);
            reveal.advance(dt);
            left -= dt;
            if reveal.wait_kind() == Some(WaitKind::Click) {
                reveal.proceed();
            }
        }
        reveal.advance(0.0);
        let build = |atlas: &mut Atlas| match &neko {
            Some(n) => build_neko_quads(&layout, &reveal, n, &font, atlas, &place),
            None => build_quads(&layout, &reveal, &font, atlas, &place),
        };
        let mut built = build(&mut atlas);
        // 満杯なら大きくしてやり直す(上限まで)
        let max = Renderer::max_atlas_size(&device);
        while built.overflow && atlas.make_room(max) {
            built = build(&mut atlas);
        }
        if built.overflow {
            eprintln!("アトラスが満杯になった");
        }
        let page = reveal.page();
        let mut under = Vec::new();
        if a.fit_box {
            let [x, y, bw, bh] = place.extent_rect(&layout, page, font.base_px);
            let pad = 0.5 * font.base_px;
            let r = [x - pad, y - pad, bw + pad * 2.0, bh + pad * 2.0];
            under.extend(build_rects(&mut atlas, &[r], [0.85, 0.85, 0.95, 0.8], true));
        }
        if a.marks {
            for m in layout.marks(page) {
                let spans = layout.pages[page].spans(m.first_glyph, m.glyph_count);
                let rects = place.span_rects(&layout, page, font.base_px, &spans);
                under.extend(build_rects(
                    &mut atlas,
                    &rects,
                    [1.0, 0.8, 0.3, 0.25],
                    false,
                ));
            }
        }
        // 文中のアイコン([icon])の絵は利用者が描くので、ここでは場所を枠で示す
        let p = &layout.pages[page];
        for (i, _) in p
            .glyphs
            .iter()
            .enumerate()
            .take(reveal.visible_count())
            .filter(|(_, g)| g.icon.is_some())
        {
            let rects = place.span_rects(&layout, page, font.base_px, &p.spans(i, 1));
            under.extend(build_rects(&mut atlas, &rects, [0.6, 0.9, 0.6, 0.9], true));
        }
        built.quads.splice(0..0, under);
        if a.grid {
            let opts = Overlay::default()
                .max_width(a.layout.max_width)
                .max_lines(a.layout.max_lines);
            built.quads.extend(build_overlay(
                &layout,
                &reveal,
                neko.as_ref(),
                &font,
                &mut atlas,
                &place,
                &opts,
            ));
        }
        let mut px = render(
            &device,
            &queue,
            &mut renderer,
            &mut atlas,
            &built.quads,
            w,
            h,
            a.bg,
        );
        // コマの境目に灰色の線
        for x in 0..w as usize {
            let i = x * 4;
            px[i..i + 4].copy_from_slice(&[96, 96, 96, 255]);
        }
        sheet.extend_from_slice(&px);
        eprintln!(
            "t={t:.2}s ページ {}/{} {} 文字",
            reveal.page() + 1,
            reveal.page_count(),
            reveal.visible_count()
        );
    }
    let rows = h * a.at.len() as u32;
    std::fs::write(&a.out, png(w, rows, &sheet)).expect("書き出せない");
    eprintln!("{} ({w}x{rows})", a.out);
}

#[allow(clippy::too_many_arguments)]
fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut Renderer,
    atlas: &mut Atlas,
    quads: &[nekomoji_wgpu::Quad],
    w: u32,
    h: u32,
    bg: [f32; 3],
) -> Vec<u8> {
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    renderer.prepare(device, queue, atlas, quads, [w as f32, h as f32]);
    let view = target.create_view(&Default::default());
    // 行の長さは 256 バイトの倍数にそろえる
    let row = (w * 4).div_ceil(256) * 256;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (row * h) as u64,
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
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: bg[0] as f64,
                        g: bg[1] as f64,
                        b: bg[2] as f64,
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
                bytes_per_row: Some(row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([enc.finish()]);
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("待てない");
    let mapped = slice.get_mapped_range().expect("読めない");
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as usize {
        let start = y * row as usize;
        out.extend_from_slice(&mapped[start..start + w as usize * 4]);
    }
    out
}

/// RGBA を無圧縮の PNG にする(deflate の非圧縮ブロック)。
fn png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w * 4 + 1) as usize * h as usize);
    for y in 0..h as usize {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * w as usize * 4..(y + 1) * w as usize * 4]);
    }
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    while let Some(c) = chunks.next() {
        z.push(u8::from(chunks.peek().is_none()));
        let len = c.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut s1, mut s2) = (1u32, 0u32);
    for &b in &raw {
        s1 = (s1 + b as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    z.extend_from_slice(&((s2 << 16) | s1).to_be_bytes());

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let crc = crc32(&out[start..]);
        out.extend_from_slice(&crc.to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}
