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
            plane_size: 1000.0,
            subdivisions: 500,
            max_height_delta: 1.0,
        }
    }
}

pub const NOISE_SCALE_FACTOR: f64 = 0.05;

/// function pointers are used for efficiency in sampling noise functions
type TerrainPerlinMap2d = fn(f32, f32, &Perlin) -> f32;
/// function pointers are used for efficiency in sampling noise functions
type TerrainPerlinMap3d = fn(f32, f32, f32, &Perlin) -> f32;

/// Terrain similar to Minecraft's use of Perlin noise with multiple octaves
pub struct TerrainPerlin {
    /// 2d heightmap given x,z position output (-1.0 to 1.0)
    perlin_continentalness: Perlin,
    transform_continentalness: TerrainPerlinMap2d,
    /// 2d heightmap given x,z position output (-1.0 to 1.0)
    perlin_erosion: Perlin,
    transform_erosion: TerrainPerlinMap2d,
    /// 2d heightmap given x,z position output (-1.0 to 1.0)
    perlin_peaks_valleys: Perlin,
    transform_peaks_valleys: TerrainPerlinMap2d,
    /// 3d world density given 3d position output (-1.0 to 1.0)
    perlin_density: Perlin,
    transform_density: TerrainPerlinMap3d,
}

pub struct Spline<'a> {
    points: &'a [(f32, f32)],
}

impl<'a> Spline<'a> {
    pub const fn new(points: &'a [(f32, f32)]) -> Self {
        Self { points }
    }

    pub fn sample(&self, x: f32) -> f32 {
        if self.points.is_empty() {
            return 0.0;
        }
        if x <= self.points[0].0 {
            return self.points[0].1;
        }
        if x >= self.points[self.points.len() - 1].0 {
            return self.points[self.points.len() - 1].1;
        }
        for i in 0..self.points.len() - 1 {
            let (x0, y0) = self.points[i];
            let (x1, y1) = self.points[i + 1];
            if x0 <= x && x <= x1 {
                let t = (x - x0) / (x1 - x0);
                return y0 + t * (y1 - y0);
            }
        }
        0.0
    }
}

// https://i.devolved.us/XWUU.png
const STANDARD_CONTINENTALNESS_INPUT_SCALE: f32 = 0.025;
const STANDARD_CONTINENTALNESS_SPLINE: Spline = Spline {
    points: &[
        // use this to create islands
        (-1.0, 1.0),
        (-0.95, -1.0),
        // ocean
        (-0.5, -1.0),
        (-0.45, -0.2),
        (-0.2, -0.2),
        // shore + land
        (-0.15, 0.8),
        (0.0, 0.82),
        (0.15, 0.85),
        (0.25, 0.90),
        (0.80, 0.95),
        (1.00, 1.00),
    ],
};

const STANDARD_EROSION_INPUT_SCALE: f32 = 0.015;
const STANDARD_EROSION_SPLINE: Spline = Spline {
    points: &[
        (-1.0, 1.0),
        (-0.8, 0.6),
        (-0.5, 0.0),
        (-0.45, 0.2),
        (-0.2, -0.5),
        (0.0, -0.7),
        (0.3, -0.8),
        (0.6, -0.85),
        (0.7, -0.4),
        (0.78, -0.4),
        (0.85, -0.85),
        (0.95, -0.95),
        (0.99, -1.00),
    ],
};

const STANDARD_PEAKS_VALLEYS_INPUT_SCALE: f32 = 1.0;
const STANDARD_PEAKS_VALLEYS_SPLINE: Spline = Spline {
    points: &[
        (-1.0, -1.0),
        (-0.9, -0.8),
        (-0.8, -0.6),
        (-0.5, -0.5),
        (0.0, -0.4),
        (0.5, 0.7),
        (0.8, 0.9),
        (0.9, 0.8),
        (0.95, 1.0),
    ],
};

fn standard_continentalness(x: f32, z: f32, perlin: &Perlin) -> f32 {
    let x = x * STANDARD_CONTINENTALNESS_INPUT_SCALE;
    let z = z * STANDARD_CONTINENTALNESS_INPUT_SCALE;
    let val = perlin.get([x as f64, z as f64]);
    STANDARD_CONTINENTALNESS_SPLINE.sample(val as f32)
}

fn standard_erosion(x: f32, z: f32, perlin: &Perlin) -> f32 {
    let x = x * STANDARD_EROSION_INPUT_SCALE;
    let z = z * STANDARD_EROSION_INPUT_SCALE;
    let val = perlin.get([x as f64, z as f64]);
    STANDARD_EROSION_SPLINE.sample(val as f32)
}

fn standard_peaks_valleys(x: f32, z: f32, perlin: &Perlin) -> f32 {
    let x = x * STANDARD_PEAKS_VALLEYS_INPUT_SCALE;
    let z = z * STANDARD_PEAKS_VALLEYS_INPUT_SCALE;
    let val = perlin.get([x as f64, z as f64]);
    STANDARD_PEAKS_VALLEYS_SPLINE.sample(val as f32)
}

// pct out of 100
const CAVEY_NESS: f32 = 0.4;

/// Output to range -0.2 to 0.21
fn standard_density(x: f32, y: f32, z: f32, perlin: &Perlin) -> f32 {
    //sample a cube: Has 0 near the center, 1.0 at corners
    let val_x = perlin.get([x as f64, y as f64, z as f64]);
    let val_y = perlin.get([(x + 100000.0) as f64, y as f64, z as f64]);
    let val_z = perlin.get([(x + 200000.0) as f64, y as f64, z as f64]);

    let vec = Vec3::new(val_x as f32, val_y as f32, val_z as f32);
    let density = vec.length() / (3f32).sqrt(); // normalize to 0.0 to 1.0
                                                // remap to -0.4 to 1.0
    let mut density = density * (1.0 + CAVEY_NESS) - CAVEY_NESS;

    // more density as we go down in y
    if y < 30.0 && y > -30.0 {
        //affect it by up to -0.1
        let y_factor = (30.0 - y.abs()) / 30.0;
        density = density - y_factor * 0.1;
    }
    density
}

impl TerrainParams {
    pub fn perlin(&self) -> TerrainPerlin {
        TerrainPerlin {
            perlin_continentalness: Perlin::new(self.seed),
            perlin_erosion: Perlin::new(self.seed.wrapping_add(1)),
            perlin_density: Perlin::new(self.seed.wrapping_add(2)),
            perlin_peaks_valleys: Perlin::new(self.seed.wrapping_add(3)),
            transform_continentalness: standard_continentalness,
            transform_erosion: standard_erosion,
            transform_peaks_valleys: standard_peaks_valleys,
            transform_density: standard_density,
        }
    }
}

impl TerrainPerlin {
    /// Given an xy, return "Offset" and "Squash" factors for terrain height
    fn sample_height_partial(&self, x: f32, z: f32) -> (f32, f32) {
        let continentalness = (self.transform_continentalness)(x, z, &self.perlin_continentalness);
        let erosion = (self.transform_erosion)(x, z, &self.perlin_erosion);
        let peaks_valleys = (self.transform_peaks_valleys)(x, z, &self.perlin_peaks_valleys);

        // Combine factors to get final height
        let mut height = continentalness * erosion;
        //scale height so this doesn't go beyond -1.0 to 1.0 too much
        height *= 0.9;
        height += peaks_valleys * 0.2;
        height *= 10.0;
        height += 5.0;

        (height, 0.0)
    }

    pub fn sample_height(&self, x: f32, z: f32) -> f32 {
        let (height, _squash) = self.sample_height_partial(x, z);
        height
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
