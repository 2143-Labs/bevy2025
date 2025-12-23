use std::{
    collections::HashMap,
    sync::{atomic::AtomicI16, Arc},
    time::Duration,
};

use bevy::{
    app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*, time::common_conditions::on_timer,
};
use message_io::network::Endpoint;
use rand::Rng;
use shared::{
    event::{
        client::{PlayerDisconnected, SpawnUnit2, UpdateUnit2, WorldData2},
        server::{ChangeMovement, Heartbeat},
        MyNetEntParentId, NetEntId, ERFE,
    },
    net_components::{
        ents::PlayerCamera,
        make_ball,
        ours::{PlayerColor, PlayerName},
        ToNetComponent,
    },
    netlib::{
        send_event_to_server, send_event_to_server_batch, EventToClient, EventToServer,
        NetworkConnectionTarget, ServerNetworkingResources,
    },
    physics::terrain::TerrainParams,
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
pub mod terrain;

fn main() {
    info!("Main Start");
    let mut app = App::new();

    shared::event::server::register_events(&mut app);
    app.insert_resource(EndpointToNetId::default())
        .insert_resource(HeartbeatList::default())
        .add_message::<PlayerDisconnected>()
        .add_plugins(DefaultPlugins)
        .add_plugins((
            ScheduleRunnerPlugin::run_loop(Duration::from_millis(1)),
            avian3d::PhysicsPlugins::default(),
        ))
        .add_plugins((
            ConfigPlugin,
            //chat::ChatPlugin,
            //game_manager::GamePlugin,
            spawns::SpawnPlugin,
            shared::physics::water::SharedWaterPlugin,
            terrain::TerrainPlugin,
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
                |mut state: ResMut<NextState<ServerState>>| {
                    info!("Server started, switching to running state");
                    state.set(ServerState::Running)
                },
            ),
        )
        .add_systems(
            OnEnter(ServerState::Running),
            (|| {
                info!("We are fully Running!");
            },),
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
        ip: config.host_ip.as_ref().unwrap_or(&config.ip).clone(),
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
    clients: Query<(&PlayerEndpoint, &NetEntId, &PlayerName, &PlayerColor), With<ConnectedPlayer>>,
    cameras: Query<
        (
            &NetEntId,
            &MyNetEntParentId,
            &Transform,
            &PlayerName,
            &PlayerColor,
        ),
        With<PlayerCamera>,
    >,
    balls: Query<(&Transform, &NetEntId, &HasColor), With<shared::net_components::ents::Ball>>,
    sr: Res<ServerNetworkingResources>,
    terrain: Res<TerrainParams>,
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
        let player_color = PlayerColor {
            hue: player.event.color_hue,
        };

        let new_player_ent_id = NetEntId::random();

        let spawn_camera_unit = SpawnUnit2 {
            net_ent_id: NetEntId::random(),
            components: vec![
                PlayerName { name: name.clone() }.to_net_component(),
                player_color.clone().to_net_component(),
                spawn_location.to_net_component(),
                PlayerCamera.to_net_component(),
                MyNetEntParentId::new(new_player_ent_id).to_net_component(),
            ],
        };

        // Add the connected player ent here (BEFORE querying cameras)
        commands.spawn((
            PlayerName { name: name.clone() },
            player_color.clone(),
            new_player_ent_id,
            PlayerEndpoint(player.endpoint),
            ConnectedPlayer,
        ));

        // Add the camera entity here (BEFORE querying cameras so it's available for next client)
        spawn_camera_unit.clone().spawn_entity_srv(&mut commands);

        let mut unit_list_to_new_client = vec![];

        // Add all existing cameras from other players to unit list as spawnunit2s
        info!(
            "Found {} existing cameras to send to new player",
            cameras.iter().len()
        );
        for (c_net_ent, c_parent_id, c_tfm, c_name, c_color) in &cameras {
            info!(
                "  - Camera {:?} at {:?} for player {:?}",
                c_net_ent, c_tfm.translation, c_parent_id
            );
            unit_list_to_new_client.push(SpawnUnit2 {
                net_ent_id: *c_net_ent,
                components: vec![
                    PlayerCamera.to_net_component(),
                    (*c_tfm).to_net_component(),
                    c_name.clone().to_net_component(),
                    c_color.clone().to_net_component(),
                    c_parent_id.clone().to_net_component(),
                ],
            });
        }

        // Add all other players to unit list too
        for (_c_net_client, c_net_ent, c_name, c_color) in &clients {
            unit_list_to_new_client.push(SpawnUnit2 {
                net_ent_id: *c_net_ent,
                components: vec![
                    c_name.clone().to_net_component(),
                    c_color.clone().to_net_component(),
                ],
            });
        }

        // Tell all other clients about your new player
        for (c_net_client, _c_net_ent, _c_name, _c_color) in &clients {
            send_event_to_server_batch(
                &sr.handler,
                c_net_client.0,
                &[
                    // their camera
                    EventToClient::SpawnUnit2(spawn_camera_unit.clone()),
                    // their player unit
                    EventToClient::SpawnUnit2(SpawnUnit2 {
                        net_ent_id: new_player_ent_id,
                        components: vec![
                            PlayerName { name: name.clone() }.to_net_component(),
                            player_color.clone().to_net_component(),
                        ],
                    }),
                ],
            );
        }
        let mut unit_list_to_new_client_balls = vec![];

        // gather the balls in the unit list
        for (&transform, &ent_id, has_color) in &balls {
            unit_list_to_new_client_balls.push(make_ball(ent_id, transform, has_color.0));
        }

        // Each time we miss a heartbeat, we increment the Atomic counter.
        // So, we initially set this to negative number to give extra time for the initial
        // connection.
        let hb_grace_period =
            (HEARTBEAT_CONNECTION_GRACE_PERIOD - 1) * (HEARTBEAT_TIMEOUT / HEARTBEAT_MILLIS);

        heartbeat_mapping.heartbeats.insert(
            new_player_ent_id,
            Arc::new(AtomicI16::new(-(hb_grace_period as i16))),
        );

        endpoint_to_net_id
            .map
            .insert(player.endpoint, new_player_ent_id);

        // Finally, tell the client all this info.
        let world_data = WorldData2 {
            your_unit_id: new_player_ent_id,
            your_camera_unit_id: spawn_camera_unit.net_ent_id,
            terrain_params: terrain.clone(),
            units: unit_list_to_new_client,
        };
        info!(
            "Player connected - sending {} existing units",
            world_data.units.len()
        );
        let event = EventToClient::WorldData2(world_data);
        send_event_to_server(&sr.handler, player.endpoint, &event);

        for ball_unit in unit_list_to_new_client_balls.chunks(100) {
            let events = ball_unit
                .iter()
                .map(|u| EventToClient::SpawnUnit2(u.clone()))
                .collect::<Vec<_>>();
            send_event_to_server_batch(&sr.handler, player.endpoint, &events);
        }
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
    mut cameras: Query<(&NetEntId, &MyNetEntParentId, &mut Transform), With<PlayerCamera>>,
    sr: Res<ServerNetworkingResources>,
) {
    for movement in pd.read() {
        // Get the player ID who sent this movement
        let sending_player_net_id = endpoint_mapping.map.get(&movement.endpoint);

        // The camera NetEntId is directly in the movement event
        let camera_net_id = movement.event.net_ent_id;

        // Find and update the camera entity
        let mut camera_updated = false;
        for (cam_net_id, _cam_parent_id, mut cam_transform) in &mut cameras {
            if cam_net_id == &camera_net_id {
                // Update the camera's transform on the server
                *cam_transform = movement.event.transform;
                camera_updated = true;
                break;
            }
        }

        // Broadcast the camera update to all OTHER clients
        if camera_updated {
            let event = EventToClient::UpdateUnit2(UpdateUnit2 {
                net_ent_id: camera_net_id,
                components: vec![movement.event.transform.to_net_component()],
            });

            for (c_net_client, c_net_ent) in &clients {
                // Don't send the update back to the player who sent it
                if let Some(sender_id) = sending_player_net_id {
                    if sender_id != c_net_ent {
                        send_event_to_server(&sr.handler, c_net_client.0, &event);
                    }
                } else {
                    // If we can't identify the sender, broadcast to everyone
                    send_event_to_server(&sr.handler, c_net_client.0, &event);
                }
            }
        }
    }
}
