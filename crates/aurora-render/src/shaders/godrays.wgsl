// Volumetric lighting (godrays): Black Dragon's volumetricLightF.glsl
// (RenderVolumetricLighting, LGPL 2.1, Linden Lab / Black Dragon). Each
// pixel's ray from the camera is walked in steps through the sun's shadow
// cascades; the light the steps see, and where they cross a shadow's edge,
// is added to the pixel in the sun's colour.

@group(0) @binding(0) var cur_tex: texture_2d<f32>;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
// atlas of 2x2 tiles, cascade i in tile (i % 2, i / 2) (common.wgsl)
@group(0) @binding(2) var shadow_map: texture_depth_2d;
@group(0) @binding(3) var shadow_sampler: sampler_comparison;

struct GodrayParams {
    inv_view_proj: mat4x4<f32>,
    cascade_vp: array<mat4x4<f32>, 3>,
    splits: vec4<f32>,     // the three cascades' far ends (m from the camera)
    camera_pos: vec4<f32>,
    sun_color: vec4<f32>,  // rgb: the sun's light, w: how visible the sun is
    // x: strength (godray_multiplier), y: steps (godray_res), z: falloff
    // (falloff_multiplier), w: haze weight (haze / (blue + haze) of the sky)
    params: vec4<f32>,
    noise: vec4<f32>,      // x: phase of the frame
};
@group(0) @binding(4) var<uniform> gr: GodrayParams;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_gr(@builtin(vertex_index) vi: u32) -> Out {
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    var o: Out;
    o.clip = vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(x, 1.0 - y);
    return o;
}

// 1 lit, 0 shadowed, 1 where no cascade reaches (nonpcfShadowAtPos: one
// tap, no filter).
fn sun_lit(p: vec3<f32>) -> f32 {
    let d = distance(p, gr.camera_pos.xyz);
    var cascade = 0u;
    if (d > gr.splits.x) { cascade = 1u; }
    if (d > gr.splits.y) { cascade = 2u; }
    if (d > gr.splits.z) {
        return 1.0;
    }
    let lp = gr.cascade_vp[cascade] * vec4<f32>(p, 1.0);
    let ndc = lp.xyz / lp.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }
    let atlas = vec2<f32>(textureDimensions(shadow_map));
    let tile = vec2<f32>(f32(cascade % 2u), f32(cascade / 2u));
    let tile_scale = vec2<f32>(0.5, atlas.x / (2.0 * atlas.y));
    let texel = 2.0 / atlas.x;
    let t = clamp(uv, vec2<f32>(0.5 * texel), vec2<f32>(1.0 - 0.5 * texel));
    return textureSampleCompareLevel(shadow_map, shadow_sampler, (t + tile) * tile_scale, ndc.z - 0.0005);
}

fn rand(co: vec2<f32>, phase: f32) -> f32 {
    return fract(sin(dot(co + phase * 1.618, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn fs_gr(in: Out) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(cur_tex));
    let px = clamp(vec2<i32>(in.clip.xy), vec2<i32>(0), size - vec2<i32>(1));
    let cur = textureLoad(cur_tex, px, 0);
    if (gr.sun_color.w <= 0.0) {
        return cur;
    }
    // reverse-Z: 0 is infinitely far (the sky reaches the 512 m limit)
    let depth = max(textureLoad(depth_tex, px, 0), 1e-7);
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let wp = gr.inv_view_proj * vec4<f32>(ndc, depth, 1.0);
    let world = wp.xyz / wp.w;
    let to_pixel = world - gr.camera_pos.xyz;
    let dist = max(length(to_pixel), 1e-3);
    let reach = min(dist, 512.0);
    let far_pos = gr.camera_pos.xyz + to_pixel * (reach / dist);

    // BD weighs the pixel's depth as its hardware depth to the 101st
    // power, which is nothing but near the far plane; for a near plane of
    // 0.1 m that is exp(-10 / distance)
    let weight_depth = exp(-10.0 / max(dist, 0.1));

    let n = i32(clamp(gr.params.y, 4.0, 128.0));
    let roffset = rand(in.clip.xy, gr.noise.x);
    var shadamount = 0.0;
    var shaftify = 0.0;
    var last = 0.0;
    for (var i = n - 1; i > 0; i--) {
        let p = mix(gr.camera_pos.xyz, far_pos, (f32(i) - roffset) / f32(n));
        var this_shad = 0.275 * sun_lit(p);
        var this_shaft = 0.15 * abs(this_shad + last);
        last = this_shad;
        this_shad *= f32(i);
        this_shaft /= f32(i);
        shadamount += this_shad;
        shaftify += this_shaft;
    }
    shadamount /= f32(n);
    shaftify /= f32(n);
    shadamount *= clamp(weight_depth, 0.0, 0.5);
    let fade = max(gr.params.z / weight_depth, 1.0);
    shaftify = (shaftify / fade) * gr.params.x;
    let add = (shaftify * gr.params.w) * shadamount * gr.sun_color.rgb * gr.sun_color.w;
    return vec4<f32>(cur.rgb + max(add, vec3<f32>(0.0)), cur.a);
}
