mod camera;
pub mod player;
pub mod inventory;
pub mod game_state;
mod grass;
mod network;
pub mod notification;
mod physics;
mod picking;
mod terrain;
mod ui;
mod water;
mod assets;
mod debug;
mod remote_players;

use bevy::{
    diagnostic::LogDiagnosticsPlugin,
    prelude::*,
};

use camera::CameraPlugin;
use clap::Parser;
use grass::GrassPlugin;
use physics::PhysicsPlugin;
use picking::PickingPlugin;
use shared::Config;
use terrain::TerrainPlugin;
use ui::{InventoryPlugin, UIPlugin};
use water::WaterPlugin;
use assets::AssetsPlugin;
use debug::DebugPlugin;
use remote_players::RemotePlayersPlugin;

#[derive(Parser, Resource, Debug)]
struct ClapArgs {
    #[clap(long)]
    print_binds: bool,
    #[clap(long)]
    print_config: bool,
    #[clap(long)]
    autoconnect: Option<String>,
}

fn main() {
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

    App::new()
        .add_plugins((
            DefaultPlugins,
            game_state::StatePlugin,
            AssetsPlugin,
            DebugPlugin,
            UIPlugin,
            InventoryPlugin,
            CameraPlugin,
            TerrainPlugin,
            PhysicsPlugin,
            PickingPlugin,
            network::NetworkingPlugin,
            RemotePlayersPlugin,
            shared::ConfigPlugin,
            notification::NotificationPlugin,
            WaterPlugin,
        ))
        .add_plugins((
            GrassPlugin,
            LogDiagnosticsPlugin::default(),
        ))
        .insert_resource(ClearColor(Color::srgb(0.4, 0.7, 1.0))) // Sky blue
        .insert_resource(args)
        .run();
}
