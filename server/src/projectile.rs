use avian3d::prelude::CollisionStart;
use bevy::prelude::*;
use shared::{
    event::{NetEntId, client::SpawnProjectile},
    netlib::ServerNetworkingResources,
    projectile::{ProjectileAI, ProjectileRealtime, ProjectileSource},
};

use crate::{ConnectedPlayer, PlayerEndpoint, ServerState, spawns::UnitDie};

pub struct ProjectilePlugin;

impl Plugin for ProjectilePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(shared::projectile::ProjectilePlugin);
        app.add_message::<ProjectileCollisionLocalServer>();
        app.add_systems(
            Update,
            (network_projectiles, read_projectile_collision_local_server)
                .run_if(in_state(ServerState::Running)),
        );
    }
}

fn network_projectiles(
    mut messager_reader: MessageReader<SpawnProjectile>,
    connected_clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
    mut commands: Commands,
    time: Res<Time>,
    tick: Res<shared::CurrentTick>,
    sr: Res<ServerNetworkingResources>,
) {
    let mut events_collected = vec![];

    for event in messager_reader.read() {
        events_collected.push(crate::EventToClient::SpawnProjectile(event.clone()));
        // spawn it in the world as well
        let mut ec = commands.spawn((
            event.base_bundle(&tick.0),
            ProjectileRealtime {
                spawn_real_time: time.elapsed_secs_f64(),
            },
        ));

        if let Some(collider) = event.collider_bundle() {
            ec.insert(collider);
        }

        // on the server, setup observers for collisions
        ec.observe(on_projectile_collision);
    }

    if events_collected.is_empty() {
        return;
    }

    for client in connected_clients {
        sr.send_outgoing_event_next_tick_batch(client.0, &events_collected);
    }
}

#[derive(Message, Debug, Clone, PartialEq)]
pub struct ProjectileCollisionLocalServer {
    pub projectile_entity: Entity,
    pub hit_entity: Entity,
    pub net_ent_id: NetEntId,
}

fn on_projectile_collision(
    coll_event: On<CollisionStart>,
    units: Query<(Entity, &NetEntId)>,
    proj_data: Query<(&ProjectileSource, &ProjectileAI)>,
    mut proj_hit_writer: MessageWriter<ProjectileCollisionLocalServer>,
) {
    let proj_collider = coll_event.collider1;
    let unit_collider = coll_event.collider2;

    let Ok((ent, unit_net_ent_id)) = units.get(unit_collider) else {
        return;
    };

    let Ok((proj_source, _proj_ai)) = proj_data.get(proj_collider) else {
        error!("Projectile collider without projectile data?");
        return;
    };

    if proj_source.source_entity == *unit_net_ent_id {
        // don't hit self
        return;
    }

    proj_hit_writer.write(ProjectileCollisionLocalServer {
        projectile_entity: proj_collider,
        hit_entity: ent,
        net_ent_id: *unit_net_ent_id,
    });
}

fn read_projectile_collision_local_server(
    mut proj_hit_reader: MessageReader<ProjectileCollisionLocalServer>,
    //ents_hit: Query<(Entity, &NetEntId)>,
    proj_data: Query<(&ProjectileSource, &ProjectileAI)>,
    //connected_clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
    //tick: Res<shared::CurrentTick>,
    //sr: Res<ServerNetworkingResources>,
    mut unit_die: MessageWriter<UnitDie>,
) {
    for ProjectileCollisionLocalServer {
        projectile_entity,
        hit_entity: _,
        net_ent_id,
    } in proj_hit_reader.read()
    {
        let Ok((_proj_source, _proj_ai)) = proj_data.get(*projectile_entity) else {
            error!("Projectile collider without projectile data?");
            return;
        };

        unit_die.write(UnitDie {
            unit_id: *net_ent_id,
        });
    }
}
