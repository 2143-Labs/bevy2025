use avian3d::prelude::*;
use bevy::{pbr::ExtendedMaterial, prelude::*};
use shared::physics::terrain::{generate_terrain_mesh, spawn_boundary_walls, BoundaryWall, Terrain, TerrainParams};

use crate::{
    grass::{GrassMaterial, WindSettings, spawn_grass_on_terrain},
    network::DespawnOnWorldData,
    water::{WaterMaterial, spawn_water_client},
};

#[derive(Message)]
pub struct SetupTerrain;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SetupTerrain>()
            .add_systems(Startup, |mut setup_events: MessageWriter<SetupTerrain>| {
                // this can also be sent by world net connect
                setup_events.write(SetupTerrain);
            })
            .add_systems(Update, draw_boundary_debug)
            .add_systems(
                Update,
                setup_terrain_client.run_if(on_message::<SetupTerrain>),
            )
            .insert_resource(TerrainParams::default());
        //.add_plugins(SharedTerrainPlugin);
    }
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

/// Setup terrain mesh with physics collider
fn setup_terrain_client(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut water_materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, WaterMaterial>>>,
    mut grass_materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, GrassMaterial>>>,
    wind: Res<WindSettings>,
    terrain_params: Res<TerrainParams>,
) {
    info!("Setting up terrain with params: {:?}", *terrain_params);

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
        DespawnOnWorldData,
    ));

    // Spawn water at calculated level
    spawn_water_client(
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

    let ents = spawn_boundary_walls(&mut commands, &terrain_params);
    for e in ents {
        commands.entity(e).insert(DespawnOnWorldData);
    }

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
