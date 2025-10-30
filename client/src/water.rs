use avian3d::prelude::*;
use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

use crate::game_state::{GameState, WorldEntity};

/// Marker for water entity
#[derive(Component)]
pub struct Water;

/// Marker for objects in water (for buoyancy)
#[derive(Component)]
struct InWater {
    submerged_volume: f32,
}

/// Resource to track water level
#[derive(Resource)]
pub struct WaterLevel(pub f32);

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
            );
    }
}

/// Setup water plane at specified level
pub fn spawn_water(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    water_materials: &mut ResMut<Assets<ExtendedMaterial<StandardMaterial, WaterMaterial>>>,
    water_level: f32,
    size: f32,
) {
    // Store water level as resource
    commands.insert_resource(WaterLevel(water_level));

    // Create a large plane for water
    let water_mesh = Rectangle::new(size, size);

    commands.spawn((
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
    ));
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

/// Check which rigid bodies are in water
fn check_water_immersion(
    water_level: Res<WaterLevel>,
    mut commands: Commands,
    balls: Query<(Entity, &Transform, &Collider), (With<RigidBody>, Without<InWater>)>,
    mut in_water: Query<(Entity, &Transform, &Collider, &mut InWater)>,
) {
    let water_y = water_level.0;

    // Check balls not yet in water
    for (entity, transform, collider) in balls.iter() {
        if let Some(sphere) = collider.shape_scaled().as_ball() {
            let sphere_bottom = transform.translation.y - sphere.radius;

            if sphere_bottom < water_y {
                // Calculate submerged volume (simplified)
                let depth = water_y - sphere_bottom;
                let submerged_ratio = (depth / (sphere.radius * 2.0)).clamp(0.0, 1.0);
                let volume = (4.0 / 3.0) * std::f32::consts::PI * sphere.radius.powi(3);

                commands.entity(entity).insert(InWater {
                    submerged_volume: volume * submerged_ratio,
                });
            }
        }
    }

    // Check balls already in water
    for (entity, transform, collider, mut in_water_comp) in in_water.iter_mut() {
        if let Some(sphere) = collider.shape_scaled().as_ball() {
            let sphere_bottom = transform.translation.y - sphere.radius;

            if sphere_bottom >= water_y {
                // No longer in water
                commands.entity(entity).remove::<InWater>();
            } else {
                // Update submerged volume
                let depth = water_y - sphere_bottom;
                let submerged_ratio = (depth / (sphere.radius * 2.0)).clamp(0.0, 1.0);
                let volume = (4.0 / 3.0) * std::f32::consts::PI * sphere.radius.powi(3);
                in_water_comp.submerged_volume = volume * submerged_ratio;
            }
        }
    }
}

/// Apply buoyancy force to objects in water
fn apply_buoyancy(mut bodies: Query<(&mut LinearVelocity, &InWater, &Mass)>, time: Res<Time>) {
    // Water density for buoyancy calculation
    // Higher value = stronger upward force on submerged objects
    let water_density = 1.5; // Tuned for floating behavior
    let gravity = 9.81;

    for (mut velocity, in_water, mass) in bodies.iter_mut() {
        // Buoyancy force = water_density * submerged_volume * gravity
        // This creates an upward force proportional to displaced water
        let buoyancy_force = water_density * in_water.submerged_volume * gravity;

        // Also subtract weight (downward force from gravity)
        let weight = mass.0 * gravity;

        // Net force = buoyancy - weight
        let net_force = buoyancy_force - weight;

        // Convert force to acceleration (F = ma, so a = F/m)
        let net_accel = net_force / mass.0;

        // Apply net acceleration (upward if buoyant, downward if heavy)
        velocity.y += net_accel * time.delta_secs();

        // Apply water drag (resistance to movement)
        let drag: f32 = 0.95; // Stronger drag for realistic settling
        velocity.0 *= drag.powf(time.delta_secs());
    }
}
