use super::styles::*;
use crate::{
    auth::AuthState,
    game_state::{GameState, MenuState, NetworkGameState},
};
use bevy::prelude::*;
use bevy::{picking::hover::Hovered, ui::Pressed, ui_widgets::Button};
use shared::{Config, netlib::NetworkConnectionTarget};

/// Marker for the connecting menu root entity
#[derive(Component)]
pub struct ConnectingMenu;

/// Marker for the status text entity
#[derive(Component)]
pub struct ConnectionStatusText;

/// Marker for the Cancel button
#[derive(Component)]
pub struct CancelButton;

/// Spawn the connecting menu UI. The connection itself starts in
/// [`start_connect_when_ticket_ready`] once the auth module has a game ticket.
pub fn spawn_connecting_menu_and_connect(
    mut commands: Commands,
    config: Res<Config>,
    auth_state: Res<State<AuthState>>,
) {
    #[cfg(feature = "web")]
    let server_display = crate::web::game_server_url();
    #[cfg(not(feature = "web"))]
    let server_display = format!("{}:{}", config.ip, config.port);
    let username_display = config.name.clone().unwrap_or_else(|| "Player".to_string());
    let status = if *auth_state.get() == AuthState::Ready {
        "Attempting to connect..."
    } else {
        "Getting ticket..."
    };

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
                    let (text, font, color) = label_text(status);
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
                        .spawn((
                            node,
                            bg_color,
                            border_color,
                            Button,
                            Hovered::default(),
                            CancelButton,
                        ))
                        .with_children(|button| {
                            button.spawn((text, font, color));
                        });
                });
        });
}

/// Start connecting once a ticket is ready. Resets [`AuthState`] to `Idle` so the ticket is
/// used for exactly one connection attempt; the next attempt fetches a new one.
pub fn start_connect_when_ticket_ready(
    mut commands: Commands,
    config: Res<Config>,
    mut next_network_state: ResMut<NextState<NetworkGameState>>,
    mut next_auth_state: ResMut<NextState<AuthState>>,
) {
    commands.insert_resource(NetworkConnectionTarget {
        ip: config.ip.clone(),
        port: config.port,
    });
    next_network_state.set(NetworkGameState::ClientConnecting);
    next_auth_state.set(AuthState::Idle);
}

/// No ticket, no connection: back to the home menu (the auth module already notified).
pub fn return_home_on_auth_failure(mut next_menu_state: ResMut<NextState<MenuState>>) {
    next_menu_state.set(MenuState::Home);
}

/// Despawn the connecting menu
pub fn despawn_connecting_menu(
    mut commands: Commands,
    menu_query: Query<Entity, With<ConnectingMenu>>,
) {
    for entity in menu_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Handle cancel button in connecting menu
pub fn handle_connecting_buttons(
    cancel_query: Query<(), (Added<Pressed>, With<CancelButton>)>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
    mut next_network_state: ResMut<NextState<NetworkGameState>>,
) {
    for () in cancel_query.iter() {
        info!("Cancel button pressed - returning to home menu");
        // Disconnect if connecting
        next_network_state.set(NetworkGameState::Disconnected);
        // Return to home menu
        next_menu_state.set(MenuState::Home);
    }
}

/// Monitor connection status and transition to Playing when connected
pub fn monitor_connection_status(
    network_state: Res<State<NetworkGameState>>,
    auth_state: Res<State<AuthState>>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut status_text_query: Query<&mut Text, With<ConnectionStatusText>>,
) {
    if !network_state.is_changed() && !auth_state.is_changed() {
        return;
    }

    // Update status text based on network state
    if let Ok(mut text) = status_text_query.single_mut() {
        match network_state.get() {
            // Not connecting yet (left over from a previous session): waiting on the ticket.
            NetworkGameState::Disconnected | NetworkGameState::Quit => {
                text.0 = match auth_state.get() {
                    AuthState::Failed => "Login failed",
                    _ => "Getting ticket...",
                }
                .to_string();
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
        }
    }
}
