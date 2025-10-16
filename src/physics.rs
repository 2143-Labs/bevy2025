use avian3d::prelude::*;
use bevy::prelude::*;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_ball_on_space);
    }
}

/// Spawn balls every frame while holding spacebar (from active camera's view)
fn spawn_ball_on_space(
    keyboard: Res<ButtonInput<KeyCode>>,
    camera_query: Query<(&Camera, &Transform)>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Only spawn while space is held down
    if !keyboard.pressed(KeyCode::Space) {
        return;
    }

    // Find the active camera and spawn ball every frame
    if let Some((_, transform)) = camera_query.iter().find(|(cam, _)| cam.is_active) {
        // Calculate spawn position 20 units ahead of camera (4x the original 5)
        let forward = transform.forward();
        let spawn_pos = transform.translation + *forward * 20.0;

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
    }
}
