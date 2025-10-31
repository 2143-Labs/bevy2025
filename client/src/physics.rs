use avian3d::prelude::Gravity;
use bevy::prelude::*;
use shared::event::ERFE;
use shared::event::{client::SpawnUnit2, server::SpawnCircle};
use shared::net_components::ents::Ball;

use crate::game_state::{GameState, NetworkGameState};

/// UI component for the ball counter parent
#[derive(Component)]
struct BallCounterUI;

/// UI component for the ball counter text
#[derive(Component)]
struct BallCounterText;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(avian3d::PhysicsPlugins::default())
            .add_systems(OnEnter(GameState::Playing), setup_ball_counter_ui)
            .add_systems(OnEnter(GameState::MainMenu), despawn_ball_counter_ui)
            .add_systems(
                Update,
                (
                    // TODO receive new world data at any time?
                    spawn_ball_network,
                )
                    .run_if(in_state(NetworkGameState::ClientConnected)),
            )
            .add_systems(
                Update,
                (spawn_ball_on_space, update_ball_counter).run_if(in_state(GameState::Playing)),
            )
        .insert_resource(Gravity(Vec3::new(0.0, -9.81, 0.0)));
    }
}

/// Spawn balls every frame while holding spacebar (from active camera's view)
fn spawn_ball_on_space(
    keyboard: Res<ButtonInput<KeyCode>>,
    camera_query: Query<(&Camera, &Transform)>,
    //mut commands: Commands,
    //mut meshes: ResMut<Assets<Mesh>>,
    //mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawn_ball_writer: MessageWriter<SpawnCircle>,
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

        spawn_ball_writer.write(SpawnCircle {
            position: spawn_pos,
            color,
        });
    }
}

fn spawn_ball_network(
    mut unit_spawns: ERFE<SpawnUnit2>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for spawn in unit_spawns.read() {
        // Spawn ball with physics
        spawn
            .event
            .clone()
            .spawn_entity(&mut commands, &mut meshes, &mut materials);
        info!("Spawned from networked SpawnUnit2");
    }
}

/// Setup UI for ball counter
fn setup_ball_counter_ui(mut commands: Commands) {
    // Root UI node
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BallCounterUI,
        ))
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

/// Despawn ball counter UI when leaving Playing state
fn despawn_ball_counter_ui(mut commands: Commands, ui_query: Query<Entity, With<BallCounterUI>>) {
    for entity in ui_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Update ball counter UI
fn update_ball_counter(
    balls: Query<&Ball>,
    mut counter_text: Query<&mut Text, With<BallCounterText>>,
) {
    let ball_count = balls.iter().count();

    if let Ok(mut text) = counter_text.single_mut() {
        text.0 = format!("Balls: {ball_count}");
    }
}
