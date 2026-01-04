use avian3d::prelude::*;
use bevy::{pbr::ExtendedMaterial, prelude::*};
use shared::physics::terrain::{
    BoundaryWall, Terrain, TerrainParams, generate_terrain_mesh, spawn_boundary_walls,
};

use crate::{
    grass::{GrassMaterial, WindSettings, create_grass_bundles},
    network::WorldEntity,
    water::{WaterMaterial, spawn_water_client},
};

use crate::game_state::{GameState, TerrainEntity};

#[derive(Message)]
pub struct SetupTerrain;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnExit(GameState::Playing), despawn_terrain)
            // Temporarily disabled to test if this is causing the red rectangle
            // .add_systems(Update, draw_boundary_debug)
            .add_message::<SetupTerrain>()
            // .add_systems(Update, draw_boundary_debug)
            .add_systems(
                Update,
                setup_terrain_client.run_if(on_message::<SetupTerrain>),
            )
            .insert_resource(TerrainParams::default());
        //.add_plugins(SharedTerrainPlugin);
    }
}

/// Draw debug wireframe borders for boundary walls
#[allow(dead_code)]
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

    // Create grass bundles before spawning terrain
    // Very dense grass with height-based variation
    // Using mesh merging + LOD, we can handle extremely high density!
    let _grass_bundles = create_grass_bundles(
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

    // Spawn terrain with grass as children using new Bevy children API
    commands.spawn((
        Name::new("Terrain"),
        Mesh3d(terrain_mesh_handle),
        MeshMaterial3d(terrain_material.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        RigidBody::Static,
        Collider::trimesh_from_mesh(&terrain_mesh).unwrap(),
        Terrain,
        TerrainEntity,
        WorldEntity,
        //Children::spawn(SpawnWith(move |parent: &mut ChildSpawner| {
        //for bundle in _grass_bundles {
        ////parent.spawn(bundle);
        //}
        //})),
    ));

    // Spawn water at calculated level
    spawn_water_client(
        &mut commands,
        &mut meshes,
        &mut water_materials,
        water_level,
        terrain_params.plane_size,
    );

    let ents = spawn_boundary_walls(&mut commands, &terrain_params);
    for e in ents {
        commands.entity(e).insert((TerrainEntity, WorldEntity));
    }

    // Add directional light (sun)
    commands.spawn((
        Name::new("Sun (Directional Light)"),
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.3, 0.0)),
        TerrainEntity,
    ));
}

/// Despawn all terrain and world entities when leaving Playing state
fn despawn_terrain(mut commands: Commands, world_entity_query: Query<Entity, With<TerrainEntity>>) {
    // Despawn all world entities (terrain, walls, light, grass, water, balls, etc.)
    for entity in world_entity_query.iter() {
        commands.entity(entity).despawn();
    }
}
