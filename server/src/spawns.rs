use avian3d::prelude::RigidBody;
use bevy::prelude::*;
use shared::{
    character_controller::{CharacterController, NPCController},
    event::{
        client::{BeginThirdpersonControllingUnit, SpawnUnit2},
        server::{SpawnCircle, SpawnMan},
        NetEntId, PlayerId, UDPacketEvent,
    },
    net_components::{
        make_man, make_small_loot,
        ours::{ControlledBy, Dead, DespawnOnPlayerDisconnect, HasInventory},
        ToNetComponent,
    },
    netlib::{EventToClient, ServerNetworkingResources},
    CurrentTick,
};

use crate::{make_ball, ConnectedPlayer, EndpointToPlayerId, PlayerEndpoint, ServerState};

pub struct SpawnPlugin;
impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (on_circle_spawn, on_man_spawn, on_unit_die)
                //.run_if(on_timer(Duration::from_millis(10)))
                .run_if(in_state(ServerState::Running)),
        );

        app.add_message::<UnitDie>();
        //.add_systems(
        //Update,
        //(send_networked_Spawn_move)
        //.run_if(in_state(ServerState::Running))
        //.run_if(on_timer(Duration::from_millis(50))),
        //);
    }
}

#[derive(Default)]
struct CircleSpawnCooldown {
    player_to_last_spawn: std::collections::HashMap<PlayerId, f64>,
}

fn on_circle_spawn(
    mut spawns: UDPacketEvent<SpawnCircle>,
    mut commands: Commands,
    endpoint_to_player_id: Res<EndpointToPlayerId>,
    sr: Res<ServerNetworkingResources>,
    clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
    time: Res<Time>,
    mut circle_spawn_cooldown: Local<CircleSpawnCooldown>,
) {
    for spawn_ev in spawns.read() {
        info!(?spawn_ev.event, "Spawning circle from event");
        let spawn = &spawn_ev.event;

        let Some(player_id_of_spawner) = endpoint_to_player_id.map.get(&spawn_ev.endpoint) else {
            warn!(
                "Could not find player ID for endpoint: {:?}",
                spawn_ev.endpoint
            );
            continue;
        };

        let now = time.elapsed_secs_f64();
        if let Some(last_spawn_time) = circle_spawn_cooldown
            .player_to_last_spawn
            .get(&player_id_of_spawner)
        {
            let time_since_last_spawn = now - *last_spawn_time;
            if time_since_last_spawn < 1.0 {
                info!(
                    "Player {:?} tried to spawn circle too quickly ({}s since last spawn)",
                    player_id_of_spawner, time_since_last_spawn
                );
                continue;
            }
        }
        circle_spawn_cooldown
            .player_to_last_spawn
            .insert(*player_id_of_spawner, now);

        debug!("Spawning circle at position: {:?}", spawn.position);
        let transform = Transform::from_translation(spawn.position);

        let mut unit;

        if rand::random_bool(0.5) {
            info!("Spawning a surprise goblin instead of a ball!");
            unit = make_small_loot(transform);
            let inventory = shared::items::goblin_drops();
            unit.components.push(
                HasInventory {
                    inventory_id: inventory.id,
                }
                .to_net_component(),
            );

            sr.send_outgoing_event_next_tick(
                spawn_ev.endpoint,
                &EventToClient::NewInventory(shared::event::client::NewInventory { inventory }),
            );
        } else {
            unit = make_ball(
                transform,
                spawn.color,
                ControlledBy::single(*player_id_of_spawner),
            );
        }
        let unit_ent = unit.clone().spawn_entity(&mut commands);
        commands.entity(unit_ent).insert(DespawnOnPlayerDisconnect {
            player_id: *player_id_of_spawner,
        });

        // Notify all clients about the new unit
        let event = EventToClient::SpawnUnit2(unit);
        info!("Notifying clients of new unit: {:?}", event);
        for endpoint in &clients {
            info!("Sending spawn event to endpoint: {:?}", endpoint.0);
            sr.send_outgoing_event_next_tick(endpoint.0, &event);
        }
    }
}

fn on_man_spawn(
    mut spawns: UDPacketEvent<SpawnMan>,
    mut commands: Commands,
    endpoint_to_player_id: Res<EndpointToPlayerId>,
    this_players_existing_units: Query<
        (&NetEntId, &ControlledBy, Entity),
        (With<shared::net_components::ents::Man>, Without<Dead>),
    >,
    mut unit_kill: MessageWriter<UnitDie>,
    sr: Res<ServerNetworkingResources>,
    clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
) {
    for spawn_ev in spawns.read() {
        info!(?spawn_ev.event, "Spawning man from event");
        let spawn = &spawn_ev.event;

        let Some(player_id_of_spawner) = endpoint_to_player_id.map.get(&spawn_ev.endpoint) else {
            warn!(
                "Could not find player ID for endpoint: {:?}",
                spawn_ev.endpoint
            );
            continue;
        };

        debug!("Spawning man at position: {:?}", spawn.position);
        let transform = Transform::from_translation(spawn.position);

        // for now
        let inventory = shared::items::goblin_drops();
        //TODO add to server inventory

        let mut unit = make_man(
            transform,
            ControlledBy::single(*player_id_of_spawner),
            &spawn.controller_type,
        );
        unit.components.push(
            HasInventory {
                inventory_id: inventory.id,
            }
            .to_net_component(),
        );

        let unit_ent = unit.clone().spawn_entity(&mut commands);
        commands.entity(unit_ent).insert(DespawnOnPlayerDisconnect {
            player_id: *player_id_of_spawner,
        });

        let event = EventToClient::SpawnUnit2(unit.clone());
        info!("Notifying clients of new unit: {:?}", event);
        for endpoint in &clients {
            info!("Sending spawn event to endpoint: {:?}", endpoint.0);
            sr.send_outgoing_event_next_tick(endpoint.0, &event);
        }

        // Now, we send the user control event to this client
        sr.send_outgoing_event_next_tick(
            spawn_ev.endpoint,
            &EventToClient::BeginThirdpersonControllingUnit(BeginThirdpersonControllingUnit {
                player_id: *player_id_of_spawner,
                unit: Some(unit.net_ent_id),
            }),
        );

        sr.send_outgoing_event_next_tick(
            spawn_ev.endpoint,
            &EventToClient::NewInventory(shared::event::client::NewInventory { inventory }),
        );

        // kill the existing units that this player controls
        for (net_id, controlled_by, _ent) in this_players_existing_units.iter() {
            if controlled_by.players.contains(&*player_id_of_spawner) {
                info!(
                    "Killing existing unit {:?} controlled by player {:?}",
                    net_id, player_id_of_spawner
                );
                unit_kill.write(UnitDie { unit_id: *net_id });
            }
        }
    }
}

#[derive(Message)]
pub struct UnitDie {
    pub unit_id: NetEntId,
}

fn on_unit_die(
    mut unit_deaths: MessageReader<UnitDie>,
    mut commands: Commands,
    units: Query<(&NetEntId, Option<&HasInventory>, &Transform, Entity), Without<Dead>>,
    sr: Res<ServerNetworkingResources>,
    tick: Res<CurrentTick>,
    clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
) {
    for death in unit_deaths.read() {
        info!("Unit died: {:?}", death.unit_id);
        let death_event = Dead {
            reason: "Died".to_string(),
            died_on_tick: tick.0,
        };
        // Despawn the unit
        for (net_id, has_inv, loc, ent) in units.iter() {
            if *net_id == death.unit_id {
                //TODO dedup this with client

                let mut angular_velocity = avian3d::prelude::AngularVelocity::default();
                angular_velocity.0 = Vec3::new(
                    rand::random_range(-5.0..5.0),
                    rand::random_range(-5.0..5.0),
                    rand::random_range(-5.0..5.0),
                );

                let mut linear_velocity = avian3d::prelude::LinearVelocity::default();
                linear_velocity.0 = Vec3::new(
                    rand::random_range(-2.0..2.0),
                    rand::random_range(2.0..5.0),
                    rand::random_range(-2.0..2.0),
                );

                commands
                    .entity(ent)
                    .insert(death_event.clone())
                    .insert(RigidBody::Dynamic)
                    .insert(linear_velocity.clone())
                    .insert(angular_velocity.clone())
                    .remove::<NPCController>()
                    .remove::<CharacterController>();

                // Notify all clients about the unit death
                let event = EventToClient::UpdateUnit2(shared::event::client::UpdateUnit2 {
                    net_ent_id: death.unit_id,
                    changed_components: vec![
                        linear_velocity.to_net_component(),
                        angular_velocity.to_net_component(),
                    ],
                    removed_components: vec![
                        "NPCController".to_string(),
                        "CharacterController".to_string(),
                        "RigidBody".to_string(),
                    ],
                    new_component: vec![
                        death_event.clone().to_net_component(),
                        RigidBody::Dynamic.to_net_component(),
                    ],
                });

                for endpoint in &clients {
                    sr.send_outgoing_event_next_tick(endpoint.0, &event);
                }

                if let Some(inv) = has_inv {
                    let position = loc.translation;
                    let loot = SpawnUnit2 {
                        net_ent_id: NetEntId::random(),
                        components: vec![
                            shared::net_components::ents::ItemDrop { source: None }
                                .to_net_component(),
                            Transform::from_translation(position).to_net_component(),
                            HasInventory {
                                inventory_id: inv.inventory_id,
                            }
                            .to_net_component(),
                        ],
                    };
                    let event = EventToClient::SpawnUnit2(loot);
                    for endpoint in &clients {
                        sr.send_outgoing_event_next_tick(endpoint.0, &event);
                    }
                }
            }
        }
    }
}

//fn send_networked_Spawn_move(
//Spawns: Query<
//(&Transform, &MovementIntention, &AttackIntention, &NetEntId),
//(
//With<AIType>,
//Or<(
//Changed<Transform>,
//Changed<MovementIntention>,
//Changed<AttackIntention>,
//)>,
//),
//>,
//clients: Query<&PlayerEndpoint, With<ConnectedPlayerName>>,
//sr: Res<ServerResources<EventToServer>>,
//) {
//let mut all_events = vec![];
//for (&movement, mi, attack_intent, &id) in &Spawns {
//all_events.extend([
//EventToClient::SomeoneMoved(SomeoneMoved {
//id,
//movement: shared::event::server::ChangeMovement::SetTransform(movement),
//}),
//EventToClient::SomeoneMoved(SomeoneMoved {
//id,
//movement: shared::event::server::ChangeMovement::Move2d(mi.0),
//}),
//EventToClient::SomeoneMoved(SomeoneMoved {
//id,
//movement: shared::event::server::ChangeMovement::AttackIntent(
//attack_intent.clone(),
//),
//}),
//]);
//}

//if !all_events.is_empty() {
//for event_list in all_events.chunks(250) {
//for endpoint in &clients {
//send_event_to_server_batch(&sr.handler, endpoint.0, event_list);
//}
//}
//}
//}
