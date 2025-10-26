mod camera;
pub mod game_state;
mod grass;
mod network;
pub mod notification;
mod physics;
mod picking;
mod terrain;
mod water;

use avian3d::prelude::*;
use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};

use camera::CameraPlugin;
use grass::GrassPlugin;
use physics::PhysicsPlugin;
use picking::PickingPlugin;
use terrain::TerrainPlugin;
use water::WaterPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PhysicsPlugins::default(),
            CameraPlugin,
            TerrainPlugin,
            PhysicsPlugin,
            PickingPlugin,
            network::NetworkingPlugin,
            game_state::StatePlugin,
            WaterPlugin,
            GrassPlugin,
            // Diagnostics
            FrameTimeDiagnosticsPlugin::default(),
            LogDiagnosticsPlugin::default(),
        ))
        .insert_resource(Gravity(Vec3::new(0.0, -9.81, 0.0)))
        .insert_resource(ClearColor(Color::srgb(0.4, 0.7, 1.0))) // Sky blue
        .run();
}
