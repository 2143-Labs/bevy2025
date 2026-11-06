//! "Reconnecting…" while a lost connection is being recovered.
//!
//! The client does not go back to the menu on a timeout: the character is still in the world and a
//! reconnect puts the player back in it. This is the only thing on screen meanwhile. It stays up for
//! the whole attempt, including while the connect sequence has already moved the network state on
//! to `ClientConnecting`/`ClientSendRequestPacket`, so it is keyed on the reconnect itself rather
//! than on `NetworkGameState::Reconnecting`.

use bevy::prelude::*;

use crate::network::{Reconnect, ReconnectLabel};

/// The overlay root.
#[derive(Component)]
pub struct ReconnectingOverlay;

fn spawn_reconnecting_overlay(mut commands: Commands, text: String) {
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
                Text::new(text),
                TextFont {
                    font_size: FontSize::Px(28.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.85, 0.3)),
            ));
        });
}

/// Keep the overlay on screen exactly as long as a reconnect is running.
fn manage_reconnecting_overlay(
    reconnect: Option<Res<Reconnect>>,
    label: Option<Res<ReconnectLabel>>,
    overlay: Query<Entity, With<ReconnectingOverlay>>,
    mut commands: Commands,
) {
    match (reconnect.is_some(), overlay.is_empty()) {
        (true, true) => {
            let text = label
                .map(|label| label.0.clone())
                .unwrap_or_else(|| "Reconnecting…".to_string());
            spawn_reconnecting_overlay(commands, text);
        }
        (false, false) => {
            for entity in overlay.iter() {
                commands.entity(entity).despawn();
            }
        }
        _ => {}
    }
}

pub struct ReconnectingOverlayPlugin;

impl Plugin for ReconnectingOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, manage_reconnecting_overlay);
    }
}
