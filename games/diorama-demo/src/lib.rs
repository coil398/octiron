//! Phase 1 visual proof of the 3D rendering foundation.
//!
//! A single cube rotates slowly under a tilted diorama camera
//! (elevation ~35°).  Directional + ambient lighting is applied in the shader.
//! No 2D HUD is shown; the 2D pass is still there but submits 0 quads.

use octiron::{Assets, CameraUniform, Frame, Game, Scene3D, World, cube_mesh, diorama_view, proj_persp};

#[cfg(target_arch = "wasm32")]
use octiron::{Config, Vec2, run_with};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

struct DioramaDemo {
    angle: f32,
}

impl Default for DioramaDemo {
    fn default() -> Self {
        DioramaDemo { angle: 0.0 }
    }
}

impl Game for DioramaDemo {
    fn start(&mut self, _world: &mut World, _assets: &mut Assets) {
        // No 2D entities needed.
    }

    fn update(&mut self, _world: &mut World, frame: &Frame) {
        // Rotate the cube at ~30 degrees per second.
        self.angle += frame.dt * 30_f32.to_radians();
    }

    fn scene_3d(&self) -> Option<Scene3D> {
        use glam::{Mat4, Vec3};

        // Model transform: rotate around Y.
        let model = Mat4::from_rotation_y(self.angle);

        // Build the cube mesh and apply the model matrix to each vertex.
        let color = [0.72, 0.53, 0.34, 1.0]; // warm terracotta
        let (raw_verts, indices) = cube_mesh(1.2, color);

        let vertices: Vec<octiron::Vertex3D> = raw_verts
            .iter()
            .map(|v| {
                let pos4 = model * glam::Vec4::new(v.position[0], v.position[1], v.position[2], 1.0);
                let nor4 = model * glam::Vec4::new(v.normal[0], v.normal[1], v.normal[2], 0.0);
                octiron::Vertex3D {
                    position: [pos4.x, pos4.y, pos4.z],
                    normal: [nor4.x, nor4.y, nor4.z],
                    color: v.color,
                    uv: v.uv,
                }
            })
            .collect();

        // Diorama camera: elevation ~35°, slight azimuth offset, distance 5.
        let elevation = 35_f32.to_radians();
        let azimuth = 30_f32.to_radians();
        let view = diorama_view(Vec3::ZERO, elevation, azimuth, 5.0);
        let proj = proj_persp(800.0 / 600.0, 45_f32.to_radians(), 0.1, 100.0);
        let view_proj = (proj * view).to_cols_array_2d();

        // Directional light pointing down-left-front (normalized).
        let light_dir = Vec3::new(-0.4, 0.8, 0.4).normalize();

        let camera = CameraUniform {
            view_proj,
            light_dir: light_dir.to_array(),
            ambient: 0.25,
        };

        Some(Scene3D {
            camera,
            meshes: vec![(vertices, indices)],
            texture: None,
            sky: None,
        })
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    run_with(
        DioramaDemo::default(),
        Config {
            clear_color: [0.13, 0.15, 0.22, 1.0],
            logical_size: Vec2::new(800.0, 600.0),
        },
    );
}
