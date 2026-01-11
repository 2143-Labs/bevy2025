use avian3d::prelude::*;
use bevy::prelude::*;
use shared::Config;
use shared::event::server::SpawnMan;
use shared::net_components::ours::Dead;

use crate::camera::LocalCamera;
use crate::game_state::GameState;
use crate::game_state::InputControlState;
use crate::game_state::OverlayMenuState;
use crate::network::CurrentThirdPersonControlledUnit;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(avian3d::PhysicsPlugins::default())
            .add_systems(
                Update,
                spawn_man_on_use1
                    .run_if(in_state(GameState::Playing))
                    .run_if(in_state(OverlayMenuState::Hidden))
                    .run_if(in_state(InputControlState::Freecam)),
            )
            .add_systems(
                Update,
                move_to_freecam_on_unit_dead.run_if(in_state(GameState::Playing)),
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
    let just_pressed_special1 =
        config.just_pressed(&keyboard, &mouse, shared::GameAction::Special1);
    let just_pressed_special2 =
        config.just_pressed(&keyboard, &mouse, shared::GameAction::Special2);
    if !just_pressed_special1 && !just_pressed_special2 {
        return;
    }

    // Find the active camera and spawn
    let Some((_, transform)) = camera_query.iter().find(|(cam, _)| cam.is_active) else {
        return;
    };

    // Calculate spawn position 5 units ahead of camera
    let forward = transform.forward();
    let spawn_pos = transform.translation + *forward * 5.0;

    let controller_type = match (just_pressed_special1, just_pressed_special2) {
        (_, true) => "TypeE".to_string(),
        _ => "TypeQ".to_string(),
    };

    spawn_man_writer.write(SpawnMan {
        position: spawn_pos,
        controller_type,
    });
}

fn move_to_freecam_on_unit_dead(
    mut commands: Commands,
    dead_cur_unit: Query<(Entity, &CurrentThirdPersonControlledUnit), With<Dead>>,
    mut new_cam_state: ResMut<NextState<InputControlState>>,
) {
    for (cur_ent, _is_third) in dead_cur_unit {
        commands
            .entity(cur_ent)
            .remove::<CurrentThirdPersonControlledUnit>();
        new_cam_state.set(InputControlState::Freecam);
    }
}
