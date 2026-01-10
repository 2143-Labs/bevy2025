use super::{styles::*, text_input::*};
use crate::game_state::MenuState;
use bevy::prelude::*;
use shared::Config;

/// Marker for the multiplayer menu root entity
#[derive(Component)]
pub struct MultiplayerMenu;

/// Marker for the server address input field
#[derive(Component)]
pub struct ServerAddressInput;

/// Marker for the username input field
#[derive(Component)]
pub struct UsernameInput;

/// Marker for the Connect button
#[derive(Component)]
pub struct ConnectButton;

/// Marker for the Back button
#[derive(Component)]
pub struct BackButton;

/// Marker for color selection buttons
#[derive(Component)]
pub struct ColorButton {
    pub hue: f32,
}

/// Spawn the multiplayer menu UI with input fields pre-filled from config
pub fn spawn_multiplayer_menu(mut commands: Commands, config: Res<Config>) {
    // Pre-fill inputs from config
    let server_address = format!("{}:{}", config.ip, config.port);
    let username = config.name.clone().unwrap_or_else(|| "Player".to_string());
    let selected_hue = config.player_color_hue;

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
            MultiplayerMenu,
        ))
        .with_children(|parent| {
            // Title
            parent.spawn({
                let (text, font, color) = heading_text("Multiplayer Setup", 48.0);
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

            // Server Address section
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Start,
                    row_gap: Val::Px(10.0),
                    margin: UiRect::bottom(Val::Px(20.0)),
                    ..default()
                })
                .with_children(|section| {
                    // Label
                    section.spawn({
                        let (text, font, color) = label_text("Server Address:");
                        (text, font, color)
                    });

                    // Input field
                    let input = TextInput::new("127.0.0.1:25565", 100).with_value(&server_address);
                    section
                        .spawn((
                            Node {
                                width: Val::Px(400.0),
                                height: Val::Px(50.0),
                                border: UiRect::all(Val::Px(2.0)),
                                padding: UiRect::all(Val::Px(10.0)),
                                justify_content: JustifyContent::Start,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BorderColor::all(Color::srgb(0.4, 0.4, 0.4)),
                            BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                            Interaction::default(),
                            input,
                            ServerAddressInput,
                        ))
                        .with_children(|input_parent| {
                            input_parent.spawn((
                                Text::new(&server_address),
                                TextFont {
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                TextInputDisplay,
                            ));
                        });
                });

            // Username section
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Start,
                    row_gap: Val::Px(10.0),
                    margin: UiRect::bottom(Val::Px(20.0)),
                    ..default()
                })
                .with_children(|section| {
                    // Label
                    section.spawn({
                        let (text, font, color) = label_text("Username:");
                        (text, font, color)
                    });

                    // Input field
                    let input = TextInput::new("Player", 32).with_value(&username);
                    section
                        .spawn((
                            Node {
                                width: Val::Px(400.0),
                                height: Val::Px(50.0),
                                border: UiRect::all(Val::Px(2.0)),
                                padding: UiRect::all(Val::Px(10.0)),
                                justify_content: JustifyContent::Start,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BorderColor::all(Color::srgb(0.4, 0.4, 0.4)),
                            BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                            Interaction::default(),
                            input,
                            UsernameInput,
                        ))
                        .with_children(|input_parent| {
                            input_parent.spawn((
                                Text::new(&username),
                                TextFont {
                                    font_size: 20.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                TextInputDisplay,
                            ));
                        });
                });

            // Player Color section
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Start,
                    row_gap: Val::Px(10.0),
                    margin: UiRect::bottom(Val::Px(40.0)),
                    ..default()
                })
                .with_children(|section| {
                    // Label
                    section.spawn({
                        let (text, font, color) = label_text("Player Color:");
                        (text, font, color)
                    });

                    // Color selection row
                    section
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(15.0),
                            ..default()
                        })
                        .with_children(|colors| {
                            // Create 8 color buttons with different hues
                            for i in 0..8 {
                                let hue = (i as f32) * 45.0; // 0, 45, 90, 135, 180, 225, 270, 315
                                let is_selected = (hue - selected_hue).abs() < 1.0;

                                // Create HSL color for UI display
                                let color = Color::hsl(hue, 0.9, 0.5);

                                colors.spawn((
                                    Node {
                                        width: Val::Px(45.0),
                                        height: Val::Px(45.0),
                                        border: UiRect::all(Val::Px(if is_selected {
                                            4.0
                                        } else {
                                            2.0
                                        })),
                                        ..default()
                                    },
                                    BackgroundColor(color.into()),
                                    BorderColor::all(if is_selected {
                                        Color::WHITE
                                    } else {
                                        Color::srgb(0.4, 0.4, 0.4)
                                    }),
                                    Interaction::default(),
                                    ColorButton { hue },
                                ));
                            }
                        });
                });

            // Buttons row
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(20.0),
                    ..default()
                })
                .with_children(|buttons| {
                    // Back button
                    {
                        let (node, bg_color, border_color) = menu_button_bundle();
                        let (text, font, color) = menu_button_text("Back");
                        buttons
                            .spawn((
                                node,
                                bg_color,
                                border_color,
                                Interaction::default(),
                                BackButton,
                            ))
                            .with_children(|button| {
                                button.spawn((text, font, color));
                            });
                    }

                    // Connect button
                    {
                        let (node, bg_color, border_color) = menu_button_bundle();
                        let (text, font, color) = menu_button_text("Connect");
                        buttons
                            .spawn((
                                node,
                                bg_color,
                                border_color,
                                Interaction::default(),
                                ConnectButton,
                            ))
                            .with_children(|button| {
                                button.spawn((text, font, color));
                            });
                    }
                });
        });
}

/// Despawn the multiplayer menu
pub fn despawn_multiplayer_menu(
    mut commands: Commands,
    menu_query: Query<Entity, With<MultiplayerMenu>>,
) {
    for entity in menu_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Handle color button interactions
pub fn handle_color_buttons(
    color_query: Query<(&Interaction, &ColorButton), Changed<Interaction>>,
    mut all_color_buttons: Query<(&mut BorderColor, &mut Node, &ColorButton)>,
    mut config: ResMut<Config>,
) {
    for (interaction, color_button) in color_query.iter() {
        if *interaction == Interaction::Pressed {
            config.player_color_hue = color_button.hue;

            // Update border colors and widths for all buttons
            for (mut border, mut node, button) in all_color_buttons.iter_mut() {
                if (button.hue - color_button.hue).abs() < 1.0 {
                    *border = BorderColor::all(Color::WHITE);
                    node.border = UiRect::all(Val::Px(4.0));
                } else {
                    *border = BorderColor::all(Color::srgb(0.4, 0.4, 0.4));
                    node.border = UiRect::all(Val::Px(2.0));
                }
            }
        }
    }
}

/// Update color button hover effects
pub fn update_color_button_hover(
    mut color_query: Query<
        (&Interaction, &ColorButton, &mut BorderColor),
        (Changed<Interaction>, With<ColorButton>),
    >,
    config: Res<Config>,
) {
    for (interaction, color_button, mut border_color) in color_query.iter_mut() {
        let is_selected = (color_button.hue - config.player_color_hue).abs() < 1.0;

        match interaction {
            Interaction::Hovered => {
                if !is_selected {
                    *border_color = BorderColor::all(Color::srgb(0.7, 0.7, 0.7));
                }
            }
            Interaction::None => {
                if !is_selected {
                    *border_color = BorderColor::all(Color::srgb(0.4, 0.4, 0.4));
                }
            }
            _ => {}
        }
    }
}

/// Handle button interactions in multiplayer menu
pub fn handle_multiplayer_buttons(
    connect_query: Query<&Interaction, (Changed<Interaction>, With<ConnectButton>)>,
    back_query: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<BackButton>,
            Without<ConnectButton>,
        ),
    >,
    server_input_query: Query<&TextInput, With<ServerAddressInput>>,
    username_input_query: Query<&TextInput, (With<UsernameInput>, Without<ServerAddressInput>)>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
    mut config: ResMut<Config>,
) {
    // Back button
    for interaction in back_query.iter() {
        if *interaction == Interaction::Pressed {
            info!("Back button pressed");
            next_menu_state.set(MenuState::Home);
        }
    }

    // Connect button
    for interaction in connect_query.iter() {
        if *interaction == Interaction::Pressed {
            // Get input values
            let Ok(server_input) = server_input_query.single() else {
                continue;
            };
            let Ok(username_input) = username_input_query.single() else {
                continue;
            };

            let server_address = server_input.value.clone();
            let username = username_input.value.clone();

            // Parse server address (format: "ip:port" or "domain:port")
            let (ip, port) = if let Some((ip_part, port_part)) = server_address.split_once(':') {
                let port = port_part.parse().unwrap_or(config.port);
                (ip_part.to_string(), port)
            } else {
                // No colon, treat entire input as IP with default port
                (server_address.clone(), config.port)
            };

            info!(
                "Connect button pressed - IP: {}, Port: {}, Username: {}, Color: {}",
                ip, port, username, config.player_color_hue
            );

            // Update config (temporary, not persisted)
            config.ip = ip;
            config.port = port;
            config.name = Some(username);

            // Transition to connecting state
            next_menu_state.set(MenuState::Connecting);
        }
    }
}
