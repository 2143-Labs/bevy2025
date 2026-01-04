use bevy::prelude::*;
use shared::{CurrentTick, event::client::SpawnProjectile, projectile::ProjectileRealtime};

use crate::{animations::get_client_tick_from_server_tick, network::ServerTick};

pub struct ProjectilePlugin;

impl Plugin for ProjectilePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(shared::projectile::ProjectilePlugin);

        app.add_systems(
            Update,
            (
                on_spawn_projectile,
                spawn_projectiles_read,
            ),
        );
    }
}

fn on_spawn_projectile(
    mut spawn_event_reader: MessageReader<SpawnProjectile>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tick: Res<CurrentTick>,
    server_tick: Res<ServerTick>,
    time: Res<Time>,
) {
    for event in spawn_event_reader.read() {
        info!(?event.projectile_type, ?event.projectile_origin, "Spawning projectile");
        let (real_time, real_tick) =
            get_client_tick_from_server_tick(&event.spawn_tick, &time, &tick, &server_tick);

        let mesh_handle = match event.projectile_type {
            ProjectileAI::Spark { .. } => meshes.add(Mesh::from(Tetrahedron {
                vertices: [
                    Vec3::new(0.0, 0.5, 0.0),
                    Vec3::new(-0.5, -0.5, 0.5),
                    Vec3::new(0.5, -0.5, 0.5),
                    Vec3::new(0.0, -0.5, -0.5),
                ],
            })),
            ProjectileAI::HammerDin { .. } => meshes.add(Mesh::from(Sphere { radius: 1.0 })),
            _ => {
                error!("Unknown projectile type for mesh!");
                continue;
            }
        };

        let projectile_source = event.projectile_source.clone();

        commands.spawn((
            event.base_bundle(&real_tick),
            ProjectileRealtime {
                spawn_real_time: real_time,
            },
            // Basic equilateral tetrahedron mesh
            Mesh3d(mesh_handle),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::linear_rgb(1.0, 0.5, 0.0),
                unlit: true,
                ..Default::default()
            })),
        ));
    }
}

fn spawn_projectiles_read(
    efre: UDPacketEvent<SpawnProjectile>,
    writer: MessageWriter<SpawnProjectile>,
) {
    for packet in efre.read() {
        writer.write(packet.event.clone());
    }
}

