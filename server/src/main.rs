use std::{
    collections::HashMap,
    sync::{atomic::AtomicI16, Arc},
    time::Duration,
};

use bevy::{log::LogPlugin, prelude::*};
use message_io::network::Endpoint;
use rand::Rng;
use shared::{
    event::{
        client::{PlayerDisconnected, SpawnUnit2, UpdateUnit2, WorldData2},
        server::{ChangeMovement, Heartbeat},
        MyNetEntParentId, NetEntId, ERFE,
    },
    net_components::{ents::PlayerCamera, make_ball, ours::PlayerName, ToNetComponent},
    netlib::{
        send_event_to_server, send_event_to_server_batch, EventToClient, EventToServer,
        NetworkConnectionTarget, ServerNetworkingResources,
    },
    Config, ConfigPlugin,
};

/// How often to run the system
const HEARTBEAT_MILLIS: u64 = 200;
/// How long until disconnect
const HEARTBEAT_TIMEOUT: u64 = 1000;
/// How long do you have to connect, as a multipler of the heartbeart timeout.
/// If the timeout is 1000 ms, then `5` would mean you have `5000ms` to connect.
const HEARTBEAT_CONNECTION_GRACE_PERIOD: u64 = 5;

#[derive(States, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum ServerState {
    #[default]
    NotReady,
    Starting,
    Running,
}

#[derive(Resource, Default)]
struct HeartbeatList {
    heartbeats: HashMap<NetEntId, Arc<AtomicI16>>,
}

#[derive(Resource, Default)]
struct EndpointToNetId {
    map: HashMap<Endpoint, NetEntId>,
}

#[derive(Debug, Component)]
struct PlayerEndpoint(Endpoint);

//pub mod chat;
//pub mod game_manager;
pub mod spawns;

fn main() {
    info!("Main Start");
    let mut app = App::new();

    shared::event::server::register_events(&mut app);
    app.insert_resource(EndpointToNetId::default())
        .insert_resource(HeartbeatList::default())
        .add_message::<PlayerDisconnected>()
        .add_plugins(DefaultPlugins)
        .add_plugins((avian3d::PhysicsPlugins::default(),))
        .add_plugins((
            ConfigPlugin,
            //chat::ChatPlugin,
            //game_manager::GamePlugin,
            spawns::SpawnPlugin,
            //StatusPlugin,
        ))
        .init_state::<ServerState>()
        .add_systems(
            Startup,
            (
                add_network_connection_info_from_config,
                |mut state: ResMut<NextState<ServerState>>| state.set(ServerState::Starting),
            ),
        )
        .add_systems(
            OnEnter(ServerState::Starting),
            (
                shared::netlib::setup_server::<EventToServer>,
                |mut state: ResMut<NextState<ServerState>>| state.set(ServerState::Running),
            ),
        )
        .add_systems(
            Update,
            (
                on_player_disconnect,
                on_player_connect,
                on_player_heartbeat,
                shared::event::server::drain_events,
                on_movement,
            )
                .run_if(in_state(ServerState::Running)),
        )
        .add_systems(
            Update,
            check_heartbeats.run_if(bevy::time::common_conditions::on_timer(
                Duration::from_millis(200),
            )),
        );

    app.run();
}

fn add_network_connection_info_from_config(config: Res<Config>, mut commands: Commands) {
    commands.insert_resource(NetworkConnectionTarget {
        ip: config.ip.clone(),
        port: config.port,
    });
}

#[derive(Component)]
pub struct ConnectedPlayer;

#[derive(Component)]
pub struct HasColor(pub Color);

fn on_player_connect(
    mut new_players: ERFE<shared::event::server::ConnectRequest>,
    mut heartbeat_mapping: ResMut<HeartbeatList>,
    mut endpoint_to_net_id: ResMut<EndpointToNetId>,
    clients: Query<(&PlayerEndpoint, &NetEntId, &PlayerName), With<ConnectedPlayer>>,
    cameras: Query<(&NetEntId, &MyNetEntParentId, &Transform, &PlayerName), With<PlayerCamera>>,
    balls: Query<(&Transform, &NetEntId, &HasColor), With<shared::net_components::ents::Ball>>,
    sr: Res<ServerNetworkingResources>,
    _config: Res<Config>,
    mut commands: Commands,
) {
    for player in new_players.read() {
        info!("Got packet");
        // Generate their name
        let name = player
            .event
            .name
            .clone()
            .unwrap_or_else(|| format!("Player #{}", rand::rng().random_range(1..10000)));

        let spawn_location = player.event.my_location;

        let new_player_ent_id = NetEntId::random();

        let spawn_camera_unit = SpawnUnit2 {
            net_ent_id: NetEntId::random(),
            components: vec![
                PlayerName { name: name.clone() }.to_net_component(),
                spawn_location.to_net_component(),
                PlayerCamera.to_net_component(),
                MyNetEntParentId::new(new_player_ent_id).to_net_component(),
            ],
        };

        let mut unit_list_to_new_client = vec![];

        // Add all existing cameras from other players to unit list as spawnunit2s
        for (c_net_ent, _c_parent_id, c_tfm, c_name) in &cameras {
            unit_list_to_new_client.push(SpawnUnit2 {
                net_ent_id: *c_net_ent,
                components: vec![
                    PlayerCamera.to_net_component(),
                    (*c_tfm).to_net_component(),
                    c_name.clone().to_net_component(),
                ],
            });
        }

        // Add all other players to unit list too
        for (_c_net_client, c_net_ent, c_name) in &clients {
            unit_list_to_new_client.push(SpawnUnit2 {
                net_ent_id: *c_net_ent,
                components: vec![c_name.clone().to_net_component()],
            });
        }

        // Tell all other clients about your new player
        for (c_net_client, _c_net_ent, c_name) in &clients {
            send_event_to_server_batch(
                &sr.handler,
                c_net_client.0,
                &[
                    // their camera
                    EventToClient::SpawnUnit2(spawn_camera_unit.clone()),
                    // their player unit
                    EventToClient::SpawnUnit2(SpawnUnit2 {
                        net_ent_id: new_player_ent_id.clone(),
                        components: vec![c_name.clone().to_net_component()],
                    }),
                ],
            );
        }

        // gather the balls in the unit list
        for (&transform, &ent_id, has_color) in &balls {
            unit_list_to_new_client.push(make_ball(ent_id, transform, has_color.0));
        }

        // Add the connected player ent here
        commands.spawn((
            PlayerName { name },
            new_player_ent_id.clone(),
            PlayerEndpoint(player.endpoint),
            ConnectedPlayer,
            // Used as a target for some AI
        ));

        // Add the camera entity here
        spawn_camera_unit.clone().spawn_entity_srv(&mut commands);

        // Each time we miss a heartbeat, we increment the Atomic counter.
        // So, we initially set this to negative number to give extra time for the initial
        // connection.
        let hb_grace_period =
            (HEARTBEAT_CONNECTION_GRACE_PERIOD - 1) * (HEARTBEAT_TIMEOUT / HEARTBEAT_MILLIS);

        heartbeat_mapping.heartbeats.insert(
            new_player_ent_id.clone(),
            Arc::new(AtomicI16::new(-(hb_grace_period as i16))),
        );

        endpoint_to_net_id
            .map
            .insert(player.endpoint, new_player_ent_id.clone());

        // Finally, tell the client all this info.
        let event = EventToClient::WorldData2(WorldData2 {
            your_unit_id: new_player_ent_id.clone(),
            your_camera_unit_id: spawn_camera_unit.net_ent_id.clone(),
            units: unit_list_to_new_client,
        });
        send_event_to_server(&sr.handler, player.endpoint, &event);
    }
}

fn check_heartbeats(
    heartbeat_mapping: Res<HeartbeatList>,
    mut on_disconnect: MessageWriter<PlayerDisconnected>,
) {
    for (ent_id, beats_missed) in &heartbeat_mapping.heartbeats {
        let beats = beats_missed.fetch_add(1, std::sync::atomic::Ordering::Acquire);
        trace!(?ent_id, ?beats, "hb");
        if beats >= (HEARTBEAT_TIMEOUT / HEARTBEAT_MILLIS) as i16 {
            warn!("Missed {beats} beats, disconnecting {ent_id:?}");
            on_disconnect.write(PlayerDisconnected { id: *ent_id });
        }
    }
}

fn on_player_disconnect(
    mut pd: MessageReader<PlayerDisconnected>,
    clients: Query<(Entity, &PlayerEndpoint, &NetEntId), With<PlayerName>>,
    clients_owned_items: Query<(Entity, &NetEntId, &MyNetEntParentId)>,
    mut commands: Commands,
    mut heartbeat_mapping: ResMut<HeartbeatList>,
    sr: Res<ServerNetworkingResources>,
) {
    for player in pd.read() {
        heartbeat_mapping.heartbeats.remove(&player.id);

        let mut events = vec![];
        events.push(EventToClient::PlayerDisconnected(PlayerDisconnected {
            id: player.id,
        }));

        for (owned_ent, net_ent_id, owner_id) in &clients_owned_items {
            if owner_id.0 == player.id.0 {
                events.push(EventToClient::DespawnUnit2(
                    shared::event::client::DespawnUnit2 {
                        net_ent_id: *net_ent_id,
                    },
                ));

                commands.entity(owned_ent).despawn();
            }
        }

        for (c_ent, net_client, net_ent_id) in &clients {
            send_event_to_server_batch(&sr.handler, net_client.0, &events);
            if net_ent_id == &player.id {
                commands.entity(c_ent).despawn();
            }
        }
    }
}

fn on_player_heartbeat(
    mut pd: ERFE<Heartbeat>,
    heartbeat_mapping: Res<HeartbeatList>,
    endpoint_mapping: Res<EndpointToNetId>,
) {
    for hb in pd.read() {
        // TODO tryblocks?
        if let Some(id) = endpoint_mapping.map.get(&hb.endpoint) {
            if let Some(hb) = heartbeat_mapping.heartbeats.get(id) {
                hb.fetch_min(0, std::sync::atomic::Ordering::Release);
            }
        }
    }
}

fn on_movement(
    mut pd: ERFE<ChangeMovement>,
    endpoint_mapping: Res<EndpointToNetId>,
    clients: Query<(&PlayerEndpoint, &NetEntId), With<ConnectedPlayer>>,
    cameras: Query<(&NetEntId, &MyNetEntParentId, &mut Transform), With<PlayerCamera>>,
    sr: Res<ServerNetworkingResources>,
) {
    for movement in pd.read() {
        if let Some(moved_net_id) = endpoint_mapping.map.get(&movement.endpoint) {
            let event = EventToClient::UpdateUnit2(UpdateUnit2 {
                net_ent_id: *moved_net_id,
                components: vec![movement.event.transform.to_net_component()],
            });

            //todo!();

            //for (c_net_client, c_net_ent) in &mut clients {
            //if moved_net_id == c_net_ent {
            ////info!(?event);
            //// If this person moved, update their transform serverside
            //match movement.event {
            //ChangeMovement::SetTransform(new_tfm) => *c_tfm = new_tfm,
            //ChangeMovement::Move2d(new_intent) => {
            //*intent = MovementIntention(new_intent)
            //}
            //_ => {}
            //}
            //} else {
            //// Else, just rebroadcast the packet to everyone else
            //send_event_to_server(&sr.handler, c_net_client.0, &event);
            //}
            //}
        }
    }
}
