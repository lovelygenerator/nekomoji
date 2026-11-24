//! wgpu のパイプラインと描画。

use crate::atlas::Atlas;
use crate::quads::Quad;
use wgpu::util::DeviceExt;

/// 画面(サーフェス)の形式を選ぶ。sRGB の付かない 8bit の形式
/// ([`Bgra8Unorm`](wgpu::TextureFormat::Bgra8Unorm)・[`Rgba8Unorm`](wgpu::TextureFormat::Rgba8Unorm))を、
/// `formats`([`Surface::get_capabilities`](wgpu::Surface::get_capabilities) の `formats`)の順で優先する。
///
/// nekomoji の色(`[color]`・縁取り・影・発光・背景)は `#rrggbb` をそのまま画面の値として扱う。
/// sRGB の形式に描くと、書き込むときに線形の値とみなして変換され、色が明るく薄くなる。
/// [`Surface::get_default_config`](wgpu::Surface::get_default_config) は WebGL2・ネイティブでは
/// sRGB の形式を先に選ぶので、そのままでは sRGB の形式を持たない WebGPU と見た目が変わる。
/// sRGB の付かない形式が無ければ先頭の形式を返す
/// (そのときは色が明るく出る)。`formats` が空なら `None`。
///
/// ```ignore
/// let mut config = surface.get_default_config(&adapter, w, h)?;
/// if let Some(f) = nekomoji_wgpu::surface_format(&surface.get_capabilities(&adapter).formats) {
///     config.format = f;
/// }
/// ```
pub fn surface_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    use wgpu::TextureFormat::{Bgra8Unorm, Rgba8Unorm};
    formats
        .iter()
        .copied()
        .find(|f| matches!(f, Bgra8Unorm | Rgba8Unorm))
        .or_else(|| formats.first().copied())
}

/// アトラスの一辺の既定(px)。[`Atlas::new`]・[`Renderer::new`] に同じ値を渡す。
/// 足りなくなれば [`Atlas::make_room`] が大きくするので、最初は小さくてよい。
pub const DEFAULT_ATLAS_SIZE: u32 = 1024;

/// サーフェスの設定を作る。大きさ `size`(幅, 高さ。0 は 1 にする)と、色が明るくならない形式
/// ([`surface_format`])を選ぶ。そのまま `surface.configure(&device, &config)` に渡せる。
/// サーフェスがこのアダプタで使えなければ `None`。
///
/// 窓の大きさが変わったら、同じ関数で作り直して `configure` する。
/// [`Renderer::new`] には `config.format` を渡す。
pub fn surface_config(
    surface: &wgpu::Surface<'_>,
    adapter: &wgpu::Adapter,
    size: [u32; 2],
) -> Option<wgpu::SurfaceConfiguration> {
    let mut config = surface.get_default_config(adapter, size[0].max(1), size[1].max(1))?;
    if let Some(f) = surface_format(&surface.get_capabilities(adapter).formats) {
        config.format = f;
    }
    Some(config)
}

/// wgpu の描画パイプライン。[`Renderer::prepare`] で四角形とアトラスを送り、[`Renderer::draw`] で描く。
pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    bind_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    texture: wgpu::Texture,
    atlas_size: u32,
    /// 文中のアイコンの絵(RGBA)。絵を登録するまでは 1px の空のテクスチャ
    icon_texture: wgpu::Texture,
    icon_size: u32,
    uniform: wgpu::Buffer,
    instances: wgpu::Buffer,
    capacity: usize,
    count: u32,
}

impl Renderer {
    /// `format` は描画先(サーフェス)の形式、`atlas_size` はアトラスの一辺(px)。
    /// アトラスの大きさが変わったら([`Atlas::make_room`])、[`Renderer::prepare`] がテクスチャを
    /// 作り直す。
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, atlas_size: u32) -> Self {
        let texture = Self::atlas_texture(device, atlas_size);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let icon_size = 1;
        let icon_texture = Self::icon_texture(device, icon_size);
        let icon_view = icon_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("nekomoji screen"),
            contents: bytemuck::cast_slice(&[1.0f32, 1.0, 0.0, 0.0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("nekomoji bind layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = Self::bind_group(device, &layout, &uniform, &view, &icon_view, &sampler);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("nekomoji shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("nekomoji pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        const ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
            0 => Float32x2, 1 => Float32x2, 2 => Float32x2, 3 => Float32x2, 4 => Float32x4,
            5 => Float32x2
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("nekomoji pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Quad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &ATTRS,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let capacity = 256;
        let instances = Self::instance_buffer(device, capacity);
        Self {
            pipeline,
            bind_group,
            bind_layout: layout,
            sampler,
            texture,
            atlas_size,
            icon_texture,
            icon_size,
            uniform,
            instances,
            capacity,
            count: 0,
        }
    }

    /// アトラスの一辺の上限の目安。端末の上限(`max_texture_dimension_2d`)と 8192 の小さいほう
    /// (8192 で 64MB)。[`Atlas::make_room`] に渡す。
    pub fn max_atlas_size(device: &wgpu::Device) -> u32 {
        device.limits().max_texture_dimension_2d.min(8192)
    }

    fn atlas_texture(device: &wgpu::Device, size: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("nekomoji atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    fn icon_texture(device: &wgpu::Device, size: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("nekomoji icons"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    fn bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
        atlas: &wgpu::TextureView,
        icons: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("nekomoji bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(atlas),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(icons),
                },
            ],
        })
    }

    fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("nekomoji instances"),
            size: (capacity * std::mem::size_of::<Quad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// アトラスの新しい文字と、新しく登録したアイコンの絵を転送し、四角形を頂点バッファへ書く。
    /// 描画の前に毎フレーム呼ぶ。
    ///
    /// アトラスの大きさがテクスチャと違えば、テクスチャを作り直してから転送する
    /// (アイコンのアトラスも同じ。絵を登録していないあいだは 1px の空のテクスチャ)。
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &mut Atlas,
        quads: &[Quad],
        screen: [f32; 2],
    ) {
        let icon_wanted = atlas.icons().size().max(1);
        let rebuilt = atlas.size() != self.atlas_size || icon_wanted != self.icon_size;
        if atlas.size() != self.atlas_size {
            self.atlas_size = atlas.size();
            self.texture = Self::atlas_texture(device, self.atlas_size);
        }
        if icon_wanted != self.icon_size {
            // 大きさが変わると、絵はすべて置き直されて転送待ちになっている
            self.icon_size = icon_wanted;
            self.icon_texture = Self::icon_texture(device, self.icon_size);
        }
        if rebuilt {
            let view = self
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let icon_view = self
                .icon_texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            self.bind_group = Self::bind_group(
                device,
                &self.bind_layout,
                &self.uniform,
                &view,
                &icon_view,
                &self.sampler,
            );
        }
        for u in atlas.take_uploads() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: u.x,
                        y: u.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &u.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(u.w),
                    rows_per_image: Some(u.h),
                },
                wgpu::Extent3d {
                    width: u.w,
                    height: u.h,
                    depth_or_array_layers: 1,
                },
            );
        }
        for u in atlas.icons_mut().take_uploads() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.icon_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: u.x,
                        y: u.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &u.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(u.w * 4),
                    rows_per_image: Some(u.h),
                },
                wgpu::Extent3d {
                    width: u.w,
                    height: u.h,
                    depth_or_array_layers: 1,
                },
            );
        }
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(&[screen[0], screen[1], 0.0, 0.0]),
        );
        if quads.len() > self.capacity {
            self.capacity = quads.len().next_power_of_two();
            self.instances = Self::instance_buffer(device, self.capacity);
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(quads));
        self.count = quads.len() as u32;
    }

    /// 描画パスへ積む。
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..self.count);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn シェーダーを_naga_で検証できる() {
        use wgpu::naga;
        let module = naga::front::wgsl::parse_str(include_str!("shader.wgsl")).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
    }

    use super::surface_format;
    use wgpu::TextureFormat as F;

    #[test]
    fn 画面の形式は_srgb_でない_8bit_の形式を選ぶ() {
        // wgpu は sRGB の形式を先に並べる
        let formats = [
            F::Bgra8UnormSrgb,
            F::Rgba8UnormSrgb,
            F::Rgba16Float,
            F::Bgra8Unorm,
            F::Rgba8Unorm,
        ];
        assert_eq!(surface_format(&formats), Some(F::Bgra8Unorm));
        assert_eq!(
            surface_format(&[F::Rgba8UnormSrgb, F::Rgba8Unorm]),
            Some(F::Rgba8Unorm)
        );
    }

    #[test]
    fn 選べる形式が無ければ先頭を返す() {
        assert_eq!(
            surface_format(&[F::Rgba8UnormSrgb]),
            Some(F::Rgba8UnormSrgb)
        );
        assert_eq!(surface_format(&[]), None);
    }
}
