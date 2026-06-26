//! 3D math helpers for the diorama rendering pipeline.
//!
//! Provides vertex layout ([`Vertex3D`]), camera uniform ([`CameraUniform`]),
//! camera matrix helpers, and a cube mesh generator.

use glam::{Mat4, Vec3};

/// A single vertex in the 3D pipeline: position, normal, RGBA color, and UV.
///
/// Matches `@location(0..3)` in `3d.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex3D {
    /// Object-space position (x, y, z).
    pub position: [f32; 3],
    /// Object-space surface normal (x, y, z) — expected to be unit length.
    pub normal: [f32; 3],
    /// RGBA vertex color in 0.0..=1.0.
    pub color: [f32; 4],
    /// Texture UV coordinates in 0.0..=1.0.  Defaults to `[0.0, 0.0]` for
    /// untextured meshes; the shader multiplies tex(uv) * color so a white
    /// fallback texture leaves the output identical to the old pipeline.
    pub uv: [f32; 2],
}

impl Vertex3D {
    pub const ATTRS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3,  // position
        1 => Float32x3,  // normal
        2 => Float32x4,  // color
        3 => Float32x2,  // uv
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex3D>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRS,
        }
    }
}

/// Camera + lighting uniform uploaded once per frame to the 3D pipeline.
///
/// Layout:
/// - `view_proj`: column-major 4x4 matrix (matches WGSL `mat4x4<f32>`).
///   Produced by glam, which is column-major, so `bytemuck::bytes_of` works
///   directly.
/// - `light_dir`: world-space direction *toward* the light (normalized).
///   The shader uses `dot(normal, light_dir)` for Lambertian shading.
/// - `ambient`: scalar ambient intensity (0.0..=1.0).
/// - `_padding`: 3 f32 to pad `light_dir` + `ambient` to 16 bytes (std140).
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    /// Column-major view-projection matrix.
    pub view_proj: [[f32; 4]; 4],
    /// World-space direction toward the directional light (normalized).
    pub light_dir: [f32; 3],
    /// Ambient light intensity.
    pub ambient: f32,
}

impl CameraUniform {
    /// Creates a default (identity view_proj, no directional light) uniform.
    pub fn identity() -> Self {
        CameraUniform {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            light_dir: [0.0, 1.0, 0.0],
            ambient: 0.3,
        }
    }
}

// ── Camera helper functions ──────────────────────────────────────────────────

/// Creates a view matrix for a diorama-style orbiting camera.
///
/// The camera orbits a `target` point at the given `distance`, pitched up by
/// `elevation_rad` from the horizontal plane and rotated around the vertical
/// axis by `azimuth_rad`.  Uses `look_at_rh` (right-handed, NDC depth [0,1]).
pub fn diorama_view(
    target: Vec3,
    elevation_rad: f32,
    azimuth_rad: f32,
    distance: f32,
) -> Mat4 {
    let sin_el = elevation_rad.sin();
    let cos_el = elevation_rad.cos();
    let sin_az = azimuth_rad.sin();
    let cos_az = azimuth_rad.cos();

    let eye = target
        + Vec3::new(
            distance * cos_el * sin_az,
            distance * sin_el,
            distance * cos_el * cos_az,
        );

    Mat4::look_at_rh(eye, target, Vec3::Y)
}

/// Orthographic projection (right-handed, NDC depth [0,1]).
///
/// `w` / `h` are the half-extents of the view volume in world units.
pub fn proj_ortho(w: f32, h: f32, near: f32, far: f32) -> Mat4 {
    Mat4::orthographic_rh(-w, w, -h, h, near, far)
}

/// Perspective projection (right-handed, NDC depth [0,1]).
///
/// `fov_y` is the full vertical field of view in radians.
pub fn proj_persp(aspect: f32, fov_y: f32, near: f32, far: f32) -> Mat4 {
    Mat4::perspective_rh(fov_y, aspect, near, far)
}

// ── Mesh helpers ─────────────────────────────────────────────────────────────

/// Generates a textured-color cube mesh centred at the origin.
///
/// Returns `(vertices, indices)`.  Each of the 6 faces has its own 4 vertices
/// so per-face normals are correct (no shared vertices across faces).
/// Indices form triangle pairs (CCW winding, front face = counter-clockwise).
pub fn cube_mesh(size: f32, color: [f32; 4]) -> (Vec<Vertex3D>, Vec<u32>) {
    let h = size * 0.5;

    // (normal, 4 corner positions in CCW order as seen from outside)
    let faces: [(Vec3, [[f32; 3]; 4]); 6] = [
        // +X right
        (
            Vec3::X,
            [
                [h, -h, -h],
                [h, h, -h],
                [h, h, h],
                [h, -h, h],
            ],
        ),
        // -X left
        (
            -Vec3::X,
            [
                [-h, -h, h],
                [-h, h, h],
                [-h, h, -h],
                [-h, -h, -h],
            ],
        ),
        // +Y top
        (
            Vec3::Y,
            [
                [-h, h, h],
                [h, h, h],
                [h, h, -h],
                [-h, h, -h],
            ],
        ),
        // -Y bottom
        (
            -Vec3::Y,
            [
                [-h, -h, -h],
                [h, -h, -h],
                [h, -h, h],
                [-h, -h, h],
            ],
        ),
        // +Z front
        (
            Vec3::Z,
            [
                [-h, -h, h],
                [h, -h, h],
                [h, h, h],
                [-h, h, h],
            ],
        ),
        // -Z back
        (
            -Vec3::Z,
            [
                [h, -h, -h],
                [-h, -h, -h],
                [-h, h, -h],
                [h, h, -h],
            ],
        ),
    ];

    let mut vertices: Vec<Vertex3D> = Vec::with_capacity(24);
    let mut indices: Vec<u32> = Vec::with_capacity(36);

    for (normal, corners) in &faces {
        let base = vertices.len() as u32;
        for pos in corners {
            vertices.push(Vertex3D {
                position: *pos,
                normal: normal.to_array(),
                color,
                uv: [0.0, 0.0],
            });
        }
        // Two triangles: (0,1,2) and (0,2,3) — CCW.
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    (vertices, indices)
}
