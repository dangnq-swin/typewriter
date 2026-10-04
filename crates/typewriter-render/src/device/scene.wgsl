// The renderer core's scene shader: vertices arrive in a mesh's own
// millimetres, the placement's `r_model` carries them into the frame's
// absolute ones, and the camera's rows project those — the divide makes
// every blend perspective-correct. Each vertex still leaves with the
// absolute millimetres it stands at, so a lamp rather than a direction can
// light each fragment: the lit materials and the shadow reading are T3's
// to land on this layout, and until they do the core draws unlit.

struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    // sRGB gamma, premultiplied, 0..=1.
    @location(1) color: vec4<f32>,
    // Where the vertex stands, in absolute machine millimetres: blended
    // across the triangle like `tex_coord`, perspective-correct through
    // the divide.
    @location(2) mm: vec3<f32>,
    @builtin(position) position: vec4<f32>,
};

// [0] the lamp's head, in machine millimetres, `w` its radius — a disc
// facing the printing point, not a bare point of a bulb; [1] toward the eye,
// unit length, `w` the distance its light is measured even at, 0 for the
// whole room alike; then [2..] chrome's bands, `x` how far down and `yzw`
// its colour in gamma; then the shadow map's rows across [7..9] — machine
// millimetres to its texel space, `x` and `y` in −1..=1, `z` its depth
// 0..=1 — and [10] its parameters: `x` the depth bias, `y` the side in
// texels, 0 for no shadows, `z` the penumbra's scale — the map's depth run
// over its width — and `w` that run, in millimetres. The lamp's own pass
// projects by [7..9] before anything else runs; the lit fragments read the
// rest when they land.
@group(1) @binding(0) var<uniform> r_lighting: array<vec4<f32>, 11>;

// The eye's clip rows, in order x, y, z, w, over `vec4(mm, 1)` absolute
// machine millimetres: the camera's rows as `Camera` carries them, and the
// CPU twin that holds the tests' readbacks divides by hand the same way.
@group(2) @binding(0) var<uniform> r_camera: array<vec4<f32>, 4>;

// The placement's transform, from the mesh's own millimetres into the
// frame's absolute ones: a standing mesh is uploaded once and moved by
// this alone. Dynamic offset, one slot a draw, 256 bytes apart.
@group(2) @binding(1) var<uniform> r_model: mat4x4<f32>;

// What the lamp sees: the depth of the nearest solid standing in each of
// its texels, which the lamp's own pass writes before the frame draws.
@group(3) @binding(0) var r_shadow_map: texture_2d<f32>;

@group(3) @binding(1) var r_shadow_sampler: sampler;

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
    @location(3) a_vector: vec3<f32>,
    @location(4) a_spec: f32,
    @location(5) a_material: u32,
    @location(6) a_shine: u32,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coord = a_tex_coord;
    out.color = unpack_color(a_color);
    // The solid's own millimetres into the frame's absolute ones.
    let mm = (r_model * vec4<f32>(a_pos, 1.0)).xyz;
    out.mm = mm;
    let eye = vec4<f32>(mm, 1.0);
    out.position = vec4<f32>(
        dot(r_camera[0], eye),
        dot(r_camera[1], eye),
        dot(r_camera[2], eye),
        dot(r_camera[3], eye),
    );
    return out;
}

// The same vertices seen from the lamp, standing into the shadow map. The
// rows come in `r_lighting`: across and up its texels in −1..=1, and its
// depth toward the lamp in 0..=1, which is what the map writes. That depth
// row is the projection's third, and every vertex carries `w` = 1, so
// `position.z` arrives at the fragment as exactly the depth — nothing to
// interpolate alongside it. The map is a plain 16-bit float colour
// texture, not a depth one — GL cannot load a depth texture's texels — so
// `fs_lamp` writes the rasterizer's depth straight into its single channel.
struct LampOutput {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_lamp(
    @location(0) a_pos: vec3<f32>,
    @location(1) a_tex_coord: vec2<f32>,
    @location(2) a_color: u32,
    @location(3) a_vector: vec3<f32>,
    @location(4) a_spec: f32,
    @location(5) a_material: u32,
    @location(6) a_shine: u32,
) -> LampOutput {
    let mm = (r_model * vec4<f32>(a_pos, 1.0)).xyz;
    let eye = vec4<f32>(mm, 1.0);
    return LampOutput(vec4<f32>(
        dot(r_lighting[7], eye),
        dot(r_lighting[8], eye),
        dot(r_lighting[9], eye),
        1.0,
    ));
}

@fragment
fn fs_lamp(in: LampOutput) -> @location(0) vec4<f32> {
    let d = clamp(in.position.z, 0.0, 1.0);
    return vec4<f32>(d, 0.0, 0.0, 1.0);
}

// egui's texture shape, kept: a texture and a sampler in group 0, so what
// the app uploads reads as its own does today.
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

// Text lies on what holds it, unlit ink: the placeholder the glyphs grow
// into, and unlike the lit `fs_main` the day it lands.
@fragment
fn fs_text_linear_framebuffer(in: VertexOutput) -> @location(0) vec4<f32> {
    let gamma = in.color * textureSample(r_tex_color, r_tex_sampler, in.tex_coord);
    return vec4<f32>(linear_from_gamma_rgb(gamma.rgb), gamma.a);
}

@fragment
fn fs_text_gamma_framebuffer(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color * textureSample(r_tex_color, r_tex_sampler, in.tex_coord);
}
