use super::{styles::*, text_input::*};
use crate::{
    assets::ImageAssets,
    auth::{AuthRequest, AuthSession, AvailableLoginMethods, LoginMethod},
    game_state::MenuState,
};
use bevy::{math::Rot2, prelude::*, ui::UiTransform};
use bevy::{picking::hover::Hovered, ui::Pressed, ui_widgets::Button};

/// Marker for the home menu root entity
#[derive(Component)]
pub struct HomeMenu;

/// Marker for the Play now button (guest or current session, default server)
#[derive(Component)]
pub struct PlayNowButton;

/// Marker for the Singleplayer button (in-process server)
#[cfg(feature = "singleplayer")]
#[derive(Component)]
pub struct SingleplayerButton;

/// Marker for the Multiplayer button (go to multiplayer setup)
#[derive(Component)]
pub struct MultiplayerButton;

#[derive(Component)]
pub struct LoginButton(pub LoginMethod);

#[derive(Component)]
pub struct LogoutButton;

#[derive(Component)]
pub struct ClaimCodeInput;

#[derive(Component)]
pub struct ClaimButton;

/// Marker for the animated logo
#[derive(Component)]
pub struct AnimatedLogo {
    pub time: f32,
}

/// Spawn the home menu UI
pub fn spawn_home_menu(
    mut commands: Commands,
    image_assets: Res<ImageAssets>,
    session: Res<AuthSession>,
    methods: Res<AvailableLoginMethods>,
) {
    build_home_menu(&mut commands, &image_assets, &session, &methods);
}

/// Respawn the menu when the session or the login methods change (login, logout, `/me`
/// answering, providers loading).
pub fn rebuild_home_menu(
    mut commands: Commands,
    menu_query: Query<Entity, With<HomeMenu>>,
    image_assets: Res<ImageAssets>,
    session: Res<AuthSession>,
    methods: Res<AvailableLoginMethods>,
) {
    for entity in menu_query.iter() {
        commands.entity(entity).despawn();
    }
    build_home_menu(&mut commands, &image_assets, &session, &methods);
}

fn build_home_menu(
    commands: &mut Commands,
    image_assets: &ImageAssets,
    session: &AuthSession,
    methods: &AvailableLoginMethods,
) {
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

            spawn_menu_button(parent, "Play now", PlayNowButton);
            #[cfg(feature = "singleplayer")]
            spawn_menu_button(parent, "Singleplayer", SingleplayerButton);
            spawn_menu_button(parent, "Multiplayer", MultiplayerButton);

            if !session.is_account() && !methods.0.is_empty() {
                parent.spawn(wrapping_row()).with_children(|row| {
                    for &method in &methods.0 {
                        spawn_menu_button(row, method.label(), LoginButton(method));
                    }
                });
            }

            if session.is_account() {
                let name = session.display_name.as_deref().unwrap_or("?");
                parent.spawn(label_text(format!("Signed in as {name}")));
                parent.spawn(wrapping_row()).with_children(|row| {
                    spawn_claim_input(row);
                    spawn_menu_button(row, "Claim", ClaimButton);
                    spawn_menu_button(row, "Log out", LogoutButton);
                });
            } else if session.kind.as_deref() == Some("guest")
                && let Some(code) = &session.claim_code
            {
                parent.spawn(label_text(format!(
                    "Guest claim code: {code} (log in later and enter it to keep this character)"
                )));
            }
        });
}

fn wrapping_row() -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        column_gap: Val::Px(20.0),
        row_gap: Val::Px(20.0),
        ..default()
    }
}

fn spawn_menu_button(parent: &mut ChildSpawnerCommands, label: &str, marker: impl Bundle) {
    let (node, bg_color, border_color) = menu_button_bundle();
    let (text, font, color) = menu_button_text(label);
    parent
        .spawn((
            node,
            bg_color,
            border_color,
            Button,
            Hovered::default(),
            marker,
        ))
        .with_children(|button| {
            button.spawn((text, font, color));
        });
}

/// Same shape as the multiplayer menu's username input.
fn spawn_claim_input(parent: &mut ChildSpawnerCommands) {
    const PLACEHOLDER: &str = "XXXXX-XXXXX";
    parent
        .spawn((
            Node {
                width: Val::Px(300.0),
                height: Val::Px(50.0),
                border: UiRect::all(Val::Px(2.0)),
                padding: UiRect::all(Val::Px(10.0)),
                justify_content: JustifyContent::Start,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(Color::srgb(0.4, 0.4, 0.4)),
            BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
            Button,
            Hovered::default(),
            TextInput::new(PLACEHOLDER, PLACEHOLDER.len()),
            ClaimCodeInput,
        ))
        .with_children(|input_parent| {
            input_parent.spawn((
                Text::new(PLACEHOLDER),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
                TextInputDisplay,
            ));
        });
}

/// Despawn the home menu
pub fn despawn_home_menu(mut commands: Commands, menu_query: Query<Entity, With<HomeMenu>>) {
    for entity in menu_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Handle Play now / Multiplayer
pub fn handle_home_buttons(
    play_query: Query<(), (Added<Pressed>, With<PlayNowButton>)>,
    multiplayer_query: Query<(), (Added<Pressed>, With<MultiplayerButton>)>,
    session: Res<AuthSession>,
    mut auth_requests: MessageWriter<AuthRequest>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
    #[cfg(not(feature = "web"))] mut config: ResMut<crate::Config>,
    #[cfg(not(feature = "web"))] args: Res<crate::ClapArgs>,
) {
    // Play now - get a ticket (as a new guest if needed) and join the default server
    for () in play_query.iter() {
        info!("Play now pressed");
        auth_requests.write(session.ticket_request());
        // Web builds always connect to /ws/0 on the page's host.
        #[cfg(not(feature = "web"))]
        crate::apply_server_addr(&mut config, &args.play_server);
        next_menu_state.set(MenuState::Connecting);
    }

    // Multiplayer button - go to multiplayer setup
    for () in multiplayer_query.iter() {
        info!("Multiplayer button pressed");
        next_menu_state.set(MenuState::Multiplayer);
    }
}

/// Singleplayer - in-process server with ticket verification disabled, so no authserver round
/// trip: an empty ticket is ready immediately.
#[cfg(feature = "singleplayer")]
pub fn handle_singleplayer_button(
    singleplayer_query: Query<(), (Added<Pressed>, With<SingleplayerButton>)>,
    mut commands: Commands,
    mut config: ResMut<crate::Config>,
    mut next_auth_state: ResMut<NextState<crate::auth::AuthState>>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
) {
    for () in singleplayer_query.iter() {
        info!("Singleplayer button pressed - starting local server");
        use shared::netlib::NetworkConnectionTarget;
        let port = rand::random_range(20000..60000);
        config.port = port;
        config.ip = "127.0.0.1".to_string();
        config.host_ip = None;
        std::thread::spawn(move || {
            // TODO exit
            server::call_from_client_for_singleplayer(NetworkConnectionTarget {
                ip: "127.0.0.1".to_string(),
                port,
            });
        });
        commands.insert_resource(crate::auth::GameTicket(String::new()));
        next_auth_state.set(crate::auth::AuthState::Ready);
        next_menu_state.set(MenuState::Connecting);
    }
}

/// Handle Login / Log out / Claim
pub fn handle_account_buttons(
    login_query: Query<&LoginButton, Added<Pressed>>,
    logout_query: Query<(), (Added<Pressed>, With<LogoutButton>)>,
    claim_query: Query<(), (Added<Pressed>, With<ClaimButton>)>,
    claim_input: Query<&TextInput, With<ClaimCodeInput>>,
    mut auth_requests: MessageWriter<AuthRequest>,
) {
    for LoginButton(method) in login_query.iter() {
        info!("Login pressed: {method:?}");
        auth_requests.write(AuthRequest::Login(*method));
    }

    for () in logout_query.iter() {
        info!("Log out pressed");
        auth_requests.write(AuthRequest::Logout);
    }

    for () in claim_query.iter() {
        let Ok(input) = claim_input.single() else {
            continue;
        };
        let code = input.value.trim().to_uppercase();
        if code.is_empty() {
            continue;
        }
        auth_requests.write(AuthRequest::Claim(code));
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
