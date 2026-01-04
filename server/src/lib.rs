use std::{collections::HashMap, sync::Arc, time::Duration};

use avian3d::prelude::{Gravity, LinearVelocity, Rotation};
use bevy::{app::ScheduleRunnerPlugin, platform::collections::HashSet, prelude::*};
use rand::Rng;
use shared::{
    event::{
        client::{
            DespawnUnit2, HeartbeatChallenge, HeartbeatResponse, PlayerDisconnected, SpawnUnit2,
            UpdateUnit2, WorldData2,
        },
        server::{ChangeMovement, Heartbeat, HeartbeatChallengeResponse, IWantToDisconnect},
        NetEntId, PlayerId, UDPacketEvent,
    },
    net_components::{
        ents::{PlayerCamera, SendNetworkTranformUpdates},
        make_ball,
        ours::{ControlledBy, DespawnOnPlayerDisconnect, PlayerColor, PlayerName},
        ToNetComponent,
    },
    netlib::{
        EndpointGeneral, EventToClient, EventToServer, NetworkConnectionTarget,
        ServerNetworkingResources, Tick,
    },
    physics::terrain::TerrainParams,
    Config, ConfigPlugin, CurrentTick, PlayerPing, PlayerPingAtomic, PlayerPingInteger,
};

/// How often to run the system
const HEARTBEAT_MILLIS: u64 = 200;
/// How long until disconnect
const HEARTBEAT_TIMEOUT: u64 = 2000;
/// How long do you have to connect, as a multipler of the heartbeart timeout.
/// If the timeout is 1000 ms, then `5` would mean you have `5000ms` to connect.
const HEARTBEAT_CONNECTION_GRACE_PERIOD: u64 = 15;

#[derive(States, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum ServerState {
    #[default]
    NotReady,
    Starting,
    Running,
}

use dashmap::DashMap;

#[derive(Resource, Default)]
struct HeartbeatList {
    heartbeats: DashMap<PlayerId, Arc<PlayerPingAtomic>>,
    pings: DashMap<PlayerId, PlayerPing<PlayerPingAtomic>>,
}

#[derive(Resource, Default)]
struct EndpointToPlayerId {
    map: DashMap<EndpointGeneral, PlayerId>,
}

#[derive(Debug, Component)]
struct PlayerEndpoint(EndpointGeneral);

//#[derive(Resource, Debug)]
//struct ServerSettings {
//tick_rate: u16,
//cheats: bool,
//}

//pub mod chat;
//pub mod game_manager;
pub mod animations;
pub mod axum;
pub mod projectile;
pub mod spawns;
pub mod terrain;
pub mod websocket;

#[derive(Resource, Clone)]
pub struct TokioRuntimeResource(pub Arc<tokio::runtime::Runtime>);
impl std::ops::Deref for TokioRuntimeResource {
    type Target = tokio::runtime::Runtime;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn main_multiplayer_server(tokio_runtime: Arc<tokio::runtime::Runtime>) {
    do_app(|app| {
        app.add_systems(Startup, add_network_connection_info_from_config);
        app.insert_resource(TokioRuntimeResource(tokio_runtime));
        app.add_plugins(axum::AxumServerPlugin);
    });
}

pub fn call_from_client_for_singleplayer(network_target: NetworkConnectionTarget) {
    info!(
        "Starting singleplayer server connecting to {:?}",
        network_target
    );
    do_app(|app| {
        app.insert_resource(network_target);
    });
}

fn do_app(f: impl FnOnce(&mut App)) {
    info!("Main Start");
    let mut app = App::new();

    app.insert_resource(EndpointToPlayerId::default())
        .insert_resource(HeartbeatList::default())
        .add_message::<PlayerDisconnected>()
        .add_message::<DespawnUnit2>()
        .add_plugins(DefaultPlugins)
        .add_plugins((
            ScheduleRunnerPlugin::run_loop(Duration::from_millis(1)),
            avian3d::PhysicsPlugins::default(),
        ))
        .insert_resource(Gravity(Vec3::new(0.0, -9.81, 0.0)))
        .add_plugins((
            ConfigPlugin,
            //chat::ChatPlugin,
            //game_manager::GamePlugin,
            spawns::SpawnPlugin,
            shared::physics::water::SharedWaterPlugin,
            terrain::TerrainPlugin,
            animations::AnimationPluginServer,
            shared::TickPlugin,
            shared::event::server::NetworkEventPlugin,
            shared::character_controller::CharacterControllerPlugin,
            websocket::WebsocketPlugin,
            projectile::ProjectilePlugin,
            //StatusPlugin,
        ))
        .init_state::<ServerState>()
        .add_systems(
            Startup,
            (|mut state: ResMut<NextState<ServerState>>| state.set(ServerState::Starting),),
        )
        .add_systems(
            OnEnter(ServerState::Starting),
            (
                shared::netlib::setup_incoming_server::<EventToServer, EventToClient>,
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
                on_receive_ping_challenge,
                on_player_scoreboard_request,
                on_unit_despawn,
                on_disconnect_packet,
                shared::event::server::drain_incoming_events,
                on_movement,
            )
                .run_if(in_state(ServerState::Running)),
        )
        // Stuff to be calculated and sent out at the end of each tick
        .add_systems(
            FixedUpdate,
            (broadcast_movement_updates).run_if(in_state(ServerState::Running)),
        )
        .add_systems(
            FixedPostUpdate,
            (
                shared::increment_ticks,
                shared::netlib::flush_outgoing_events_udp::<EventToServer, EventToClient>,
                add_tick_just_happened_packet,
            )
                .chain()
                .run_if(in_state(ServerState::Running)),
        )
        .add_systems(
            Update,
            send_ping_challenge.run_if(bevy::time::common_conditions::on_timer(
                Duration::from_millis(500),
            )),
        )
        .add_systems(
            Update,
            check_heartbeats.run_if(bevy::time::common_conditions::on_timer(
                Duration::from_millis(200),
            )),
        );

    f(&mut app);

    app.run();
}

fn add_tick_just_happened_packet(
    current_tick: Res<CurrentTick>,
    sr: Res<ServerNetworkingResources>,
    clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
) {
    for net_client in &clients {
        let event = EventToClient::TickHappened(shared::event::client::TickHappened {
            tick: current_tick.0,
        });
        sr.send_outgoing_event_next_tick(net_client.0, &event);
    }
}

fn add_network_connection_info_from_config(config: Res<Config>, mut commands: Commands) {
    commands.insert_resource(NetworkConnectionTarget {
        ip: config.host_ip.as_ref().unwrap_or(&config.ip).clone(),
        port: config.port,
    });
}

/// This component is added to each of the meta entities representing a connected player
#[derive(Component)]
pub struct ConnectedPlayer;

#[derive(Component)]
pub struct DisconnectedPlayer {
    pub disconnect_tick: Tick,
}

#[allow(clippy::too_many_arguments)]
fn on_player_connect(
    mut new_players: UDPacketEvent<shared::event::server::ConnectRequest>,
    // We need the world here so we can do dynamic queries for all existing units with NetEntId
    world: &World,
    mut commands: Commands,
) {
    let sr = world.resource::<ServerNetworkingResources>().clone();
    let terrain = world.resource::<TerrainParams>().clone();
    let _config = world.resource::<Config>().clone();
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

        let new_player_id = PlayerId::random();

        // Spawn player entity as ConnectedPlayer
        // SPAWN A
        commands.spawn((
            PlayerName { name: name.clone() },
            player_color.clone(),
            new_player_id,
            PlayerEndpoint(player.endpoint),
            ConnectedPlayer,
        ));

        // This is the unit to represent the player themselves
        // SPAWN B
        let spawn_camera_unit = SpawnUnit2 {
            net_ent_id: NetEntId::random(),
            components: vec![
                PlayerCamera.to_net_component(),
                spawn_location.to_net_component(),
                PlayerName { name: name.clone() }.to_net_component(),
                player_color.clone().to_net_component(),
                ControlledBy::single(new_player_id).to_net_component(),
                SendNetworkTranformUpdates.to_net_component(),
            ],
        };

        // Mark the camera to despawn when the player disconnects (server-side only)
        let ent = spawn_camera_unit.clone().spawn_entity(&mut commands);
        commands.entity(ent).insert(DespawnOnPlayerDisconnect {
            player_id: new_player_id,
        });

        let mut unit_list_to_new_client = vec![];

        let mut client_query = world.try_query_filtered::<(&PlayerEndpoint, &PlayerId, &PlayerName, &PlayerColor), With<ConnectedPlayer>>();
        if let Some(client_query_thing) = &mut client_query {
            for (c_net_client, c_player_id, c_name, c_color) in client_query_thing.iter(world) {
                // Send each existing player's info to the new client
                // SPAWN A
                unit_list_to_new_client.push(SpawnUnit2 {
                    net_ent_id: NetEntId::none(),
                    components: vec![
                        c_name.clone().to_net_component(),
                        c_color.clone().to_net_component(),
                        c_player_id.to_net_component(),
                        //ConnectedPlayer.to_net_component(),
                    ],
                });

                // Tell all connected clients about your new player and camera
                sr.send_outgoing_event_next_tick_batch(
                    c_net_client.0,
                    &[
                        // their camera
                        // SPAWN B
                        // TODO make the player actually move to their spawn location
                        EventToClient::SpawnUnit2(spawn_camera_unit.clone()),
                        // their player unit
                        // SPAWN A
                        EventToClient::SpawnUnit2(SpawnUnit2 {
                            net_ent_id: NetEntId::none(),
                            components: vec![
                                PlayerName { name: name.clone() }.to_net_component(),
                                player_color.clone().to_net_component(),
                                new_player_id.to_net_component(),
                                //ConnectedPlayer.to_net_component(),
                            ],
                        }),
                    ],
                );
            }
        }

        let mut large_unit_list_to_send: Vec<SpawnUnit2> = vec![];

        let units_to_spawn =
            world.try_query_filtered::<(Entity, &NetEntId), Without<ConnectedPlayer>>();

        if let Some(mut units_spawns_thing) = units_to_spawn {
            for (unit_ent, unit_net_ent_id) in units_spawns_thing.iter(world) {
                let component_info = world.inspect_entity(unit_ent).unwrap();
                info!(
                    "Preparing to send existing unit {:?} to new player",
                    unit_net_ent_id
                );
                let ciids = component_info.map(|ci| ci.id()).collect::<HashSet<_>>();
                let Ok(ents_res) = world.entity(unit_ent).get_by_id(&ciids) else {
                    error!("Failed to get components for entity {:?}", unit_ent);
                    continue;
                };

                let mut spawn_unit = SpawnUnit2 {
                    net_ent_id: *unit_net_ent_id,
                    components: vec![],
                };

                for (component_id, component_ptr) in ents_res.iter() {
                    let type_id = world
                        .components()
                        .get_info(*component_id)
                        .unwrap()
                        .type_id()
                        .unwrap();

                    // SAFETY: Trust that bevy gives us a valid type id and pointer from `get_by_id`
                    if let Some(net_comp) = unsafe {
                        shared::net_components::NetComponent::from_type_id_ptr(
                            type_id,
                            *component_ptr,
                        )
                    } {
                        info!("Component to send: {:?}", net_comp);
                        spawn_unit.components.push(net_comp);
                    }
                }
                large_unit_list_to_send.push(spawn_unit);
            }
        }

        // Each time we miss a heartbeat, we increment the Atomic counter.
        // So, we initially set this to negative number to give extra time for the initial
        // connection.
        let hb_grace_period =
            (HEARTBEAT_CONNECTION_GRACE_PERIOD - 1) * (HEARTBEAT_TIMEOUT / HEARTBEAT_MILLIS);

        let heartbeat_mapping = world.resource::<HeartbeatList>();
        heartbeat_mapping.heartbeats.insert(
            new_player_id,
            Arc::new(PlayerPingAtomic::new(
                -(hb_grace_period as PlayerPingInteger),
            )),
        );
        heartbeat_mapping.pings.insert(
            new_player_id,
            PlayerPing {
                server_challenged_ping_microsec: PlayerPingAtomic::new(-1),
                client_reported_ping_microsec: PlayerPingAtomic::new(-1),
            },
        );

        world
            .resource::<EndpointToPlayerId>()
            .map
            .insert(player.endpoint, new_player_id);

        // Finally, tell the client all this info.
        let world_data = WorldData2 {
            your_player_id: new_player_id,
            your_camera_unit_id: spawn_camera_unit.net_ent_id,
            terrain_params: terrain.clone(),
            units: unit_list_to_new_client,
        };

        // send initial world data
        info!(
            who = ?player.endpoint,
            "Player connected - sending {} existing units",
            world_data.units.len()
        );
        let event = EventToClient::WorldData2(world_data);
        sr.send_outgoing_event_next_tick(player.endpoint, &event);

        // send remaining world data in batches
        let events = large_unit_list_to_send
            .iter()
            .map(|u| EventToClient::SpawnUnit2(u.clone()))
            .collect::<Vec<_>>();

        sr.send_outgoing_event_next_tick_batch(player.endpoint, &events);
    }
}

fn check_heartbeats(
    heartbeat_mapping: Res<HeartbeatList>,
    mut on_disconnect: MessageWriter<PlayerDisconnected>,
) {
    use rayon::prelude::*;
    let dcs = heartbeat_mapping.heartbeats.par_iter().filter_map(|entry| {
        let player_id = entry.key();
        let beats_missed = entry.value();

        let beats = beats_missed.fetch_add(1, std::sync::atomic::Ordering::Acquire);
        trace!(?player_id, ?beats, "hb");
        if beats >= (HEARTBEAT_TIMEOUT / HEARTBEAT_MILLIS) as PlayerPingInteger {
            warn!("Missed {beats} beats, disconnecting {player_id:?}");
            return Some(PlayerDisconnected {
                id: *player_id,
                reason: "Player timeout".to_string(),
            });
        }
        None
    });

    for dc in dcs.collect::<Vec<_>>() {
        on_disconnect.write(dc);
    }
}

fn on_disconnect_packet(
    mut on_disconnect: MessageWriter<PlayerDisconnected>,
    mut disconnect_packets: UDPacketEvent<IWantToDisconnect>,
    endpoint_mapping: Res<EndpointToPlayerId>,
) {
    for dp in disconnect_packets.read() {
        if let Some(player_id) = endpoint_mapping.map.get(&dp.endpoint) {
            on_disconnect.write(PlayerDisconnected {
                id: *player_id,
                reason: "Client quit".to_string(),
            });
        }
    }
}

fn send_ping_challenge(
    clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
    time: Res<Time>,
    sr: Res<ServerNetworkingResources>,
) {
    let event = EventToClient::HeartbeatChallenge(HeartbeatChallenge {
        server_time: time.elapsed_secs_f64(),
    });
    for net_client in &clients {
        sr.send_outgoing_event_now(net_client.0, &event);
    }
}

fn on_receive_ping_challenge(
    mut pd: UDPacketEvent<HeartbeatChallengeResponse>,
    time: Res<Time>,
    //tick: Res<CurrentTick>,
    heartbeat_mapping: Res<HeartbeatList>,
    endpoint_mapping: Res<EndpointToPlayerId>,
) {
    for hb in pd.read() {
        if let Some(player_id) = endpoint_mapping.map.get(&hb.endpoint) {
            let ping = time.elapsed_secs_f64() - hb.event.server_time;
            let ping = ping / 2.0;
            let ping = (ping * 1_000_000.0) as PlayerPingInteger; // in us
            if let Some(player_ping) = heartbeat_mapping.pings.get(&*player_id) {
                player_ping
                    .server_challenged_ping_microsec
                    .store(ping, std::sync::atomic::Ordering::Release);

                player_ping.client_reported_ping_microsec.store(
                    hb.event.local_latency_microsecs as PlayerPingInteger,
                    std::sync::atomic::Ordering::Release,
                );

                trace!(?player_id, ?player_ping, "ping updated");
            } else {
                error!(?player_id, "no ping entry found");
            }
        }
    }
}

fn on_player_disconnect(
    mut pd: MessageReader<PlayerDisconnected>,

    clients: Query<(Entity, &PlayerEndpoint, &PlayerId), With<ConnectedPlayer>>,
    clients_owned_items: Query<(&NetEntId, &DespawnOnPlayerDisconnect)>,

    mut despawn_unit: MessageWriter<DespawnUnit2>,
    mut commands: Commands,
    heartbeat_mapping: Res<HeartbeatList>,
    tick: Res<CurrentTick>,
    sr: Res<ServerNetworkingResources>,
) {
    for player in pd.read() {
        heartbeat_mapping.heartbeats.remove(&player.id);
        heartbeat_mapping.pings.remove(&player.id);

        let events = vec![EventToClient::PlayerDisconnected(player.clone())];

        for (owned_ent_id, despawn_tag) in &clients_owned_items {
            if despawn_tag.player_id == player.id {
                despawn_unit.write(DespawnUnit2 {
                    net_ent_id: *owned_ent_id,
                });
            }
        }

        for (c_ent, net_client, player_id) in &clients {
            sr.send_outgoing_event_next_tick_batch(net_client.0, &events);
            if player_id == &player.id {
                commands
                    .entity(c_ent)
                    .remove::<ConnectedPlayer>()
                    .insert(DisconnectedPlayer {
                        disconnect_tick: tick.0,
                    });
            }
        }
    }
}

fn on_unit_despawn(
    mut pd: MessageReader<DespawnUnit2>,
    clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
    units: Query<(Entity, &NetEntId)>,
    mut commands: Commands,
    sr: Res<ServerNetworkingResources>,
) {
    let mut events = vec![];
    for despawn in pd.read() {
        'unit: for (unit_ent, unit_net_ent_id) in &units {
            if unit_net_ent_id == &despawn.net_ent_id {
                commands.entity(unit_ent).despawn();
                break 'unit;
            }
        }

        trace!("Despawning unit {:?}", despawn.net_ent_id);

        // Now tell all clients to also despawn
        let event = EventToClient::DespawnUnit2(DespawnUnit2 {
            net_ent_id: despawn.net_ent_id,
        });
        events.push(event);
    }

    if events.is_empty() {
        return;
    }

    for net_client in &clients {
        sr.send_outgoing_event_next_tick_batch(net_client.0, &events);
    }
}

fn on_player_heartbeat(
    mut pd: UDPacketEvent<Heartbeat>,
    tick: Res<CurrentTick>,
    time: Res<Time>,
    heartbeat_mapping: Res<HeartbeatList>,
    endpoint_mapping: Res<EndpointToPlayerId>,
    sr: Res<ServerNetworkingResources>,
) {
    for hb in pd.read() {
        // TODO tryblocks?
        if let Some(id) = endpoint_mapping.map.get(&hb.endpoint) {
            if let Some(heartbeat_pointer) = heartbeat_mapping.heartbeats.get(&*id) {
                heartbeat_pointer.fetch_min(0, std::sync::atomic::Ordering::Release);
                let event = EventToClient::HeartbeatResponse(HeartbeatResponse {
                    client_started_time: hb.event.client_started_time,
                    server_time: time.elapsed_secs_f64(),
                    server_tick: tick.0,
                });
                sr.send_outgoing_event_now(hb.endpoint, &event);
            }
        }
    }
}

fn on_player_scoreboard_request(
    mut pd: UDPacketEvent<shared::event::server::RequestScoreboard>,
    plys: Query<(&PlayerId, &PlayerName), With<ConnectedPlayer>>,
    heartbeat_mapping: Res<HeartbeatList>,
    sr: Res<ServerNetworkingResources>,
) {
    let mut scoreboard_data = shared::event::client::RequestScoreboardResponse {
        player_names: HashMap::new(),
        player_pings: HashMap::new(),
    };
    for (ply_id, ply_name) in &plys {
        if let Some(ping) = heartbeat_mapping.pings.get(ply_id) {
            let ping = ping.to_integer();
            scoreboard_data
                .player_names
                .insert(*ply_id, ply_name.name.clone());
            scoreboard_data.player_pings.insert(*ply_id, ping);
        }
    }
    for req in pd.read() {
        let event = EventToClient::RequestScoreboardResponse(scoreboard_data.clone());
        sr.send_outgoing_event_now(req.endpoint, &event);
    }
}

fn on_movement(
    mut pd: UDPacketEvent<ChangeMovement>,
    mut ent_to_move: Query<
        (
            &NetEntId,
            &mut Transform,
            Option<&mut LinearVelocity>,
            Option<&mut Rotation>,
        ),
        With<SendNetworkTranformUpdates>,
    >,
) {
    'event: for movement in pd.read() {
        // The camera NetEntId is directly in the movement event
        let camera_net_id = movement.event.net_ent_id;

        // Find and update the camera entity
        for (cam_net_id, mut cam_transform, maybe_lv, maybe_rot) in &mut ent_to_move {
            if cam_net_id == &camera_net_id {
                // Update the camera's transform on the server
                *cam_transform = movement.event.transform;

                // Update linear velocity if provided
                if let Some(mut lv) = maybe_lv {
                    if let Some(new_lv) = movement.event.velocity {
                        *lv = new_lv;
                    }
                }

                // Update rotation if provided
                if let Some(mut rot) = maybe_rot {
                    if let Some(new_rot) = movement.event.rotation {
                        *rot = new_rot;
                    }
                }
                continue 'event;
            }
        }

        warn!(
            "Received movement update for unknown entity {:?}",
            camera_net_id
        );
    }
}

// TODO make this more efficient by batching updates per client and only sending changed components
// for physics if we think the client needs them
fn broadcast_movement_updates(
    clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
    sr: Res<ServerNetworkingResources>,
    changed_transforms: Query<
        (
            &NetEntId,
            &Transform,
            Option<&LinearVelocity>,
            Option<&Rotation>,
        ),
        (With<SendNetworkTranformUpdates>, Changed<Transform>),
    >,
    //current_tick: Res<CurrentTick>,
) {
    let mut events_to_send = vec![];
    for (cam_net_id, cam_transform, cam_lv, cam_rot) in &changed_transforms {
        let mut components = vec![cam_transform.to_net_component()];
        if let Some(lv) = cam_lv {
            components.push(lv.to_net_component());
        }
        if let Some(rot) = cam_rot {
            components.push(rot.to_net_component());
        }

        let event = EventToClient::UpdateUnit2(UpdateUnit2 {
            net_ent_id: *cam_net_id,
            changed_components: components,
            new_component: vec![],
            removed_components: vec![],
        });

        events_to_send.push(event);
    }

    if !events_to_send.is_empty() {
        trace!(
            "Broadcasting {} movement updates to clients",
            events_to_send.len()
        );
        for c_net_client in &clients {
            sr.send_outgoing_event_next_tick_batch(c_net_client.0, &events_to_send);
        }
    }
}
