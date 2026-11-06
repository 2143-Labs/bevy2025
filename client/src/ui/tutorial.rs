//! "You win!" when a player in this tutorial reaches the flag.
//!
//! The server sends `TutorialComplete` to everyone in the instance; five seconds later it asks to
//! send everyone back to the main menu (`HostMigration::ToMenu` with the `tutorial_complete`
//! reason, see `network::receive_host_migration`).

use bevy::prelude::*;
use shared::event::{UDPacketEvent, client::TutorialComplete};

use crate::game_state::GameState;
use crate::ui::styles::heading_text;

/// The banner root.
#[derive(Component)]
pub struct WinBanner;

pub struct TutorialUiPlugin;

impl Plugin for TutorialUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show_win_banner.run_if(in_state(GameState::Playing)))
            .add_systems(OnExit(GameState::Playing), despawn_win_banner);
    }
}

fn show_win_banner(
    complete: UDPacketEvent<TutorialComplete>,
    existing: Query<(), With<WinBanner>>,
    mut commands: Commands,
) {
    if complete.is_empty() || !existing.is_empty() {
        return;
    }
    let (text, font, _) = heading_text("You win!", 96.0);
    commands
        .spawn((
            WinBanner,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Percent(30.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(20),
        ))
        .with_children(|parent| {
            parent.spawn((text, font, TextColor(Color::srgb(0.2, 0.9, 0.3))));
        });
}

fn despawn_win_banner(banner: Query<Entity, With<WinBanner>>, mut commands: Commands) {
    for entity in banner.iter() {
        commands.entity(entity).despawn();
    }
}
