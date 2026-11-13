// Motion blur: the scene smeared along each pixel's screen motion since the
// last frame, as Black Dragon's RenderMotionBlur does (motionBlurF.glsl,
// LGPL 2.1, Linden Lab / Black Dragon). The motion comes from the depth
// prepass (motion_history.rs) where it drew something, else (sky, water)
// from the camera alone through the depth, as taa.wgsl does.

@group(0) @binding(0) var cur_tex: texture_2d<f32>;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
// xy: the mesh normal, zw: the pixel's screen motion since the last frame
// in uv, without the jitter (object.wgsl depth_normal)
@group(0) @binding(2) var motion_tex: texture_2d<f32>;
@group(0) @binding(3) var lin: sampler;

struct MotionBlurParams {
    inv_view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
    // x: strength (the shutter's share of the frame), y: samples,
    // z: longest smear (uv), w: noise phase of the frame
    params: vec4<f32>,
    // x: 1 when the prepass's motion is read
    motion: vec4<f32>,
};
@group(0) @binding(4) var<uniform> mb: MotionBlurParams;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_mb(@builtin(vertex_index) vi: u32) -> Out {
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    var o: Out;
    o.clip = vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(x, 1.0 - y);
    return o;
}

// A NaN counts as black and an infinite value is held at the largest of
// the format, as in taa.wgsl (#1380): one bad pixel must not spread.
fn safe(c: vec3<f32>) -> vec3<f32> {
    return select(min(c, vec3<f32>(65504.0)), vec3<f32>(0.0), c != c);
}

// Interleaved gradient noise (Jimenez 2014): steady from frame to frame
// up to the phase, so the smear's dither does not crawl.
fn ign(p: vec2<f32>, phase: f32) -> f32 {
    return fract(52.9829189 * fract(dot(p + phase * 5.588238, vec2<f32>(0.06711056, 0.00583715))));
}

@fragment
fn fs_mb(in: Out) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(cur_tex));
    let px = clamp(vec2<i32>(in.clip.xy), vec2<i32>(0), size - vec2<i32>(1));
    let cur = textureLoad(cur_tex, px, 0);
    let depth = textureLoad(depth_tex, px, 0);
    let motion = textureLoad(motion_tex, px, 0).zw;
    var vel = vec2<f32>(0.0);
    if (mb.motion.x > 0.5 && depth > 0.0 && any(motion != vec2<f32>(0.0))) {
        vel = motion;
    } else {
        // the sky, what only the main pass drew and what did not move:
        // the camera alone, through the depth
        let d = max(depth, 1e-7);
        let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
        let wp = mb.inv_view_proj * vec4<f32>(ndc, d, 1.0);
        let world = wp.xyz / wp.w;
        let pp = mb.prev_view_proj * vec4<f32>(world, 1.0);
        if (pp.w > 0.0) {
            let pn = pp.xy / pp.w;
            let puv = vec2<f32>(pn.x * 0.5 + 0.5, 0.5 - pn.y * 0.5);
            vel = in.uv - puv;
        }
    }
    vel = vel * mb.params.x;
    let len = length(vel);
    let reach = mb.params.z;
    if (len > reach) {
        vel = vel * (reach / len);
    }
    // under a pixel of smear there is nothing to blur
    let dims = vec2<f32>(size);
    if (length(vel * dims) < 1.0 || vel.x != vel.x || vel.y != vel.y) {
        return cur;
    }
    let n = i32(clamp(mb.params.y, 2.0, 64.0));
    let jitter = ign(in.clip.xy, mb.params.w) - 0.5;
    var sum = safe(cur.rgb);
    var weight = 1.0;
    for (var i = 0; i < n; i++) {
        let t = (f32(i) + 0.5 + jitter) / f32(n) - 0.5;
        let uv = clamp(in.uv + vel * t, vec2<f32>(0.0), vec2<f32>(1.0));
        sum += safe(textureSampleLevel(cur_tex, lin, uv, 0.0).rgb);
        weight += 1.0;
    }
    // alpha (the glow amount of the frame) stays the pixel's own
    return vec4<f32>(sum / weight, cur.a);
}
