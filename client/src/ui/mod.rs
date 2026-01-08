pub mod connecting_menu;
pub mod home_menu;
pub mod multiplayer_menu;
pub mod paused_menu;
pub mod styles;
pub mod text_input;

use crate::{camera::FreeCam, game_state::{MenuState, OverlayMenuState}};
use bevy::prelude::*;

use shared::character_controller::MovementAction;
pub use text_input::FocusedInput;

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app
            // Setup UI camera
            .add_systems(Startup, setup_ui_camera)
            // Resources
            .init_resource::<FocusedInput>()
            // Home Menu
            .add_systems(OnEnter(MenuState::Home), home_menu::spawn_home_menu)
            .add_systems(OnExit(MenuState::Home), home_menu::despawn_home_menu)
            .add_systems(
                Update,
                (home_menu::handle_home_buttons, home_menu::animate_logo)
                    .run_if(in_state(MenuState::Home)),
            )
            // Multiplayer Menu
            .add_systems(
                OnEnter(MenuState::Multiplayer),
                multiplayer_menu::spawn_multiplayer_menu,
            )
            .add_systems(
                OnExit(MenuState::Multiplayer),
                multiplayer_menu::despawn_multiplayer_menu,
            )
            .add_systems(
                Update,
                (
                    multiplayer_menu::handle_multiplayer_buttons,
                    multiplayer_menu::handle_color_buttons,
                    text_input::handle_text_input_focus,
                    text_input::handle_text_input_keyboard,
                    text_input::update_text_input_visual_feedback,
                )
                    .run_if(in_state(MenuState::Multiplayer)),
            )
            // Connecting Menu
            .add_systems(
                OnEnter(MenuState::Connecting),
                connecting_menu::spawn_connecting_menu_and_connect,
            )
            .add_systems(
                OnExit(MenuState::Connecting),
                connecting_menu::despawn_connecting_menu,
            )
            .add_systems(
                Update,
                (
                    connecting_menu::handle_connecting_buttons,
                    connecting_menu::monitor_connection_status,
                )
                    .run_if(in_state(MenuState::Connecting)),
            )
            // Paused Menu
            .add_systems(
                OnEnter(OverlayMenuState::Paused),
                paused_menu::spawn_paused_menu,
            )
            .add_systems(
                OnExit(OverlayMenuState::Paused),
                paused_menu::despawn_paused_menu,
            )
            .add_systems(Update, paused_menu::handle_paused_menu_buttons)
            .add_systems(
                Update,
                keyboard_input_tps,
            )
            // Global button feedback
            .add_systems(Update, styles::button_visual_feedback);
    }
}

/// Setup UI camera for rendering menus
fn setup_ui_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            is_active: true,
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
    ));
}


/// TODO move this
/// Sends [`MovementAction`] events based on keyboard input.
fn keyboard_input_tps(
    mut movement_writer: MessageWriter<MovementAction>,
    freecam: Query<&FreeCam>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    use avian3d::{math::*};
    let left = keyboard_input.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]);
    let right = keyboard_input.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]);
    let forward = keyboard_input.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]);
    let backward = keyboard_input.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]);

    let horizontal = right as i8 - left as i8;
    let vertical = forward as i8 - backward as i8;
    let direction = Vector2::new(horizontal as Scalar, vertical as Scalar).normalize_or_zero();

    if direction != Vector2::ZERO {
        let Ok(fc) = freecam.single() else {return;};
        let direction_new = Vector2::new(
            direction.x * fc.yaw.cos() - direction.y * fc.yaw.sin(),
            direction.x * fc.yaw.sin() + direction.y * fc.yaw.cos(),
        );
        movement_writer.write(MovementAction::Move(direction_new));
    }

    if keyboard_input.just_pressed(KeyCode::Space) {
        movement_writer.write(MovementAction::Jump);
    }
}

