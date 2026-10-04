// egui's shading (egui-wgpu's egui.wgsl), with depth and light: positions
// arrive in absolute machine millimetres and the camera's rows project
// them, so the divide makes every blend perspective-correct; each vertex
// still brings the millimetres it stands at, so a lamp rather than a
// direction can light each fragment — its head a disc, its width felt
// wherever the fragment stands.

struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    // sRGB gamma, premultiplied, 0..=1.
    @location(1) color: vec4<f32>,
    // What the vertex faces (matte, polished) or runs along (streak):
    // blended across the triangle, so not unit length.
    @location(2) vector: vec3<f32>,
    // 0 unlit, 1 matte, 2 polished, 3 streak, 4 chrome. Keep in step with `Shade` in lighting.rs.
    @location(3) @interpolate(flat) material: u32,
    // Polished and streak: how narrow the highlight is. Chrome: how far down
    // the plate, 0 at its top.
    @location(4) spec: f32,
    // The material's highlight, packed like `color`. Flat: a part shines one
    // colour all over.
    @location(5) @interpolate(flat) shine: u32,
    // Where the vertex stands, in machine millimetres from the printing
    // point: blended across the triangle like `vector`, perspective-
    // correctly through the divide.
    @location(6) mm: vec3<f32>,
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
// over its width — and `w` that run, in millimetres.
@group(1) @binding(0) var<uniform> r_lighting: array<vec4<f32>, 11>;

// The eye's clip rows, in order x, y, z, w, over `vec4(x, y, z, 1)` machine
// millimetres: `Eye::camera` builds them, and its own `at` and `depth`
// follow the same maths.
@group(2) @binding(0) var<uniform> r_camera: array<vec4<f32>, 4>;

// What the lamp sees: the depth of the nearest solid standing in each of its
// texels, which the lamp's own pass writes before the frame draws. A plain
// 16-bit float map — not a depth one, GL cannot load a depth texture's
// texels — read through a filtering sampler that blends its texels, as the
// CPU twin blends them by hand.
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

// Chrome mirroring the room, `t` of the way down: bright sky above, a band
// where it turns to the light, the dark room below. The bands come from the
// uniform, so `Shade::apply` in lighting.rs shades the same plate.
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

// How bright a thin metal part running along `tangent` catches light from
// `light`: brushed and milled metal streaks along its grain
// (Heidrich–Seidel). The head's width `ang` broadens the lobe and dims its
// peak by `peak`, as every shine's; `fall` is the share of the lamp's light
// these millimetres stand in, and the mix must not overrun the shine.
fn streak_along(
    tangent: vec3<f32>,
    light: vec3<f32>,
    sharpness: f32,
    ang: f32,
    fall: f32,
) -> f32 {
    let t = normalize(tangent);
    let lt = dot(t, light);
    let vt = dot(t, r_lighting[1].xyz);
    let across = sqrt(max(1.0 - lt * lt, 0.0)) * sqrt(max(1.0 - vt * vt, 0.0));
    let peak = 1.0 / (1.0 + ang * sharpness);
    return min(pow(max(across - lt * vt, 0.0), sharpness * peak) * peak * fall, 1.0);
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
    out.mm = a_pos;
    let mm = vec4<f32>(a_pos, 1.0);
    out.position = vec4<f32>(
        dot(r_camera[0], mm),
        dot(r_camera[1], mm),
        dot(r_camera[2], mm),
        dot(r_camera[3], mm),
    );
    return out;
}

// The same vertices seen from the lamp, standing into the shadow map. The
// rows come in `r_lighting`: across its texels in −1..=1, and its depth
// toward the lamp in 0..=1, which is what the map writes and `shadow_lit`
// compares. That depth row is the projection's third, and every vertex
// carries `w` = 1, so `position.z` arrives at the fragment as exactly the
// depth — nothing to interpolate alongside it. The map is a plain 16-bit
// float colour texture, not a depth one — GL cannot load a depth texture's
// texels — so `fs_lamp` writes the rasterizer's depth straight into its
// single channel.
struct LampOutput {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_lamp(@location(0) a_pos: vec3<f32>) -> LampOutput {
    let mm = vec4<f32>(a_pos, 1.0);
    return LampOutput(vec4<f32>(
        dot(r_lighting[7], mm),
        dot(r_lighting[8], mm),
        dot(r_lighting[9], mm),
        1.0,
    ));
}

@fragment
fn fs_lamp(in: LampOutput) -> @location(0) vec4<f32> {
    let d = clamp(in.position.z, 0.0, 1.0);
    return vec4<f32>(d, 0.0, 0.0, 1.0);
}

// How the shadow's edge is felt: a spiral of taps about the spot, on
// equal-area rings at the golden angle, each spot's own spun by a hash of
// its texel. A fixed pattern — a Poisson disk tapped the same everywhere —
// traces a contour of constant tap-count along every silhouette the disk
// slides across, nested hard arcs where shadows overlap; a spiral spun per
// spot breaks those contours into a dither too fine for the eye. The CPU
// twin in `lighting.rs` computes the very same taps. Sixteen taps: the
// ramp over the half-gap hides the count, and more taps only priced the
// pass up.
const PCF_TAPS: i32 = 16;
const GOLDEN_ANGLE: f32 = 2.39996323;

// How far the disk reaches, in texels: past a texel's own step, so an
// edge is felt as a run across the screen rather than one hard jump.
const PCF_RADIUS: f32 = 1.5;

// How far a penumbra may open past the disk, in texels: near the map's
// half-side. Past a shadow's own width the head is wholly off the caster
// and the shadow has faded anyway; the bound only keeps the disk from
// wandering the map's whole side for nothing.
const PENUMBRA_MAX: f32 = 96.0;

// The map's depth at continuous texel coordinates `xy`, read at explicit
// LOD 0 — no derivatives to mind, in the tap loop or after a return — and
// clamped to the map's edge like the twin blends by hand.
fn shadow_depth(xy: vec2<f32>, side: f32) -> f32 {
    return textureSampleLevel(r_shadow_map, r_shadow_sampler, xy / side, 0.0).r;
}

// How much of the lamp's light reaches `mm`: a spiral of map depths about
// its spot, its reach starting at the disk's own and opening with the
// penumbra — the head's width thrown by how deep this fragment stands
// behind the caster hiding it, capped at `PENUMBRA_MAX`. Each depth ramps
// from dark to light against the depth it stands at: wholly lit within the
// map's bias widened by how fast the map's own depth runs across the disk —
// a face slanted across the lamp's eye mislays its depth by just that much,
// and must not hide itself for it — darkening over the half-gap it stands in
// behind its caster, so the disk's answers blend continuously and the
// head's edge reads as one soft shade, not steps. The caster's own texels, standing nearer the lamp, never widen a
// shadow's own bias. Past the map's edge, and beyond its near and far, the
// spot clamps into it: a caster that reaches the edge keeps hiding what
// lies beyond, and an edge the lamp found clear holds its shadow off. The
// twin of `Shadow::occlusion` in lighting.rs, read alike by the pass and
// `rasterize`.
fn shadow_lit(mm: vec3<f32>) -> f32 {
    let side = r_lighting[10].y;
    if (side <= 0.0) {
        return 1.0;
    }
    let p = vec4<f32>(mm, 1.0);
    let u = clamp(dot(r_lighting[7], p), -1.0, 1.0);
    let v = clamp(dot(r_lighting[8], p), -1.0, 1.0);
    let d = clamp(dot(r_lighting[9], p), 0.0, 1.0);
    let xy = vec2<f32>((u + 1.0) * 0.5 * side, (1.0 - v) * 0.5 * side);
    let base = r_lighting[10].x;
    let centre = shadow_depth(xy, side);
    // The head's width thrown past the caster this fragment stands
    // behind, by similar triangles at the caster's surface: how wide the
    // head looms from there sets how far its rays slip past the edge, the
    // deeper behind it the fragment stands.
    let dist = max(length(r_lighting[0].xyz - mm), 1e-3);
    let deep = max(d - centre, 0.0);
    let span = r_lighting[10].w;
    let to_caster = max(dist - deep * span, 1e-3);
    let reach = PCF_RADIUS + min(
        deep * r_lighting[10].z * side * r_lighting[0].w / to_caster,
        PENUMBRA_MAX,
    );
    let spin = fract(sin(dot(xy, vec2<f32>(12.9898, 78.233))) * 43758.5453) * 6.28318531;
    var stored: array<f32, PCF_TAPS>;
    var slope = 0.0;
    for (var i = 0; i < PCF_TAPS; i = i + 1) {
        let ring = sqrt((f32(i) + 0.5) / f32(PCF_TAPS)) * reach;
        let angle = f32(i) * GOLDEN_ANGLE + spin;
        let there = shadow_depth(xy + vec2<f32>(cos(angle), sin(angle)) * ring, side);
        stored[i] = there;
        if (centre + base >= d && there + base >= d) {
            slope = max(slope, abs(there - centre));
        }
    }
    let bias = base + 2.0 * slope;
    // The fade's width: half the gap this fragment stands in behind its
    // caster, where the head's disk is wholly past the edge; only a floor
    // at the contact, which keeps its old hard answer.
    let soft = max(deep * 0.5 + base, 1e-6);
    var clear = 0.0;
    for (var i = 0; i < PCF_TAPS; i = i + 1) {
        clear = clear + smoothstep(d - bias - soft, d - bias, stored[i]);
    }
    return clear / f32(PCF_TAPS);
}

// `gamma` (premultiplied sRGB gamma), standing `mm` away, as its material
// takes the light from the lamp's head.
fn lit(gamma: vec4<f32>, in: VertexOutput) -> vec4<f32> {
    // From this fragment toward the head's centre: its millimetres come
    // through the divide, so they blend perspective-correctly. The head is
    // a disc, not a point: `ang` is how wide it looms from here, softening
    // every term of the light, and `fall` the share of its lamp-light these
    // millimetres stand in — the pool brighter near the head, thinning away.
    let to_lamp = r_lighting[0].xyz - in.mm;
    let dist = max(length(to_lamp), 1e-3);
    let light = to_lamp / dist;
    let ang = min(r_lighting[0].w / dist, 1.0);
    let wide = 1.0 + ang;
    let even = r_lighting[1].w;
    var fall = 1.0;
    if (even > 0.0) {
        fall = clamp(even * even / (dist * dist), 0.0, 2.5);
    }
    let eye = r_lighting[1].xyz;
    let shine = unpack_color(in.shine).rgb;
    switch in.material {
        // Matte plastic: over an ambient floor, the light wrapping past the
        // terminator the way a wide head's does (wrapped Lambert), of the
        // lamp's light that reaches these millimetres at all.
        case 1u: {
            let facing = max((dot(normalize(in.vector), light) + ang) / wide, 0.0) * shadow_lit(in.mm);
            return vec4<f32>(min(gamma.rgb * (0.7 + 0.4 * facing * fall), vec3<f32>(1.0)), gamma.a);
        }
        // Polished metal: lit, then turning to `shine` in a highlight the
        // head broadens and dims to keep its energy (Blinn–Phong of a wide
        // lobe), the shadow deepening both.
        case 2u: {
            let shade = shadow_lit(in.mm);
            let normal = normalize(in.vector);
            let facing = max((dot(normal, light) + ang) / wide, 0.0) * shade;
            let peak = 1.0 / (1.0 + ang * in.spec);
            let highlight = pow(max(dot(normal, normalize(light + eye)), 0.0), in.spec * peak) * peak * shade * fall;
            let body = min(gamma.rgb * (0.5 + 0.6 * facing * fall), vec3<f32>(1.0));
            return vec4<f32>(mix(body, shine, highlight), gamma.a);
        }
        // Brushed metal: `shine` streaked along its grain, the head's width
        // riding on the streak the same. A feathered decal's rgb is
        // premultiplied, so turn toward `shine` premultiplied too: toward
        // the raw colour its clear halo would shine. The streak keeps its
        // own: the steel bars down the machine's insides are read by their
        // hair of light, shadow or not.
        case 3u: {
            let streak = streak_along(in.vector, light, in.spec, ang, fall);
            return vec4<f32>(mix(gamma.rgb, shine * gamma.a, streak), gamma.a);
        }
        // Chrome: the room, by how far down the plate it lies: it mirrors
        // the whole room, not the lamp's beam alone.
        case 4u: {
            return vec4<f32>(chrome(in.spec), gamma.a);
        }
        // The catcher: the colour it lies out with, shown only as deep as
        // the shadow it stands in.
        case 5u: {
            let by = in.spec * (1.0 - shadow_lit(in.mm));
            return gamma * by;
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
