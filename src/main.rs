mod camera;
mod physics;
mod terrain;

use avian3d::prelude::*;
use bevy::prelude::*;

use camera::CameraPlugin;
use physics::PhysicsPlugin;
use terrain::TerrainPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PhysicsPlugins::default(),
            CameraPlugin,
            TerrainPlugin,
            PhysicsPlugin,
        ))
        .insert_resource(Gravity(Vec3::new(0.0, -9.81, 0.0)))
        .insert_resource(ClearColor(Color::srgb(0.4, 0.7, 1.0))) // Sky blue
        .run();
}
