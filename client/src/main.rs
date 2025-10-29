mod camera;
pub mod game_state;
mod grass;
mod network;
pub mod notification;
mod physics;
mod picking;
mod terrain;
mod water;
mod ui;

use avian3d::prelude::*;
use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};

use camera::CameraPlugin;
use clap::Parser;
use grass::GrassPlugin;
use physics::PhysicsPlugin;
use picking::PickingPlugin;
use shared::Config;
use terrain::TerrainPlugin;
use water::WaterPlugin;
use ui::UIPlugin;

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
            PhysicsPlugins::default(),
            UIPlugin,
            CameraPlugin,
            TerrainPlugin,
            PhysicsPlugin,
            PickingPlugin,
            network::NetworkingPlugin,
            shared::ConfigPlugin,
            notification::NotificationPlugin,
            game_state::StatePlugin,
            WaterPlugin,
            GrassPlugin,
            // Diagnostics
            FrameTimeDiagnosticsPlugin::default(),
            LogDiagnosticsPlugin::default(),
        ))
        .insert_resource(Gravity(Vec3::new(0.0, -9.81, 0.0)))
        .insert_resource(ClearColor(Color::srgb(0.4, 0.7, 1.0))) // Sky blue
        .insert_resource(args)
        .run();
}
