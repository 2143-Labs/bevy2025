use bevy::prelude::*;
use shared::{
    Config, GameAction,
    character_controller::{MovementAction, UnitChangedMovement},
    event::NetEntId,
};

use crate::{
    game_state::{GameState, InputControlState, OverlayMenuState},
    network::CurrentThirdPersonControlledUnit,
};

/// Marker for FreeCam (playing mode, perspective)
#[derive(Component, Clone, PartialEq)]
pub struct FreeCam {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub move_speed: f32,
}

/// Marker for the player camera that we control
#[derive(Component)]
pub struct LocalCamera;

#[derive(Resource)]
/// Insert this global resource to indicate that our controls should not be forwarded to any unit
/// controller
pub struct ChompInputs;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), setup_cameras)
            .add_systems(OnExit(GameState::Playing), despawn_cameras)
            // Input related: dont run in ChompInputs mode
            .add_systems(
                Update,
                (
                    handle_pause_and_inventory_input,
                    freecam_controller.run_if(in_state(InputControlState::Freecam)),
                    (tps_camera_controller, keyboard_input_tps)
                        .run_if(in_state(InputControlState::ThirdPerson)),
                )
                    .run_if(in_state(GameState::Playing))
                    .run_if(not(resource_exists::<ChompInputs>)),
            );
        // Alawys run these even while Chomping
        app.add_systems(
            Update,
            update_freecam_transform_from_settings_tps
                .run_if(in_state(InputControlState::ThirdPerson))
                .run_if(in_state(GameState::Playing)),
        );

        app.insert_resource(LastInputDirection::default());
    }
}

/// Setup all three cameras
fn setup_cameras(mut commands: Commands) {
    // FreeCam - Perspective, active by default
    commands.spawn((
        Camera3d::default(),
        Camera {
            is_active: true,
            order: 0,
            ..default()
        },
        Transform::from_xyz(50.0, 30.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
        Projection::Perspective(PerspectiveProjection::default()),
        LocalCamera,
        FreeCam {
            yaw: -std::f32::consts::FRAC_PI_4,
            pitch: -0.6,
            zoom: 0.0,
            move_speed: 20.0,
        },
    ));
}

/// Despawn all 3D cameras when leaving Playing state
fn despawn_cameras(mut commands: Commands, freecam_query: Query<Entity, With<FreeCam>>) {
    for entity in freecam_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Handle pause toggling (Escape key)
fn handle_pause_and_inventory_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    config: Res<Config>,
    current_state: Res<State<OverlayMenuState>>,
    mut next_state: ResMut<NextState<OverlayMenuState>>,
) {
    if config.just_pressed(&keyboard, &mouse, GameAction::Escape) {
        match current_state.get() {
            OverlayMenuState::Hidden => {
                next_state.set(OverlayMenuState::Paused);
            }
            OverlayMenuState::Paused => {
                next_state.set(OverlayMenuState::Hidden);
            }
            OverlayMenuState::Inventory => {
                next_state.set(OverlayMenuState::Hidden);
            }
            OverlayMenuState::Scoreboard => {
                next_state.set(OverlayMenuState::Hidden);
            }
            OverlayMenuState::Skills => {
                next_state.set(OverlayMenuState::Hidden);
            }
            _ => {}
        }
    }

    if config.just_pressed(&keyboard, &mouse, GameAction::OpenInventory) {
        match current_state.get() {
            OverlayMenuState::Hidden => {
                next_state.set(OverlayMenuState::Inventory);
            }
            OverlayMenuState::Inventory => {
                next_state.set(OverlayMenuState::Hidden);
            }
            _ => {}
        }
    }

    if config.just_pressed(&keyboard, &mouse, GameAction::Scoreboard) {
        match current_state.get() {
            OverlayMenuState::Hidden => {
                next_state.set(OverlayMenuState::Scoreboard);
            }
            OverlayMenuState::Scoreboard => {
                next_state.set(OverlayMenuState::Hidden);
            }
            _ => {}
        }
    }

    if config.just_pressed(&keyboard, &mouse, GameAction::Skills) {
        match current_state.get() {
            OverlayMenuState::Hidden => {
                next_state.set(OverlayMenuState::Skills);
            }
            OverlayMenuState::Skills => {
                next_state.set(OverlayMenuState::Hidden);
            }
            _ => {}
        }
    }
}

/// FreeCam controller - WASD movement relative to camera orientation
fn freecam_controller(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    time: Res<Time>,
    mut camera_query: Query<(&mut Transform, &mut FreeCam)>,
    config: Res<Config>,
) {
    let Ok((mut transform, mut freecam)) = camera_query.single_mut() else {
        return;
    };

    let old_freecam = freecam.clone();

    // Mouse rotation (right-click to pan)
    if config.pressed(&keyboard, &mouse, GameAction::Fire2) {
        let sensitivity = 0.003;
        for motion in mouse_motion.read() {
            freecam.yaw -= motion.delta.x * sensitivity;
            freecam.pitch -= motion.delta.y * sensitivity;
            freecam.pitch = freecam.pitch.clamp(
                -std::f32::consts::FRAC_PI_2 + 0.1,
                std::f32::consts::FRAC_PI_2 - 0.1,
            );
        }
    }

    if old_freecam != *freecam {
        //info!("FreeCam yaw: {}, pitch: {}", freecam.yaw, freecam.pitch);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, freecam.yaw, freecam.pitch, 0.0);
    }

    // WASD movement relative to camera orientation
    let mut movement = Vec3::ZERO;
    let speed = freecam.move_speed;

    // Get forward/right from camera's current rotation
    let forward = transform.forward();
    let right = transform.right();

    // Check if key pressed is in keybindings vector for gameaction
    if config.pressed(&keyboard, &mouse, GameAction::MoveForward) {
        movement += *forward * speed * time.delta_secs();
    }
    if config.pressed(&keyboard, &mouse, GameAction::MoveBackward) {
        movement -= *forward * speed * time.delta_secs();
    }
    if config.pressed(&keyboard, &mouse, GameAction::StrafeLeft) {
        movement -= *right * speed * time.delta_secs();
    }
    if config.pressed(&keyboard, &mouse, GameAction::StrafeRight) {
        movement += *right * speed * time.delta_secs();
    }
    if config.pressed(&keyboard, &mouse, GameAction::Ascend) {
        movement.y += speed * time.delta_secs();
    }
    if config.pressed(&keyboard, &mouse, GameAction::Descend) {
        movement.y -= speed * time.delta_secs();
    }

    if movement != Vec3::ZERO {
        transform.translation += movement;
    }
}

fn tps_camera_controller(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    time: Res<Time>,
    mut camera_query: Query<&mut FreeCam, Without<CurrentThirdPersonControlledUnit>>,
    config: Res<Config>,
) {
    let Ok(mut freecam_settings) = camera_query.single_mut() else {
        warn!("No TPS camera found");
        return;
    };

    // Mouse rotation (right-click to pan)
    if config.pressed(&keyboard, &mouse, GameAction::Fire2) {
        let sensitivity = 0.003;
        for motion in mouse_motion.read() {
            freecam_settings.yaw -= motion.delta.x * sensitivity;
            freecam_settings.pitch -= motion.delta.y * sensitivity;
            freecam_settings.pitch = freecam_settings.pitch.clamp(
                -std::f32::consts::FRAC_PI_2 + 0.1,
                std::f32::consts::FRAC_PI_2 - 0.1,
            );
        }
    }

    if config.pressed(&keyboard, &mouse, GameAction::ZoomCameraIn) {
        freecam_settings.zoom -= 40.0 * time.delta_secs();
        freecam_settings.zoom = freecam_settings.zoom.clamp(-10.0, 100.0);
    }

    if config.pressed(&keyboard, &mouse, GameAction::ZoomCameraOut) {
        freecam_settings.zoom += 40.0 * time.delta_secs();
        freecam_settings.zoom = freecam_settings.zoom.clamp(-10.0, 100.0);
    }
}

fn update_freecam_transform_from_settings_tps(
    mut camera_query: Query<(&mut Transform, &FreeCam), Without<CurrentThirdPersonControlledUnit>>,
    my_controlled_unit: Query<&Transform, With<CurrentThirdPersonControlledUnit>>,
) {
    let Ok((mut freecam_transform, freecam_settings)) = camera_query.single_mut() else {
        warn!("No TPS camera found");
        return;
    };

    let Ok(controlled_transform) = my_controlled_unit.single() else {
        warn!("No controlled unit found for TPS camera");
        return;
    };

    // place camera at an offset behind and above the controlled unit, based on yaw + zoom + pitch
    let offset_distance_base = 25.0;
    // height above the unit to look by default
    let offset_height = 3.0;

    let look_at_position = controlled_transform.translation + Vec3::new(0.0, offset_height, 0.0);
    let offset_distance = offset_distance_base + freecam_settings.zoom;
    let offset_x = offset_distance * freecam_settings.yaw.sin() * freecam_settings.pitch.cos();
    let offset_y = offset_distance * -freecam_settings.pitch.sin();
    let offset_z = offset_distance * freecam_settings.yaw.cos() * freecam_settings.pitch.cos();
    let camera_position =
        look_at_position + Vec3::new(-offset_x, offset_y + offset_height, -offset_z);
    freecam_transform.translation = camera_position;
    freecam_transform.look_at(look_at_position, Vec3::Y);
}

use avian3d::math::*;

#[derive(Resource, Default)]
struct LastInputDirection(Vector2);

/// TODO move this
/// Sends [`MovementAction`] events based on keyboard input.
fn keyboard_input_tps(
    mut movement_writer: MessageWriter<UnitChangedMovement>,
    my_controlled_unit: Query<&NetEntId, With<CurrentThirdPersonControlledUnit>>,
    mut last_input_direction: ResMut<LastInputDirection>,
    freecam: Query<&FreeCam>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    config: Res<Config>,
) {
    let left = config.pressed(&keyboard, &mouse, GameAction::StrafeLeft);
    let right = config.pressed(&keyboard, &mouse, GameAction::StrafeRight);
    let forward = config.pressed(&keyboard, &mouse, GameAction::MoveForward);
    let backward = config.pressed(&keyboard, &mouse, GameAction::MoveBackward);

    let vertical = (forward as i8 - backward as i8) as f32;
    let horizontal = (right as i8 - left as i8) as f32;
    let direction = Vector2::new(horizontal, vertical).normalize_or_zero();
    //info!("TPS input direction: {:?}", direction);
    let unit_net_id = match my_controlled_unit.single() {
        Ok(data) => data,
        Err(_) => {
            // No controlled unit
            warn!("No controlled unit found for TPS keyboard input");
            return;
        }
    };

    if last_input_direction.0 != direction {
        last_input_direction.0 = direction;
    }

    let Ok(fc) = freecam.single() else {
        return;
    };

    let jumping = config.pressed(&keyboard, &mouse, GameAction::Jump);

    let movement_action = MovementAction {
        move_input_dir: direction,
        camera_yaw: fc.yaw,
        move_speed_modifier: 1.0,
        is_jumping: jumping,
    };

    movement_writer.write(UnitChangedMovement {
        net_ent_id: *unit_net_id,
        movement_action,
    });
}
