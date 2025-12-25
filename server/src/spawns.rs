use bevy::prelude::*;
use shared::{
    event::{
        NetEntId, UDPacketEvent, client::BeginThirdpersonControllingUnit, server::{SpawnCircle, SpawnMan}
    },
    net_components::{
        make_man,
        ours::{ControlledBy, DespawnOnPlayerDisconnect, HasInventory},
    },
    netlib::{EventToClient, ServerNetworkingResources, send_outgoing_event_next_tick},
};

use crate::{make_ball, ConnectedPlayer, EndpointToPlayerId, PlayerEndpoint, ServerState};

pub struct SpawnPlugin;
impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (on_circle_spawn, on_man_spawn)
                //.run_if(on_timer(Duration::from_millis(10)))
                .run_if(in_state(ServerState::Running)),
        );
        //.add_systems(
        //Update,
        //(send_networked_Spawn_move)
        //.run_if(in_state(ServerState::Running))
        //.run_if(on_timer(Duration::from_millis(50))),
        //);
    }
}

fn on_circle_spawn(
    mut spawns: UDPacketEvent<SpawnCircle>,
    mut commands: Commands,
    endpoint_to_player_id: Res<EndpointToPlayerId>,
    sr: Res<ServerNetworkingResources>,
    clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
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

        debug!("Spawning circle at position: {:?}", spawn.position);
        let transform = Transform::from_translation(spawn.position);
        let ent_id = NetEntId::random();

        let unit = make_ball(
            ent_id,
            transform,
            spawn.color,
            ControlledBy::single(*player_id_of_spawner),
        );
        let unit_ent = unit.clone().spawn_entity(&mut commands);
        commands.entity(unit_ent).insert(DespawnOnPlayerDisconnect {
            player_id: *player_id_of_spawner,
        });

        // Notify all clients about the new unit
        let event = EventToClient::SpawnUnit2(unit);
        info!("Notifying clients of new unit: {:?}", event);
        for endpoint in &clients {
            info!("Sending spawn event to endpoint: {:?}", endpoint.0);
            send_outgoing_event_next_tick(&sr, endpoint.0, &event);
        }
    }
}

fn on_man_spawn(
    mut spawns: UDPacketEvent<SpawnMan>,
    mut commands: Commands,
    endpoint_to_player_id: Res<EndpointToPlayerId>,
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
        let ent_id = NetEntId::random();

        // for now
        let inventory = shared::items::goblin_drops();
        //TODO add to server inventory

        let unit = make_man(
            ent_id,
            transform,
            ControlledBy::single(*player_id_of_spawner),
        );
        let unit_ent = unit.clone().spawn_entity(&mut commands);
        commands.entity(unit_ent).insert(DespawnOnPlayerDisconnect {
            player_id: *player_id_of_spawner,
        }).insert(HasInventory {
            inventory_id: inventory.id,
        });

        let event = EventToClient::SpawnUnit2(unit);
        info!("Notifying clients of new unit: {:?}", event);
        for endpoint in &clients {
            info!("Sending spawn event to endpoint: {:?}", endpoint.0);
            send_outgoing_event_next_tick(&sr, endpoint.0, &event);
        }

        // Now, we send the user control event to this client
        send_outgoing_event_next_tick(
            &sr,
            spawn_ev.endpoint,
            &EventToClient::BeginThirdpersonControllingUnit(BeginThirdpersonControllingUnit {
                player_id: *player_id_of_spawner,
                unit: Some(ent_id),
            }),
        );

        send_outgoing_event_next_tick(
            &sr,
            spawn_ev.endpoint,
            &EventToClient::NewInventory(shared::event::client::NewInventory {
                inventory,
            }),
        );
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
