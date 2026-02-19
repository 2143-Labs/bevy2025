use avian3d::prelude::*;
use bevy::prelude::*;
use shared::physics::{
    terrain::{Terrain, TerrainParams, generate_terrain_trimesh, spawn_boundary_walls},
    water::spawn_water_shared,
};

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TerrainParams::default())
            .add_systems(Startup, setup_terrain_server);
    }
}

/// Setup terrain mesh with physics collider
pub fn setup_terrain_server(mut commands: Commands, terrain_params: Res<TerrainParams>) {
    // Calculate water level: 30% between min and max terrain height
    // Terrain heights range from -max_height_delta to +max_height_delta
    let min_height = -terrain_params.max_height_delta;
    let max_height = terrain_params.max_height_delta;
    let water_level = min_height + 0.3 * (max_height - min_height);

    // Generate terrain collision mesh data
    let (vertices, indices) = generate_terrain_trimesh(&terrain_params);

    // Spawn terrain with physics collider
    commands.spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        RigidBody::Static,
        Collider::trimesh(vertices, indices),
        Terrain,
    ));

    // Add boundary walls around the terrain
    spawn_boundary_walls(&mut commands, &terrain_params);

    // Spawn water at calculated level
    spawn_water_shared(&mut commands, water_level, terrain_params.plane_size);
}
