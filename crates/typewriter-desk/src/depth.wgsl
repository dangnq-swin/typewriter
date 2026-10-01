// egui's shading (egui-wgpu's egui.wgsl), with depth and light: positions
// come already in normalized device coordinates, depth and all.

struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    // sRGB gamma, premultiplied, 0..=1.
    @location(1) color: vec4<f32>,
    // What the vertex faces (matte, polished) or runs along (streak):
    // blended across the triangle, so not unit length.
    @location(2) vector: vec3<f32>,
    // 0 unlit, 1 matte, 2 polished, 3 streak, 4 chrome. Keep in step with `Shade` in depth.rs.
    @location(3) @interpolate(flat) material: u32,
    // Polished and streak: how narrow the highlight is. Chrome: how far down
    // the plate, 0 at its top.
    @location(4) spec: f32,
    // The material's highlight, packed like `color`. Flat: a part shines one
    // colour all over.
    @location(5) @interpolate(flat) shine: u32,
    @builtin(position) position: vec4<f32>,
};

// [0] toward the light, [1] toward the eye, unit length and `w` unused; then
// [2..] chrome's bands, `x` how far down and `yzw` its colour in gamma.
@group(1) @binding(0) var<uniform> r_lighting: array<vec4<f32>, 7>;

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

// Chrome mirroring the room, `t` of the way down: bright sky above, a band
// where it turns to the light, the dark room below. The bands come from the
// uniform, so `Shade::apply` in depth.rs shades the same plate.
fn chrome(t: f32) -> vec3<f32> {
    let at = clamp(t, 0.0, 1.0);
    for (var i: u32 = 0u; i < 4u; i = i + 1u) {
        let under = r_lighting[2u + i];
        let over = r_lighting[3u + i];
        if at <= over.x {
            return mix(under.yzw, over.yzw, (at - under.x) / (over.x - under.x));
        }
    }
    return r_lighting[6u].yzw;
}

// How bright a thin metal part running along `tangent` catches the light:
// brushed and milled metal streaks along its grain (Heidrich–Seidel).
fn streak_along(tangent: vec3<f32>, sharpness: f32) -> f32 {
    let t = normalize(tangent);
    let lt = dot(t, r_lighting[0].xyz);
    let vt = dot(t, r_lighting[1].xyz);
    let across = sqrt(max(1.0 - lt * lt, 0.0)) * sqrt(max(1.0 - vt * vt, 0.0));
    return pow(max(across - lt * vt, 0.0), sharpness);
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
    out.vector = a_vector;
    out.shine = a_shine;
    out.spec = a_spec;
    out.material = a_material;
    out.position = vec4<f32>(a_pos, 1.0);
    return out;
}

// `gamma` (premultiplied sRGB gamma) as its material takes the light.
fn lit(gamma: vec4<f32>, in: VertexOutput) -> vec4<f32> {
    let light = r_lighting[0].xyz;
    let eye = r_lighting[1].xyz;
    let shine = unpack_color(in.shine).rgb;
    switch in.material {
        // Matte plastic: over an ambient floor (Lambert).
        case 1u: {
            let facing = max(dot(normalize(in.vector), light), 0.0);
            return vec4<f32>(min(gamma.rgb * (0.7 + 0.4 * facing), vec3<f32>(1.0)), gamma.a);
        }
        // Polished metal: lit, then turning to `shine` in a tight highlight
        // (Blinn–Phong).
        case 2u: {
            let normal = normalize(in.vector);
            let facing = max(dot(normal, light), 0.0);
            let highlight = pow(max(dot(normal, normalize(light + eye)), 0.0), in.spec);
            let body = min(gamma.rgb * (0.5 + 0.6 * facing), vec3<f32>(1.0));
            return vec4<f32>(mix(body, shine, highlight), gamma.a);
        }
        // Brushed metal: `shine` streaked along its grain.
        case 3u: {
            let streak = streak_along(in.vector, in.spec);
            return vec4<f32>(mix(gamma.rgb, shine, streak), gamma.a);
        }
        // Chrome: the room, by how far down the plate it lies.
        case 4u: {
            return vec4<f32>(chrome(in.spec), gamma.a);
        }
        default: {}
    }
    return gamma;
}

// egui's own texture bind groups: its textures aren't sRGB-aware.
@group(0) @binding(0) var r_tex_color: texture_2d<f32>;
@group(0) @binding(1) var r_tex_sampler: sampler;

@fragment
fn fs_main_linear_framebuffer(in: VertexOutput) -> @location(0) vec4<f32> {
    let gamma = lit(in.color * textureSample(r_tex_color, r_tex_sampler, in.tex_coord), in);
    return vec4<f32>(linear_from_gamma_rgb(gamma.rgb), gamma.a);
}

@fragment
fn fs_main_gamma_framebuffer(in: VertexOutput) -> @location(0) vec4<f32> {
    return lit(in.color * textureSample(r_tex_color, r_tex_sampler, in.tex_coord), in);
}
