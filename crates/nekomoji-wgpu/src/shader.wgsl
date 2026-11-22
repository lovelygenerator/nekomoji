struct Screen {
    size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> screen: Screen;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct Instance {
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv_min: vec2<f32>,
    @location(3) uv_max: vec2<f32>,
    @location(4) color: vec4<f32>,
    // x: 回転(ラジアン、中心が軸)、y: 予約(0)
    @location(5) extra: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Instance) -> VsOut {
    // 2枚の三角形で四角形を作る
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi];
    // 中心を軸に回す(画面は y が下向きなので、正の角度は時計回り)
    let half = inst.size * 0.5;
    let local = c * inst.size - half;
    let s = sin(inst.extra.x);
    let k = cos(inst.extra.x);
    let px = inst.pos + half + vec2<f32>(local.x * k - local.y * s, local.x * s + local.y * k);
    var out: VsOut;
    out.clip = vec4<f32>(px.x / screen.size.x * 2.0 - 1.0, 1.0 - px.y / screen.size.y * 2.0, 0.0, 1.0);
    out.uv = mix(inst.uv_min, inst.uv_max, c);
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let cov = textureSample(atlas, atlas_sampler, in.uv).r;
    // 事前乗算なしのアルファ合成
    return vec4<f32>(in.color.rgb, in.color.a * cov);
}
