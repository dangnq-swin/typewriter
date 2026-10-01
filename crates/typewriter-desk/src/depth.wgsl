// egui's shading (egui-wgpu's egui.wgsl), with depth: positions come
// already in normalized device coordinates, depth and all.

struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    // sRGB gamma, premultiplied, 0..=1.
    @location(1) color: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

// [u8; 4] sRGB as u32 to [r, g, b, a] in 0..=1.
fn unpack_color(color: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(color & 255u),
        f32((color >> 8u) & 255u),
        f32((color >> 16u) & 255u),
        f32((color >> 24u) & 255u),
    ) / 255.0;
}

// 0..=1 linear from 0..=1 sRGB gamma.
fn linear_from_gamma_rgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let lower = srgb / vec3<f32>(12.92);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

@vertex
fn vs_main(
    @location(0) a_pos: vec3<f32>,
    @location(1) a_tex_coord: vec2<f32>,
    @location(2) a_color: u32,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coord = a_tex_coord;
    out.color = unpack_color(a_color);
    out.position = vec4<f32>(a_pos, 1.0);
    return out;
}

// egui's own texture bind groups: its textures aren't sRGB-aware.
@group(0) @binding(0) var r_tex_color: texture_2d<f32>;
@group(0) @binding(1) var r_tex_sampler: sampler;

@fragment
fn fs_main_linear_framebuffer(in: VertexOutput) -> @location(0) vec4<f32> {
    let gamma = in.color * textureSample(r_tex_color, r_tex_sampler, in.tex_coord);
    return vec4<f32>(linear_from_gamma_rgb(gamma.rgb), gamma.a);
}

@fragment
fn fs_main_gamma_framebuffer(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color * textureSample(r_tex_color, r_tex_sampler, in.tex_coord);
}
