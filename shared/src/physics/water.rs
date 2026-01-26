use avian3d::prelude::*;
use bevy_internal::prelude::*;

use crate::net_components::ents::Ball;

/// Marker for water entity
#[derive(Component)]
pub struct Water;

/// Resource to track water level
#[derive(Resource)]
pub struct WaterLevel(pub f32);

/// Marker for objects in water (for buoyancy)
#[derive(Component)]
pub struct InWater {
    submerged_volume: f32,
}

pub struct SharedWaterPlugin;

impl Plugin for SharedWaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (check_water_immersion, apply_buoyancy)
                .chain()
                .run_if(resource_exists::<WaterLevel>),
        );
    }
}

/// Setup water plane at specified level
pub fn spawn_water_shared(commands: &mut Commands, water_level: f32, _size: f32) {
    // Store water level as resource
    commands.insert_resource(WaterLevel(water_level));
}

/// Check which rigid bodies are in water
pub fn check_water_immersion(
    water_level: Res<WaterLevel>,
    mut commands: Commands,
    balls: Query<(Entity, &Transform, &Collider), (With<RigidBody>, With<Ball>, Without<InWater>)>,
    mut in_water: Query<(Entity, &Transform, &Collider, &mut InWater), With<Ball>>,
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
pub fn apply_buoyancy(mut bodies: Query<(&mut LinearVelocity, &InWater, &Mass)>, time: Res<Time>) {
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
