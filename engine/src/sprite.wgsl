// Instanced textured-quad shader.
//
// A unit quad (vertex buffer 0) is drawn once per instance (vertex buffer 1).
// Each instance carries a pixel-space offset/size, a UV sub-rect into the bound
// texture, and a tint color. Solid fills bind a 1x1 white texture so the same
// shader covers rects, sprites, and text.

struct Resolution {
    size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> resolution: Resolution;

@group(1) @binding(0) var t_sprite: texture_2d<f32>;
@group(1) @binding(1) var s_sprite: sampler;

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) quad_pos: vec2<f32>,   // unit quad, 0.0..=1.0
    @location(1) offset: vec2<f32>,     // instance top-left, pixels
    @location(2) size: vec2<f32>,       // instance size, pixels
    @location(3) uv_min: vec2<f32>,
    @location(4) uv_max: vec2<f32>,
    @location(5) color: vec4<f32>,
) -> VsOut {
    let pixel = offset + quad_pos * size;
    var clip = (pixel / resolution.size) * 2.0 - vec2<f32>(1.0, 1.0);
    clip.y = -clip.y;

    var out: VsOut;
    out.clip_position = vec4<f32>(clip, 0.0, 1.0);
    out.uv = mix(uv_min, uv_max, quad_pos);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(t_sprite, s_sprite, in.uv) * in.color;
}
