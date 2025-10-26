use avian3d::prelude::*;
use bevy::prelude::*;

/// Marker component for spawned balls
#[derive(Component)]
pub struct Ball;

/// UI component for the ball counter text
#[derive(Component)]
struct BallCounterText;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ball_counter_ui)
            .add_systems(Update, (spawn_ball_on_space, update_ball_counter));
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
        let color = Color::srgb(fastrand::f32(), fastrand::f32(), fastrand::f32());

        // Spawn ball with physics
        // Ball volume = (4/3) * π * r³ = (4/3) * π * 0.5³ ≈ 0.524 m³
        // With density ~0.5, mass = 0.524 * 0.5 ≈ 0.26 kg (light, floaty balls)
        commands.spawn((
            Ball, // Marker component for counting
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
            Mass(0.3), // Lighter balls that will float (density ~0.57 of water)
        ));
    }
}

/// Setup UI for ball counter
fn setup_ball_counter_ui(mut commands: Commands) {
    // Root UI node
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        })
        .with_children(|parent| {
            // Counter text
            parent.spawn((
                Text::new("Balls: 0"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                BallCounterText,
            ));
        });
}

/// Update ball counter UI
fn update_ball_counter(
    balls: Query<&Ball>,
    mut counter_text: Query<&mut Text, With<BallCounterText>>,
) {
    let ball_count = balls.iter().count();

    if let Ok(mut text) = counter_text.single_mut() {
        text.0 = format!("Balls: {}", ball_count);
    }
}
