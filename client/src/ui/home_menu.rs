use super::styles::*;
use crate::{assets::ImageAssets, game_state::MenuState};
use bevy::{math::Rot2, prelude::*, ui::UiTransform};
use bevy::{picking::hover::Hovered, ui::Pressed, ui_widgets::Button};

/// Marker for the home menu root entity
#[derive(Component)]
pub struct HomeMenu;

/// Marker for the Play button (skip multiplayer setup, use config defaults)
#[derive(Component)]
pub struct PlayButton;

/// Marker for the Multiplayer button (go to multiplayer setup)
#[derive(Component)]
pub struct MultiplayerButton;

/// Marker for the animated logo
#[derive(Component)]
pub struct AnimatedLogo {
    pub time: f32,
}

/// Spawn the home menu UI
pub fn spawn_home_menu(mut commands: Commands, image_assets: Res<ImageAssets>) {
    info!("Spawning home menu");
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
            HomeMenu,
        ))
        .with_children(|parent| {
            // Logo - animated with bobbing/floating motion
            parent.spawn((
                ImageNode {
                    image: image_assets.logo.clone(),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
                Node {
                    width: Val::Px(436.0),  // 109 * 4 to maintain aspect ratio
                    height: Val::Px(160.0), // 40 * 4 to scale up
                    margin: UiRect::bottom(Val::Px(60.0)),
                    ..default()
                },
                UiTransform::default(),
                AnimatedLogo { time: 0.0 },
            ));

            // Subtitle
            parent.spawn({
                let (text, font, color) = label_text("Roll Over The Sphere");
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

            // Play button
            {
                let (node, bg_color, border_color) = menu_button_bundle();
                let (text, font, color) = menu_button_text("Play");
                parent
                    .spawn((
                        node,
                        bg_color,
                        border_color,
                        Button,
                        Hovered::default(),
                        PlayButton,
                    ))
                    .with_children(|button| {
                        button.spawn((text, font, color));
                    });
            }

            // Multiplayer button
            {
                let (node, bg_color, border_color) = menu_button_bundle();
                let (text, font, color) = menu_button_text("Multiplayer");
                parent
                    .spawn((
                        node,
                        bg_color,
                        border_color,
                        Button,
                        Hovered::default(),
                        MultiplayerButton,
                    ))
                    .with_children(|button| {
                        button.spawn((text, font, color));
                    });
            }
        });
}

/// Despawn the home menu
pub fn despawn_home_menu(mut commands: Commands, menu_query: Query<Entity, With<HomeMenu>>) {
    for entity in menu_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Handle button interactions in home menu
pub fn handle_home_buttons(
    play_query: Query<(), (Added<Pressed>, With<PlayButton>)>,
    multiplayer_query: Query<(), (Added<Pressed>, With<MultiplayerButton>, Without<PlayButton>)>,
    //next_game_state: ResMut<NextState<GameState>>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
    mut _config: ResMut<crate::Config>,
) {
    // Play button - skip networking, go directly to single-player
    for () in play_query.iter() {
        info!("Play button pressed - starting single-player");
        #[cfg(feature = "singleplayer")]
        {
            use shared::netlib::NetworkConnectionTarget;
            let port = rand::random_range(20000..60000);
            _config.port = port;
            _config.ip = "127.0.0.1".to_string();
            _config.host_ip = None;
            std::thread::spawn(move || {
                // TODO exit
                server::call_from_client_for_singleplayer(NetworkConnectionTarget {
                    ip: "127.0.0.1".to_string(),
                    port,
                });
            });
            next_menu_state.set(MenuState::Connecting);
        }
        #[cfg(not(feature = "singleplayer"))]
        {
            warn!("Single-player mode is not enabled in this build.");
        }
    }

    // Multiplayer button - go to multiplayer setup
    for () in multiplayer_query.iter() {
        info!("Multiplayer button pressed");
        next_menu_state.set(MenuState::Multiplayer);
    }
}

/// Animate the logo with rotation and subtle floating motion
pub fn animate_logo(time: Res<Time>, mut query: Query<(&mut AnimatedLogo, &mut UiTransform)>) {
    for (mut logo, mut ui_transform) in query.iter_mut() {
        logo.time += time.delta_secs();

        // Rotation: gentle swing left and right (-5 to +5 degrees)
        let rotation_angle = (logo.time * 1.5).sin() * 0.087; // 0.087 rad ≈ 5 degrees

        // Floating motion: subtle diagonal drift from top-right to bottom-left
        let float_x = (logo.time * 0.8).sin() * 8.0; // Horizontal movement
        let float_y = (logo.time * 0.8).cos() * 8.0; // Vertical movement

        ui_transform.rotation = Rot2::radians(rotation_angle);
        ui_transform.translation = Val2::px(float_x, float_y);
    }
}
