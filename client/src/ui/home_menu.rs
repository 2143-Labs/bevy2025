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

/// Marker for the Enter testing button (host the testing gym on this machine, or join it)
#[cfg(feature = "testing")]
#[derive(Component)]
pub struct EnterTestingButton;

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
            #[cfg(feature = "testing")]
            spawn_menu_button(parent, "Enter testing", EnterTestingButton);

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

/// Text input sized like a menu button.
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

/// Handle Play now
pub fn handle_home_buttons(
    play_query: Query<(), (Added<Pressed>, With<PlayNowButton>)>,
    session: Res<AuthSession>,
    mut auth_requests: MessageWriter<AuthRequest>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
    #[cfg(not(feature = "web"))] mut commands: Commands,
    #[cfg(not(feature = "web"))] mut config: ResMut<crate::Config>,
    #[cfg(not(feature = "web"))] args: Res<crate::ClapArgs>,
    #[cfg(not(feature = "web"))] mut next_auth_state: ResMut<NextState<crate::auth::AuthState>>,
) {
    // Play now - get placed by the authserver (as a new guest if needed) and connect where it
    // says; the address arrives with the ticket (`auth::AssignedServer`).
    for () in play_query.iter() {
        info!("Play now pressed");
        // Dev only: connect straight to --play-server with an empty ticket, no login.
        #[cfg(not(feature = "web"))]
        if let Some(addr) = &args.play_server {
            warn!("Play now -> {addr} (--play-server: skipping login and the ticket)");
            crate::apply_server_addr(&mut config, addr);
            commands.insert_resource(crate::auth::GameTicket(String::new()));
            next_auth_state.set(crate::auth::AuthState::Ready);
            next_menu_state.set(MenuState::Connecting);
            continue;
        }
        auth_requests.write(session.ticket_request());
        next_menu_state.set(MenuState::Connecting);
    }
}

/// Connect to a testing gym. Gyms accept any ticket, so an empty one is ready immediately.
#[cfg(feature = "testing")]
pub(crate) fn connect_to_gym(
    ip: String,
    port: u16,
    commands: &mut Commands,
    config: &mut crate::Config,
    next_auth_state: &mut NextState<crate::auth::AuthState>,
    next_menu_state: &mut NextState<MenuState>,
) {
    config.ip = ip;
    config.port = port;
    config.host_ip = None;
    commands.insert_resource(crate::auth::GameTicket(String::new()));
    next_auth_state.set(crate::auth::AuthState::Ready);
    next_menu_state.set(MenuState::Connecting);
}

/// Enter testing - host the gym on port 2143, or join it if this machine already hosts one.
#[cfg(feature = "testing")]
pub fn handle_enter_testing_button(
    testing_query: Query<(), (Added<Pressed>, With<EnterTestingButton>)>,
    session: Res<AuthSession>,
    mut commands: Commands,
    mut config: ResMut<crate::Config>,
    mut next_auth_state: ResMut<NextState<crate::auth::AuthState>>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
) {
    use shared::gym::GYM_GAME_PORT;
    for () in testing_query.iter() {
        match std::net::UdpSocket::bind(("0.0.0.0", GYM_GAME_PORT)) {
            Ok(probe) => {
                drop(probe);
                info!(
                    "Enter testing pressed - hosting gym on port {}",
                    GYM_GAME_PORT
                );
                // The name other players see in the LAN gym list.
                let host_name = session
                    .display_name
                    .clone()
                    .filter(|_| session.is_account())
                    .unwrap_or_else(|| {
                        let user = std::env::var("USER")
                            .or_else(|_| std::env::var("USERNAME"))
                            .unwrap_or_else(|_| "player".to_string());
                        format!("{user}@{}", gethostname::gethostname().to_string_lossy())
                    });
                std::thread::spawn(move || {
                    // The client owns the window, event loop and global logger; the server must
                    // not create its own, so it runs on a plain schedule loop instead.
                    let plugins = DefaultPlugins
                        .build()
                        .disable::<bevy::log::LogPlugin>()
                        .disable::<bevy::winit::WinitPlugin>()
                        .set(bevy::window::WindowPlugin {
                            primary_window: None,
                            exit_condition: bevy::window::ExitCondition::DontExit,
                            close_when_requested: false,
                            ..default()
                        })
                        .add(bevy::app::ScheduleRunnerPlugin::run_loop(
                            server::SERVER_LOOP_WAIT,
                        ));
                    // The gym thread never returns: main() ends the process without tearing it
                    // down (see main_client), so the gym is not signalled to stop.
                    server::host_testing_server(plugins, host_name);
                });
            }
            Err(e) => info!(
                "Enter testing pressed - port {} busy ({e}), joining the local gym",
                GYM_GAME_PORT
            ),
        }
        connect_to_gym(
            "127.0.0.1".to_string(),
            GYM_GAME_PORT,
            &mut commands,
            &mut config,
            &mut next_auth_state,
            &mut next_menu_state,
        );
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
