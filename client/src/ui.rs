use bevy::{
    input_focus::InputFocus,
    math::Rot2,
    prelude::*,
    ui::UiTransform,
};

use crate::{assets::ImageAssets, game_state::GameState};

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputFocus>()
            .add_systems(Startup, setup_ui_camera)
            .add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
            .add_systems(OnExit(GameState::MainMenu), despawn_main_menu)
            .add_systems(OnEnter(GameState::Paused), spawn_paused_menu)
            .add_systems(OnExit(GameState::Paused), despawn_paused_menu)
            .add_systems(
                Update,
                (
                    button_system,
                    animate_logo,
                    handle_menu_buttons,
                    handle_paused_menu_buttons,
                ),
            );
    }
}

/// Marker component for the main menu
#[derive(Component)]
pub struct MainMenu;

/// Marker components for different menu buttons
#[derive(Component)]
pub struct PlayButton;

#[derive(Component)]
pub struct JoinLobbyButton;

#[derive(Component)]
pub struct SettingsButton;

/// Marker for the animated logo
#[derive(Component)]
struct AnimatedLogo {
    time: f32,
}

/// Marker component for the paused menu
#[derive(Component)]
pub struct PausedMenu;

/// Marker components for paused menu buttons
#[derive(Component)]
pub struct ResumeButton;

#[derive(Component)]
pub struct MainMenuButton;

/// Setup UI camera once at startup - stays active for all UI rendering
fn setup_ui_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            is_active: true,
            order: 1,
            ..default()
        },
    ));
}

/// Spawn the main menu when entering MainMenu state
fn spawn_main_menu(mut commands: Commands, image_assets: Res<ImageAssets>, assets: Res<AssetServer>) {
    // Main menu container
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: px(20),
                ..default()
            },
            MainMenu,
        ))
        .with_children(|parent| {
            // Logo - scaled to ~1/4 of screen height
            // Original size is 109x40, we'll scale it up significantly
            // Using nearest neighbor filtering for crisp pixel art
            parent.spawn((
                ImageNode {
                    image: image_assets.logo.clone(),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
                Node {
                    width: px(436.0),  // 109 * 4 to maintain aspect ratio
                    height: px(160.0), // 40 * 4 to scale up
                    margin: UiRect::bottom(px(60)),
                    ..default()
                },
                UiTransform::default(),
                AnimatedLogo { time: 0.0 },
            ));

            // Play button
            parent.spawn(menu_button(&assets, "Play", PlayButton));

            // Join Lobby button
            parent.spawn(menu_button(&assets, "Join Lobby", JoinLobbyButton));

            // Settings button
            parent.spawn(menu_button(&assets, "Settings", SettingsButton));
        });
}

/// Despawn main menu when exiting MainMenu state
fn despawn_main_menu(mut commands: Commands, menu_query: Query<Entity, With<MainMenu>>) {
    for entity in menu_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Spawn the paused menu when entering Paused state
fn spawn_paused_menu(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: px(30),
                ..default()
            },
            PausedMenu,
        ))
        .with_children(|parent| {
            // "PAUSED" title
            parent.spawn((
                Text::new("PAUSED"),
                TextFont {
                    font: assets.load("fonts/PTSans-Regular.ttf"),
                    font_size: 80.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(px(40)),
                    ..default()
                },
            ));

            // Resume button
            parent.spawn(menu_button(&assets, "Resume", ResumeButton));

            // Main Menu button
            parent.spawn(menu_button(&assets, "Main Menu", MainMenuButton));
        });
}

/// Despawn paused menu when exiting Paused state
fn despawn_paused_menu(mut commands: Commands, menu_query: Query<Entity, With<PausedMenu>>) {
    for entity in menu_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Handle main menu button clicks
fn handle_menu_buttons(
    play_button_query: Query<&Interaction, (Changed<Interaction>, With<PlayButton>)>,
    join_button_query: Query<&Interaction, (Changed<Interaction>, With<JoinLobbyButton>)>,
    settings_button_query: Query<&Interaction, (Changed<Interaction>, With<SettingsButton>)>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    // Play button - transition to Playing state
    for interaction in play_button_query.iter() {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::Playing);
        }
    }

    // Join Lobby button - TODO: implement lobby functionality
    for interaction in join_button_query.iter() {
        if *interaction == Interaction::Pressed {
            info!("Join Lobby clicked - not yet implemented");
        }
    }

    // Settings button - TODO: implement settings menu
    for interaction in settings_button_query.iter() {
        if *interaction == Interaction::Pressed {
            info!("Settings clicked - not yet implemented");
        }
    }
}

/// Handle paused menu button clicks
fn handle_paused_menu_buttons(
    resume_button_query: Query<&Interaction, (Changed<Interaction>, With<ResumeButton>)>,
    main_menu_button_query: Query<&Interaction, (Changed<Interaction>, With<MainMenuButton>)>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    // Resume button - transition back to Playing state
    for interaction in resume_button_query.iter() {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::Playing);
        }
    }

    // Main Menu button - transition to MainMenu state
    for interaction in main_menu_button_query.iter() {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::MainMenu);
        }
    }
}

const NORMAL_BUTTON: Color = Color::srgb(0.2, 0.2, 0.25);
const HOVERED_BUTTON: Color = Color::srgb(0.3, 0.3, 0.4);
const PRESSED_BUTTON: Color = Color::srgb(0.4, 0.5, 0.7);
const NORMAL_BORDER: Color = Color::srgb(0.4, 0.4, 0.5);
const HOVERED_BORDER: Color = Color::srgb(0.6, 0.6, 0.8);
const PRESSED_BORDER: Color = Color::srgb(0.8, 0.85, 1.0);

fn button_system(
    mut input_focus: ResMut<InputFocus>,
    mut interaction_query: Query<
        (
            Entity,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut Button,
        ),
        Changed<Interaction>,
    >,
) {
    for (entity, interaction, mut color, mut border_color, mut button) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                input_focus.set(entity);
                *color = PRESSED_BUTTON.into();
                *border_color = BorderColor::all(PRESSED_BORDER);
                button.set_changed();
            }
            Interaction::Hovered => {
                input_focus.set(entity);
                *color = HOVERED_BUTTON.into();
                *border_color = BorderColor::all(HOVERED_BORDER);
                button.set_changed();
            }
            Interaction::None => {
                input_focus.clear();
                *color = NORMAL_BUTTON.into();
                *border_color = BorderColor::all(NORMAL_BORDER);
            }
        }
    }
}

/// Animate the logo with rotation and subtle floating motion
fn animate_logo(time: Res<Time>, mut query: Query<(&mut AnimatedLogo, &mut UiTransform)>) {
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


/// Creates a styled menu button with a marker component
fn menu_button(asset_server: &AssetServer, label: &str, marker: impl Component) -> impl Bundle {
    (
        Button,
        Node {
            width: px(280),
            height: px(70),
            border: UiRect::all(px(3)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BorderColor::all(NORMAL_BORDER),
        BorderRadius::all(px(8)),
        BackgroundColor(NORMAL_BUTTON),
        marker,
        children![(
            Text::new(label),
            TextFont {
                font: asset_server.load("fonts/PTSans-Regular.ttf"),
                font_size: 40.0,
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.95, 0.95)),
        )],
    )
}
