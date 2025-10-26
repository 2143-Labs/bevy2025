use avian3d::prelude::*;
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use noise::{NoiseFn, Perlin};

use crate::grass::{GrassMaterial, WindSettings, spawn_grass_on_terrain};
use crate::water::{WaterMaterial, spawn_water};
use bevy::pbr::ExtendedMaterial;

/// Marker for terrain entity
#[derive(Component)]
pub struct Terrain;

/// Marker for boundary walls
#[derive(Component)]
pub struct BoundaryWall;

/// Terrain generation parameters
#[derive(Resource)]
pub struct TerrainParams {
    pub seed: u32,
    pub plane_size: f32,
    pub subdivisions: u32,
    pub max_height_delta: f32,
}

impl Default for TerrainParams {
    fn default() -> Self {
        Self {
            seed: fastrand::u32(..),
            plane_size: 100.0,
            subdivisions: 100,
            max_height_delta: 10.0,
        }
    }
}

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_terrain)
            .add_systems(Update, draw_boundary_debug);
    }
}

/// Setup terrain mesh with physics collider
fn setup_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut water_materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, WaterMaterial>>>,
    mut grass_materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, GrassMaterial>>>,
    wind: Res<WindSettings>,
) {
    let terrain_params = TerrainParams::default();

    // Calculate water level: 30% between min and max terrain height
    // Terrain heights range from -max_height_delta to +max_height_delta
    let min_height = -terrain_params.max_height_delta;
    let max_height = terrain_params.max_height_delta;
    let water_level = min_height + 0.3 * (max_height - min_height);

    // Generate terrain mesh
    let terrain_mesh = generate_terrain_mesh(&terrain_params);
    let terrain_mesh_handle = meshes.add(terrain_mesh.clone());

    // Create brown terrain material
    let terrain_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.4, 0.2), // Brown earth
        metallic: 0.0,
        perceptual_roughness: 0.9,
        ..default()
    });

    // Spawn terrain with physics collider
    commands.spawn((
        Mesh3d(terrain_mesh_handle),
        MeshMaterial3d(terrain_material.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        RigidBody::Static,
        Collider::trimesh_from_mesh(&terrain_mesh).unwrap(),
        Terrain,
    ));

    // Add boundary walls around the terrain
    spawn_boundary_walls(&mut commands, &mut meshes, &mut materials, &terrain_params);

    // Spawn water at calculated level
    spawn_water(
        &mut commands,
        &mut meshes,
        &mut water_materials,
        water_level,
        terrain_params.plane_size,
    );

    // Spawn grass on terrain - very dense grass with height-based variation
    // Using mesh merging + LOD, we can handle extremely high density!
    spawn_grass_on_terrain(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut grass_materials,
        &wind,
        terrain_params.plane_size,
        8.0, // grass density: 8.0 blades per square meter base (LOD reduces in distance)
        terrain_params.seed,
        terrain_params.max_height_delta,
        water_level,
    );

    // Add directional light (sun)
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.3, 0.0)),
    ));
}

/// Spawn invisible boundary walls around the terrain to keep balls from falling off
fn spawn_boundary_walls(
    commands: &mut Commands,
    _meshes: &mut ResMut<Assets<Mesh>>,
    _materials: &mut ResMut<Assets<StandardMaterial>>,
    params: &TerrainParams,
) {
    let size = params.plane_size;
    let wall_height = size; // Height matches terrain size (100 units)
    let wall_thickness = 0.1; // Very thin planes
    let half_size = size * 0.5;

    // Base at lowest possible terrain point
    let base_y = -params.max_height_delta;
    let center_y = base_y + wall_height / 2.0;

    // North wall (positive Z) - plane perpendicular to Z axis
    commands.spawn((
        Transform::from_xyz(0.0, center_y, half_size),
        RigidBody::Static,
        Collider::cuboid(size, wall_height, wall_thickness),
        BoundaryWall,
    ));

    // South wall (negative Z) - plane perpendicular to Z axis
    commands.spawn((
        Transform::from_xyz(0.0, center_y, -half_size),
        RigidBody::Static,
        Collider::cuboid(size, wall_height, wall_thickness),
        BoundaryWall,
    ));

    // East wall (positive X) - plane perpendicular to X axis
    commands.spawn((
        Transform::from_xyz(half_size, center_y, 0.0),
        RigidBody::Static,
        Collider::cuboid(wall_thickness, wall_height, size),
        BoundaryWall,
    ));

    // West wall (negative X) - plane perpendicular to X axis
    commands.spawn((
        Transform::from_xyz(-half_size, center_y, 0.0),
        RigidBody::Static,
        Collider::cuboid(wall_thickness, wall_height, size),
        BoundaryWall,
    ));
}

/// Draw debug wireframe borders for boundary walls
fn draw_boundary_debug(
    mut gizmos: Gizmos,
    walls: Query<(&Transform, &Collider), With<BoundaryWall>>,
) {
    for (transform, collider) in walls.iter() {
        // Get the cuboid dimensions from the collider shape data
        if let Some(cuboid) = collider.shape_scaled().as_cuboid() {
            let half_extents = cuboid.half_extents;
            let pos = transform.translation;

            // Draw a wireframe box
            gizmos.cuboid(
                Transform::from_translation(pos).with_scale(Vec3::from(half_extents) * 2.0),
                Color::srgb(1.0, 0.0, 0.0), // Red debug lines
            );
        }
    }
}

/// Generate procedural terrain mesh using Perlin noise
fn generate_terrain_mesh(params: &TerrainParams) -> Mesh {
    let noise = Perlin::new(params.seed);
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
            let noise_x = x_pos * 0.05; // Scale factor for noise frequency
            let noise_z = z_pos * 0.05;
            let height = noise.get([noise_x as f64, noise_z as f64]) as f32 * height_scale;

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
