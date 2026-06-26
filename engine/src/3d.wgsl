// 3D pipeline shader — directional + ambient lighting, vertex colors, optional texture.
//
// group(0) binding(0): CameraUniform (view_proj + light_dir + ambient)
// group(1) binding(0): texture_2d<f32>  — 1x1 white when no texture is provided
// group(1) binding(1): sampler          — NEAREST
// Vertex layout: position(f32x3), normal(f32x3), color(f32x4), uv(f32x2)
//
// White-sentinel: if uv.x < 0.0 the fragment uses tex = vec4(1.0) instead of
// sampling, so meshes with uv = [-1, -1] render as pure vertex-color even when
// an atlas texture is bound.  uv >= 0 meshes are textured as normal.
//
// Final color = tex_col.rgb * vertex_color.rgb * light_factor
// When tex is the 1x1 white fallback (or sentinel), this equals vertex-color output.

struct CameraUniform {
    view_proj: mat4x4<f32>,
    light_dir: vec3<f32>,   // direction *toward* the light (normalized)
    ambient: f32,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;

@group(1) @binding(1)
var s_diffuse: sampler;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) normal:  vec3<f32>,
    @location(1) color:   vec4<f32>,
    @location(2) uv:      vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @location(2) color:    vec4<f32>,
    @location(3) uv:       vec2<f32>,
) -> VsOut {
    var out: VsOut;
    out.clip_pos = camera.view_proj * vec4<f32>(position, 1.0);
    out.normal   = normal;
    out.color    = color;
    out.uv       = uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n       = normalize(in.normal);
    let l       = normalize(camera.light_dir);
    let ndl     = max(dot(n, l), 0.0);
    let light   = camera.ambient + (1.0 - camera.ambient) * ndl;
    // White-sentinel: uv.x < 0 means "no texture" — skip sampling and use white.
    // This lets untextured meshes (uv = [-1, -1]) coexist with textured meshes
    // in the same draw call even when an atlas is bound.
    let tex_col = select(textureSample(t_diffuse, s_diffuse, in.uv), vec4<f32>(1.0), in.uv.x < 0.0);
    let rgb     = tex_col.rgb * in.color.rgb * light;
    return vec4<f32>(rgb, in.color.a * tex_col.a);
}
