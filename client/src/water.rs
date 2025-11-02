use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use shared::physics::water::{Water, apply_buoyancy, check_water_immersion, spawn_water_shared};

use crate::game_state::{GameState, WorldEntity};

#[derive(Component)]
struct InWater {
    submerged_volume: f32,
}

/// Resource to track water level
#[derive(Resource)]
pub struct WaterLevel(pub f32);
use crate::network::DespawnOnWorldData;

pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<
            ExtendedMaterial<StandardMaterial, WaterMaterial>,
        >::default())
            .register_type::<WaterMaterial>()
            .insert_resource(WaterLevel(0.0)) // Initialize with default value
            .add_systems(
                Update,
                (check_water_immersion, apply_buoyancy)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            .add_plugins(shared::physics::water::SharedWaterPlugin);
    }
}

/// Custom water material extension for animated water
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct WaterMaterial {
    /// Water color (using high binding number to avoid conflicts)
    #[uniform(100)]
    pub water_color: Vec4,
}

impl MaterialExtension for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }
}

/// Setup water plane at specified level
pub fn spawn_water_client(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    water_materials: &mut ResMut<Assets<ExtendedMaterial<StandardMaterial, WaterMaterial>>>,
    water_level: f32,
    size: f32,
) {
    spawn_water_shared(commands, water_level, size);

    // Create a large plane for water
    let water_mesh = Rectangle::new(size, size);

    commands.spawn((
        Name::new("Water"),
        Mesh3d(meshes.add(water_mesh)),
        MeshMaterial3d(water_materials.add(ExtendedMaterial {
            base: StandardMaterial {
                base_color: Color::srgba(0.2, 0.5, 0.8, 0.4), // Natural blue-green, more transparent
                alpha_mode: AlphaMode::Blend,
                unlit: true, // Make water glow/unlit so color shows properly
                ..default()
            },
            extension: WaterMaterial {
                water_color: Vec4::new(0.2, 0.5, 0.8, 0.4), // Match base color
            },
        })),
        Transform::from_xyz(0.0, water_level, 0.0)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)), // Rotate to be horizontal
        Water,
        WorldEntity,
        DespawnOnWorldData,
    ));
}
