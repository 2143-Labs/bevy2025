//! "Reconnecting…" while a lost connection is being recovered.
//!
//! The client does not go back to the menu on a timeout: the character is still in the world and a
//! reconnect puts the player back in it. This is the only thing on screen meanwhile.

use bevy::prelude::*;

use crate::game_state::NetworkGameState;

/// The overlay root.
#[derive(Component)]
pub struct ReconnectingOverlay;

pub fn spawn_reconnecting_overlay(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(24.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            ReconnectingOverlay,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Reconnecting…"),
                TextFont {
                    font_size: FontSize::Px(28.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.85, 0.3)),
            ));
        });
}

pub fn despawn_reconnecting_overlay(
    mut commands: Commands,
    overlay: Query<Entity, With<ReconnectingOverlay>>,
) {
    for entity in overlay.iter() {
        commands.entity(entity).despawn();
    }
}

pub struct ReconnectingOverlayPlugin;

impl Plugin for ReconnectingOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(NetworkGameState::Reconnecting),
            spawn_reconnecting_overlay,
        )
        .add_systems(
            OnExit(NetworkGameState::Reconnecting),
            despawn_reconnecting_overlay,
        );
    }
}
