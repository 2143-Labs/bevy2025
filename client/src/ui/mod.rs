pub mod connecting_menu;
pub mod home_menu;
pub mod multiplayer_menu;
pub mod paused_menu;
pub mod styles;
pub mod text_input;

use crate::game_state::{MenuState, OverlayMenuState};
use bevy::prelude::*;

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
