use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use shared::{
    CurrentTick,
    event::{UDPacketEvent, client::SpawnProjectile},
    net_components::ours::Dead,
    projectile::{ProjectileAI, ProjectileRealtime},
};

use crate::{animations::get_client_tick_from_server_tick, network::ServerTick};

pub struct ProjectilePlugin;

impl Plugin for ProjectilePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(shared::projectile::ProjectilePlugin);

        app.add_systems(Update, (on_spawn_projectile, spawn_projectiles_read));

        app.add_systems(Update, update_dead_units);

        app.add_systems(Startup, |world: &mut World| {
            world
                .register_component_hooks::<Dead>()
                .on_add(on_client_user_die);
        });
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
        trace!(?event.projectile_type, ?event.projectile_origin, "Spawning projectile");
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

        let mut ec = commands.spawn((
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

        if let Some(collider) = event.collider_bundle() {
            ec.insert(collider);
        }
    }
}

fn spawn_projectiles_read(
    mut efre: UDPacketEvent<SpawnProjectile>,
    mut writer: MessageWriter<SpawnProjectile>,
) {
    for packet in efre.read() {
        writer.write(packet.event.clone());
    }
}

#[derive(Component)]
pub struct DeathAnimation {
    pub started_at: f64,
}

/// This is called when a unit receives the Dead component
fn on_client_user_die(mut cmds: DeferredWorld, hc: HookContext) {
    info!("Unit {:?} died, starting death animation", hc.entity);
    let time = cmds.resource::<Time>().elapsed_secs_f64();
    cmds.commands()
        .entity(hc.entity)
        //.remove::<RigidBody>()
        // ragdoll
        .insert(DeathAnimation { started_at: time });

    // print all components on the entity for debugging
    for comp in cmds.entity(hc.entity).archetype().components() {
        info!("Component on dead entity: {:?}", comp);
        let type_id = cmds
            .components()
            .get_info(*comp)
            .unwrap()
            .type_id()
            .unwrap();

        // SAFETY: Trust that bevy gives us a valid type id and pointer from `get_by_id`
        if let Some(net_comp) = unsafe {
            shared::net_components::NetComponent::from_type_id_ptr(
                type_id,
                cmds.entity(hc.entity).get_by_id(*comp).unwrap(),
            )
        } {
            info!("Component to send: {:?}", net_comp);
            //spawn_unit.components.push(net_comp);
        }
    }
}

fn update_dead_units(
    mut commands: Commands,
    time: Res<Time>,
    query: Query<(Entity, &mut Transform, &DeathAnimation), With<Dead>>,
) {
    for (e, mut _transform, death_anim) in query {
        let elapsed = time.elapsed_secs_f64() - death_anim.started_at;
        if elapsed >= 10.0 {
            commands.entity(e).despawn();
        }
    }
}
