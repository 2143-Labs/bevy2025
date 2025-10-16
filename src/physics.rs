use avian3d::prelude::*;
use bevy::prelude::*;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_ball_on_space);
    }
}

/// Spawn a ball when spacebar is pressed (from active camera's view)
fn spawn_ball_on_space(
    keyboard: Res<ButtonInput<KeyCode>>,
    camera_query: Query<&Transform, With<Camera>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        // Find the active camera
        for transform in camera_query.iter() {
            // Calculate spawn position 5 units ahead of camera
            let forward = transform.forward();
            let spawn_pos = transform.translation + *forward * 5.0;

            // Random color for the ball
            let color = Color::srgb(
                fastrand::f32(),
                fastrand::f32(),
                fastrand::f32(),
            );

            // Spawn ball with physics
            commands.spawn((
                Mesh3d(meshes.add(Sphere::new(0.5))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: color,
                    metallic: 0.0,
                    perceptual_roughness: 0.5,
                    ..default()
                })),
                Transform::from_translation(spawn_pos),
                RigidBody::Dynamic,
                Collider::sphere(0.5),
                Mass(1.0),
            ));

            // Only spawn from first active camera
            break;
        }
    }
}
