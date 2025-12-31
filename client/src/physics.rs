use avian3d::prelude::*;
use bevy::prelude::*;
use shared::Config;
use shared::event::server::SpawnMan;

use crate::camera::LocalCamera;
use crate::game_state::GameState;
use crate::game_state::OverlayMenuState;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(avian3d::PhysicsPlugins::default())
            .add_systems(
                Update,
                spawn_man_on_use1
                    .run_if(in_state(GameState::Playing))
                    .run_if(in_state(OverlayMenuState::Hidden)),
            )
            .insert_resource(Gravity(Vec3::new(0.0, -9.81, 0.0)));
    }
}

//if let Some((_, transform)) = camera_query.iter().find(|(cam, _)| cam.is_active) {
// Calculate spawn position 20 units ahead of camera (4x the original 5)
//let forward = transform.forward();
//let spawn_pos = transform.translation + *forward * 20.0;

// Random color for the ball
//let color = Color::srgb(fastrand::f32(), fastrand::f32(), fastrand::f32());

//spawn_ball_writer.write(SpawnCircle {
//position: spawn_pos,
//color,
//});
//}

///Spawn man on use 1
fn spawn_man_on_use1(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    config: Res<Config>,
    camera_query: Query<(&Camera, &Transform), With<LocalCamera>>,
    mut spawn_man_writer: MessageWriter<SpawnMan>,
) {
    if !config.just_pressed(&keyboard, &mouse, shared::GameAction::Special1) {
        return;
    }

    // Find the active camera and spawn
    if let Some((_, transform)) = camera_query.iter().find(|(cam, _)| cam.is_active) {
        // Calculate spawn position 5 units ahead of camera
        let forward = transform.forward();
        let spawn_pos = transform.translation + *forward * 5.0;

        spawn_man_writer.write(SpawnMan {
            position: spawn_pos,
        });
    }
}
