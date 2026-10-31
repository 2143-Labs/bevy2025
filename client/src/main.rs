#![allow(clippy::type_complexity)]
mod animations;
mod assets;
mod auth;
mod camera;
mod character_controller_client;
mod debug;
pub mod game_state;
mod grass;
mod network;
pub mod notification;
mod physics;
mod picking;
mod projectile;
mod remote_players;
mod terrain;
mod ui;
mod water;

#[cfg(feature = "web")]
mod web;

use bevy::{diagnostic::LogDiagnosticsPlugin, prelude::*};

use assets::AssetsPlugin;
use camera::CameraPlugin;
use clap::Parser;
use debug::DebugPlugin;
use grass::GrassPlugin;
use physics::PhysicsPlugin;
use picking::PickingPlugin;
use remote_players::RemotePlayersPlugin;
use shared::Config;
use terrain::TerrainPlugin;
use ui::UIPlugin;
use water::WaterPlugin;

#[derive(Parser, Resource, Debug)]
struct ClapArgs {
    #[clap(long)]
    print_binds: bool,
    #[clap(long)]
    print_config: bool,
    #[clap(long)]
    autoconnect: Option<String>,
    /// Authserver base URL (rots-authserver behind `/auth`)
    #[cfg(not(feature = "web"))]
    #[clap(long, default_value = "https://rots.john2143.com/auth")]
    auth_url: String,
    /// Game server used by "Play now", as `host:port` or `host`
    #[cfg(not(feature = "web"))]
    #[clap(long, default_value = "rots.john2143.com:35000")]
    play_server: String,
    /// If set, will simulate a fake ping to the server with the given ms delay. See also
    /// --fake-ping-inbound, --fake-ping-outbound, --fake-ping-jitter
    #[clap(long, short = 'p')]
    fake_ping: Option<u64>,
    #[clap(long)]
    fake_ping_inbound: Option<u64>,
    #[clap(long)]
    fake_ping_outbound: Option<u64>,
    /// If set, will add jitter equal to `0..fake_ping` to the fake ping with the given ms max jitter
    #[clap(long)]
    fake_ping_jitter: Option<u64>,
}

#[cfg(not(feature = "web"))]
fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let runtime = std::sync::Arc::new(runtime);

    let runtime2 = runtime.clone();
    runtime.block_on(async {
        main_client(runtime2);
    });
}

#[cfg(feature = "web")]
fn main() {
    main_client();
}

fn main_client(#[cfg(not(feature = "web"))] runtime: std::sync::Arc<tokio::runtime::Runtime>) {
    let mut args = ClapArgs::parse();

    if args.print_binds {
        println!("{:?}", Config::load_from_main_dir().keybindings);
        return;
    }

    if args.print_config {
        println!("{}", Config::default_config_str());
        return;
    }

    // If we are building a static release, then just add the autoconnect argument by default
    if args.autoconnect.is_none() && std::option_env!("BUILD_CTX") == Some("action") {
        args.autoconnect = Some("main".to_string());
    }

    let mut app = App::new();

    // bevy_steamworks must be added before DefaultPlugins (RenderPlugin).
    #[cfg(feature = "steam")]
    auth::steam::init(&mut app);

    #[cfg(feature = "web")]
    {
        app.add_plugins(web::WebPlugin);
    }

    #[cfg(not(feature = "web"))]
    {
        app.add_plugins(DefaultPlugins);
    }

    // Login backends are registered here only; `auth` itself is platform-agnostic.
    app.add_plugins(auth::AuthPlugin);
    #[cfg(not(feature = "web"))]
    app.add_plugins(auth::native::NativeAuthPlugin {
        runtime,
        auth_url: args.auth_url.clone(),
    });
    #[cfg(feature = "web")]
    app.add_plugins(auth::web::WebAuthPlugin);

    app.add_plugins((
        game_state::StatePlugin,
        AssetsPlugin,
        DebugPlugin,
        UIPlugin,
        CameraPlugin,
        TerrainPlugin,
        PhysicsPlugin,
        PickingPlugin,
        network::NetworkingPlugin,
        RemotePlayersPlugin,
        shared::ConfigPlugin,
        notification::NotificationPlugin,
        WaterPlugin,
        shared::TickPlugin,
        // Too many plugins here
    ))
    .add_plugins((
        GrassPlugin,
        LogDiagnosticsPlugin::default(),
        shared::event::client::NetworkEventPlugin,
        shared::character_controller::CharacterControllerPlugin,
        character_controller_client::ClientCharacterControllerPlugin,
        animations::CharacterAnimationPlugin,
        projectile::ProjectilePlugin,
    ))
    .insert_resource(ClearColor(Color::srgb(0.4, 0.7, 1.0))) // Sky blue
    .insert_resource(args)
    .add_systems(Startup, check_all_clap_args);

    app.run();
}

/// This looks for the clap args like autoconnect and modifys the config if neede
fn check_all_clap_args(mut config: ResMut<Config>, args: Res<ClapArgs>) {
    if let Some(ip_and_port) = &args.autoconnect {
        apply_server_addr(&mut config, ip_and_port);
    }
}

/// Point `config` at `ip:port`, or just `ip` keeping the configured port.
fn apply_server_addr(config: &mut Config, addr: &str) {
    let mut parts = addr.split(':');
    let ip_as_str = parts.next();
    let port: Option<u16> = parts.next().and_then(|s| s.parse().ok());

    if let Some(ip) = ip_as_str {
        config.ip = ip.to_string();
    }
    if let Some(port) = port {
        config.port = port;
    }
}
