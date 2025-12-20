use bevy::prelude::*;
use shared::{Config, netlib::NetworkConnectionTarget};
use crate::game_state::{GameState, MenuState, NetworkGameState};
use super::styles::*;

/// Marker for the connecting menu root entity
#[derive(Component)]
pub struct ConnectingMenu;

/// Marker for the status text entity
#[derive(Component)]
pub struct ConnectionStatusText;

/// Marker for the Cancel button
#[derive(Component)]
pub struct CancelButton;

/// Spawn the connecting menu UI
pub fn spawn_connecting_menu_and_connect(mut commands: Commands, config: Res<Config>, mut next_network_state: ResMut<NextState<NetworkGameState>>) {
    let server_display = format!("{}:{}", config.ip, config.port);
    let username_display = config.name.clone().unwrap_or_else(|| "Player".to_string());

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
            ConnectingMenu,
        ))
        .with_children(|parent| {
            // Title
            parent.spawn({
                let (text, font, color) = heading_text("Connecting...", 48.0);
                (text, font, color)
            });

            // Server info
            parent.spawn({
                let (text, font, color) = label_text(format!("Server: {}", server_display));
                (
                    Node {
                        margin: UiRect::bottom(Val::Px(10.0)),
                        ..default()
                    },
                    text,
                    font,
                    color,
                )
            });

            // Username info
            parent.spawn({
                let (text, font, color) = label_text(format!("Username: {}", username_display));
                (
                    Node {
                        margin: UiRect::bottom(Val::Px(40.0)),
                        ..default()
                    },
                    text,
                    font,
                    color,
                )
            });

            // Status text
            parent.spawn((
                {
                    let (text, font, color) = label_text("Attempting to connect...");
                    (text, font, color)
                },
                ConnectionStatusText,
            ));

            // Cancel button
            parent
                .spawn(Node {
                    margin: UiRect::top(Val::Px(40.0)),
                    ..default()
                })
                .with_children(|button_parent| {
                    let (node, bg_color, border_color) = menu_button_bundle();
                    let (text, font, color) = menu_button_text("Cancel");
                    button_parent
                        .spawn((node, bg_color, border_color, Interaction::default(), CancelButton))
                        .with_children(|button| {
                            button.spawn((text, font, color));
                        });
                });
        });

    commands.insert_resource(NetworkConnectionTarget {
        ip: config.ip.clone(),
        port: config.port,
    });

    // Start connection process
    next_network_state.set(NetworkGameState::ClientConnecting);
}

/// Despawn the connecting menu
pub fn despawn_connecting_menu(
    mut commands: Commands,
    menu_query: Query<Entity, With<ConnectingMenu>>,
) {
    for entity in menu_query.iter() {
        commands.entity(entity).despawn();
    }

    // Remove connection trigger
    commands.remove_resource::<ConnectionTrigger>();
}

/// Handle cancel button in connecting menu
pub fn handle_connecting_buttons(
    cancel_query: Query<&Interaction, (Changed<Interaction>, With<CancelButton>)>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
    mut next_network_state: ResMut<NextState<NetworkGameState>>,
) {
    for interaction in cancel_query.iter() {
        if *interaction == Interaction::Pressed {
            info!("Cancel button pressed - returning to home menu");
            // Disconnect if connecting
            next_network_state.set(NetworkGameState::Disconnected);
            // Return to home menu
            next_menu_state.set(MenuState::Home);
        }
    }
}

/// Monitor connection status and transition to Playing when connected
pub fn monitor_connection_status(
    network_state: Res<State<NetworkGameState>>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut status_text_query: Query<&mut Text, With<ConnectionStatusText>>,
    mut last_state: Local<Option<NetworkGameState>>,
) {
    let current_state = network_state.get();

    // Only update if state changed
    if Some(current_state) == last_state.as_ref() {
        return;
    }
    *last_state = Some(current_state.clone());

    // Update status text based on network state
    if let Ok(mut text) = status_text_query.single_mut() {
        match current_state {
            NetworkGameState::Disconnected => {
                text.0 = "Disconnected".to_string();
            }
            NetworkGameState::ClientConnecting => {
                text.0 = "Connecting to server...".to_string();
            }
            NetworkGameState::ClientSendRequestPacket => {
                text.0 = "Sending connection request...".to_string();
            }
            NetworkGameState::ClientConnected => {
                text.0 = "Connected! Starting game...".to_string();
                // Transition to Playing state
                info!("Successfully connected - transitioning to Playing");
                next_game_state.set(GameState::Playing);
            }
            NetworkGameState::Paused => {
                text.0 = "Connection paused".to_string();
            }
            NetworkGameState::Quit => {
                text.0 = "Connection failed".to_string();
            }
        }
    }
}
