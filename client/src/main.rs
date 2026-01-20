#![allow(clippy::type_complexity)]
mod animations;
mod assets;
mod camera;
mod character_controller_client;
mod debug;
pub mod game_state;
mod grass;
mod login;
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

#[cfg(feature = "steam")]
use bevy_steamworks::AppId;
#[cfg(feature = "steam")]
mod steamworks;

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
    #[clap(long, default_value = "http://192.168.1.32:8002")]
    login_server: String,
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
use tokio::runtime::Runtime;
#[cfg(not(feature = "web"))]
fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let runtime = std::sync::Arc::new(runtime);

    let runtime2 = runtime.clone();
    runtime.block_on(async {
        main_client(Some(runtime2));
    });
}

#[cfg(feature = "web")]
pub struct Runtime;
#[cfg(feature = "web")]
fn main() {
    main_client(None);
}

fn main_client(runtime: Option<std::sync::Arc<Runtime>>) {
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

    #[cfg(feature = "web")]
    {
        app.add_plugins(web::WebPlugin);
    }

    #[cfg(not(feature = "web"))]
    {
        app.add_plugins(DefaultPlugins);
    }

    #[cfg(feature = "steam")]
    {
        app.add_plugins((steamworks::SteamworksPlugin::new(AppId(440), runtime.unwrap().clone()),));
    }

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
        // 2 choices: ip:port or just ip and then default port
        let mut parts = ip_and_port.split(':');
        let ip_as_str = parts.next();
        let port: Option<u16> = parts.next().and_then(|s| s.parse().ok());

        if let Some(ip) = ip_as_str {
            config.ip = ip.to_string();
        }
        if let Some(port) = port {
            config.port = port;
        }
    }
}
