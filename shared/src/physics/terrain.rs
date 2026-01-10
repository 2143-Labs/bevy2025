use avian3d::prelude::*;
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use noise::{NoiseFn, Perlin};

use serde::{Deserialize, Serialize};

/// Marker for terrain entity
#[derive(Component)]
pub struct Terrain;

/// Marker for boundary walls
#[derive(Component)]
pub struct BoundaryWall;

/// Terrain generation parameters
#[derive(Debug, Clone, Serialize, Deserialize, Message, Resource)]
pub struct TerrainParams {
    pub seed: u32,
    pub plane_size: f32,
    pub subdivisions: u32,
    pub max_height_delta: f32,
}

impl Default for TerrainParams {
    fn default() -> Self {
        let seed = fastrand::u32(..);
        Self {
            seed,
            plane_size: 250.0,
            subdivisions: 250,
            max_height_delta: 6.0,
        }
    }
}

pub const NOISE_SCALE_FACTOR: f64 = 0.05;

pub struct TerrainPerlin {
    perlin_a: Perlin,
    perlin_b: Perlin,
    perlin_c: Perlin,
}

impl TerrainParams {
    pub fn perlin(&self) -> TerrainPerlin {
        TerrainPerlin {
            perlin_a: Perlin::new(self.seed),
            perlin_b: Perlin::new(self.seed.wrapping_add(1)),
            perlin_c: Perlin::new(self.seed.wrapping_add(1)),
        }
    }
}

impl TerrainPerlin {
    pub fn sample_height(&self, x: f32, z: f32) -> f32 {
        let noise_x = x as f64 * NOISE_SCALE_FACTOR;
        let noise_z = z as f64 * NOISE_SCALE_FACTOR;
        let remap_x = self.perlin_a.get([noise_x, noise_z]);
        let remap_z = self.perlin_b.get([noise_x, noise_z]);
        let final_noise = self.perlin_c.get([remap_x, remap_z]);
        final_noise as f32
    }
}

/// Spawn invisible boundary walls around the terrain to keep balls from falling off
pub fn spawn_boundary_walls(commands: &mut Commands, params: &TerrainParams) -> [Entity; 4] {
    let size = params.plane_size;
    let wall_height = size; // Height matches terrain size (100 units)
    let wall_thickness = 0.1; // Very thin planes
    let half_size = size * 0.5;

    // Base at lowest possible terrain point
    let base_y = -params.max_height_delta;
    let center_y = base_y + wall_height / 2.0;

    // North wall (positive Z) - plane perpendicular to Z axis
    let e1 = commands
        .spawn((
            Name::new("Boundary Wall (North)"),
            Transform::from_xyz(0.0, center_y, half_size),
            RigidBody::Static,
            Collider::cuboid(size, wall_height, wall_thickness),
            BoundaryWall,
        ))
        .id();

    // South wall (negative Z) - plane perpendicular to Z axis
    let e2 = commands
        .spawn((
            Name::new("Boundary Wall (South)"),
            Transform::from_xyz(0.0, center_y, -half_size),
            RigidBody::Static,
            Collider::cuboid(size, wall_height, wall_thickness),
            BoundaryWall,
        ))
        .id();

    // East wall (positive X) - plane perpendicular to X axis
    let e3 = commands
        .spawn((
            Name::new("Boundary Wall (East)"),
            Transform::from_xyz(half_size, center_y, 0.0),
            RigidBody::Static,
            Collider::cuboid(wall_thickness, wall_height, size),
            BoundaryWall,
        ))
        .id();

    // West wall (negative X) - plane perpendicular to X axis
    let e4 = commands
        .spawn((
            Name::new("Boundary Wall (West)"),
            Transform::from_xyz(-half_size, center_y, 0.0),
            RigidBody::Static,
            Collider::cuboid(wall_thickness, wall_height, size),
            BoundaryWall,
        ))
        .id();

    [e1, e2, e3, e4]
}

/// Generate procedural terrain mesh using Perlin noise
pub fn generate_terrain_mesh(params: &TerrainParams) -> Mesh {
    let noise = params.perlin();
    let subdivisions = params.subdivisions;
    let size = params.plane_size;
    let height_scale = params.max_height_delta;

    // Calculate vertex count
    let vertices_per_side = subdivisions + 1;
    let vertex_count = (vertices_per_side * vertices_per_side) as usize;

    // Generate vertices
    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut uvs = Vec::with_capacity(vertex_count);

    let step = size / subdivisions as f32;
    let half_size = size * 0.5;

    for z in 0..vertices_per_side {
        for x in 0..vertices_per_side {
            let x_pos = x as f32 * step - half_size;
            let z_pos = z as f32 * step - half_size;

            // Generate height using Perlin noise
            let height = noise.sample_height(x_pos, z_pos) as f32 * height_scale;

            positions.push([x_pos, height, z_pos]);
            normals.push([0.0, 1.0, 0.0]); // Will recalculate proper normals
            uvs.push([
                x as f32 / subdivisions as f32,
                z as f32 / subdivisions as f32,
            ]);
        }
    }

    // Generate indices for triangles
    let mut indices = Vec::new();
    for z in 0..subdivisions {
        for x in 0..subdivisions {
            let i = z * vertices_per_side + x;

            // Two triangles per quad
            indices.push(i);
            indices.push(i + vertices_per_side);
            indices.push(i + 1);

            indices.push(i + 1);
            indices.push(i + vertices_per_side);
            indices.push(i + vertices_per_side + 1);
        }
    }

    // Calculate proper normals
    let mut calculated_normals = vec![Vec3::ZERO; vertex_count];

    for triangle in indices.chunks(3) {
        let i0 = triangle[0] as usize;
        let i1 = triangle[1] as usize;
        let i2 = triangle[2] as usize;

        let v0 = Vec3::from(positions[i0]);
        let v1 = Vec3::from(positions[i1]);
        let v2 = Vec3::from(positions[i2]);

        let normal = (v1 - v0).cross(v2 - v0).normalize();

        calculated_normals[i0] += normal;
        calculated_normals[i1] += normal;
        calculated_normals[i2] += normal;
    }

    // Normalize accumulated normals
    for normal in &mut calculated_normals {
        *normal = normal.normalize();
    }

    // Convert back to array format
    for (i, normal) in calculated_normals.iter().enumerate() {
        normals[i] = normal.to_array();
    }

    // Create mesh
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}
