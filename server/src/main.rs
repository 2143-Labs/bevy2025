use std::{
    collections::HashMap,
    sync::{atomic::AtomicI16, Arc},
    time::Duration,
};

use avian3d::prelude::LinearVelocity;
use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use message_io::network::Endpoint;
use rand::Rng;
use shared::{
    event::{
        client::{DespawnUnit2, PlayerDisconnected, SpawnUnit2, UpdateUnit2, WorldData2},
        server::{ChangeMovement, Heartbeat},
        NetEntId, PlayerId, UDPacketEvent,
    },
    net_components::{
        ents::{PlayerCamera, SendNetworkTranformUpdates},
        foreign::ComponentColor,
        make_ball, make_man,
        ours::{ControlledBy, DespawnOnPlayerDisconnect, PlayerColor, PlayerName},
        ToNetComponent,
    },
    netlib::{
        send_outgoing_event_next_tick, send_outgoing_event_next_tick_batch, EventToClient,
        EventToServer, NetworkConnectionTarget, ServerNetworkingResources, Tick,
    },
    physics::terrain::TerrainParams,
    Config, ConfigPlugin, CurrentTick,
};

/// How often to run the system
const HEARTBEAT_MILLIS: u64 = 200;
/// How long until disconnect
const HEARTBEAT_TIMEOUT: u64 = 2000;
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
    heartbeats: HashMap<PlayerId, Arc<AtomicI16>>,
}

#[derive(Resource, Default)]
struct EndpointToPlayerId {
    map: HashMap<Endpoint, PlayerId>,
}

#[derive(Debug, Component)]
struct PlayerEndpoint(Endpoint);

//#[derive(Resource, Debug)]
//struct ServerSettings {
//tick_rate: u16,
//cheats: bool,
//}

//pub mod chat;
//pub mod game_manager;
pub mod spawns;
pub mod terrain;

fn main() {
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
        .add_plugins((
            ConfigPlugin,
            //chat::ChatPlugin,
            //game_manager::GamePlugin,
            spawns::SpawnPlugin,
            shared::physics::water::SharedWaterPlugin,
            terrain::TerrainPlugin,
            shared::TickPlugin,
            shared::event::server::NetworkEventPlugin,
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
                on_unit_despawn,
                shared::event::server::drain_incoming_events,
                on_movement,
            )
                .run_if(in_state(ServerState::Running)),
        )
        .add_systems(
            FixedUpdate,
            (broadcast_movement_updates, shared::increment_ticks)
                .run_if(in_state(ServerState::Running)),
        )
        .add_systems(
            FixedPostUpdate,
            (shared::netlib::flush_outgoing_events::<EventToServer, EventToClient>)
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
    mut heartbeat_mapping: ResMut<HeartbeatList>,
    mut endpoint_to_player_id: ResMut<EndpointToPlayerId>,

    clients: Query<(&PlayerEndpoint, &PlayerId, &PlayerName, &PlayerColor), With<ConnectedPlayer>>,
    cameras: Query<
        (
            &NetEntId,
            &ControlledBy,
            &Transform,
            &PlayerName,
            &PlayerColor,
        ),
        With<PlayerCamera>,
    >,
    balls: Query<
        (&Transform, &ControlledBy, &NetEntId, &ComponentColor),
        With<shared::net_components::ents::Ball>,
    >,
    men: Query<(&Transform, &ControlledBy, &NetEntId), With<shared::net_components::ents::Man>>,

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

        // Next, add all cameras, clients, balls, and men to the unit list
        info!(
            "Found {} existing cameras to send to new player",
            cameras.iter().len()
        );
        for (c_net_ent, c_controlled_by, c_tfm, c_name, c_color) in &cameras {
            info!(
                "  - Camera {:?} at {:?} for player {:?}",
                c_net_ent, c_tfm.translation, c_controlled_by
            );
            // SPAWN B
            unit_list_to_new_client.push(SpawnUnit2 {
                net_ent_id: *c_net_ent,
                components: vec![
                    PlayerCamera.to_net_component(),
                    (*c_tfm).to_net_component(),
                    c_name.clone().to_net_component(),
                    c_color.clone().to_net_component(),
                    c_controlled_by.clone().to_net_component(),
                    SendNetworkTranformUpdates.to_net_component(),
                ],
            });
        }

        for (c_net_client, c_player_id, c_name, c_color) in &clients {
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
            send_outgoing_event_next_tick_batch(
                &sr,
                c_net_client.0,
                &[
                    // their camera
                    // SPAWN B
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

        let mut unit_list_to_new_client_balls_and_men = vec![];

        // gather the balls in the unit list
        for (&transform, controlled_by, &ent_id, has_color) in &balls {
            unit_list_to_new_client_balls_and_men.push(make_ball(
                ent_id,
                transform,
                has_color.0,
                controlled_by.clone(),
            ));
        }

        // gather the men in the unit list
        for (&transform, controlled_by, &ent_id) in &men {
            unit_list_to_new_client_balls_and_men.push(make_man(
                ent_id,
                transform,
                controlled_by.clone(),
            ));
        }

        // Each time we miss a heartbeat, we increment the Atomic counter.
        // So, we initially set this to negative number to give extra time for the initial
        // connection.
        let hb_grace_period =
            (HEARTBEAT_CONNECTION_GRACE_PERIOD - 1) * (HEARTBEAT_TIMEOUT / HEARTBEAT_MILLIS);

        heartbeat_mapping.heartbeats.insert(
            new_player_id,
            Arc::new(AtomicI16::new(-(hb_grace_period as i16))),
        );

        endpoint_to_player_id
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
            "Player connected - sending {} existing units",
            world_data.units.len()
        );
        let event = EventToClient::WorldData2(world_data);
        send_outgoing_event_next_tick(&sr, player.endpoint, &event);

        // send remaining world data in batches
        for ball_unit in unit_list_to_new_client_balls_and_men.chunks(100) {
            let events = ball_unit
                .iter()
                .map(|u| EventToClient::SpawnUnit2(u.clone()))
                .collect::<Vec<_>>();

            send_outgoing_event_next_tick_batch(&sr, player.endpoint, &events);
        }
    }
}

fn check_heartbeats(
    heartbeat_mapping: Res<HeartbeatList>,
    mut on_disconnect: MessageWriter<PlayerDisconnected>,
) {
    for (player_id, beats_missed) in &heartbeat_mapping.heartbeats {
        let beats = beats_missed.fetch_add(1, std::sync::atomic::Ordering::Acquire);
        trace!(?player_id, ?beats, "hb");
        if beats >= (HEARTBEAT_TIMEOUT / HEARTBEAT_MILLIS) as i16 {
            warn!("Missed {beats} beats, disconnecting {player_id:?}");
            on_disconnect.write(PlayerDisconnected { id: *player_id });
        }
    }
}

fn on_player_disconnect(
    mut pd: MessageReader<PlayerDisconnected>,

    clients: Query<(Entity, &PlayerEndpoint, &PlayerId), With<ConnectedPlayer>>,
    clients_owned_items: Query<(&NetEntId, &DespawnOnPlayerDisconnect)>,

    mut despawn_unit: MessageWriter<DespawnUnit2>,
    mut commands: Commands,
    mut heartbeat_mapping: ResMut<HeartbeatList>,
    tick: Res<CurrentTick>,
    sr: Res<ServerNetworkingResources>,
) {
    for player in pd.read() {
        heartbeat_mapping.heartbeats.remove(&player.id);

        let mut events = vec![];
        events.push(EventToClient::PlayerDisconnected(PlayerDisconnected {
            id: player.id,
        }));

        for (owned_ent_id, despawn_tag) in &clients_owned_items {
            if despawn_tag.player_id == player.id {
                despawn_unit.write(DespawnUnit2 {
                    net_ent_id: *owned_ent_id,
                });
            }
        }

        for (c_ent, net_client, player_id) in &clients {
            send_outgoing_event_next_tick_batch(&sr, net_client.0, &events);
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
        send_outgoing_event_next_tick_batch(&sr, net_client.0, &events);
    }
}

fn on_player_heartbeat(
    mut pd: UDPacketEvent<Heartbeat>,
    heartbeat_mapping: Res<HeartbeatList>,
    endpoint_mapping: Res<EndpointToPlayerId>,
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
    mut pd: UDPacketEvent<ChangeMovement>,
    mut ent_to_move: Query<(&NetEntId, &mut Transform), With<SendNetworkTranformUpdates>>,
) {
    'event: for movement in pd.read() {
        // The camera NetEntId is directly in the movement event
        let camera_net_id = movement.event.net_ent_id;

        // Find and update the camera entity
        for (cam_net_id, mut cam_transform) in &mut ent_to_move {
            if cam_net_id == &camera_net_id {
                // Update the camera's transform on the server
                *cam_transform = movement.event.transform;
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
        (&NetEntId, &Transform, Option<&LinearVelocity>),
        (With<SendNetworkTranformUpdates>, Changed<Transform>),
    >,
    //current_tick: Res<CurrentTick>,
) {
    let mut events_to_send = vec![];
    for (cam_net_id, cam_transform, cam_lv) in &changed_transforms {
        let mut components = vec![cam_transform.to_net_component()];
        if let Some(lv) = cam_lv {
            components.push(lv.to_net_component());
        }

        let event = EventToClient::UpdateUnit2(UpdateUnit2 {
            net_ent_id: *cam_net_id,
            components,
        });

        events_to_send.push(event);
    }

    if !events_to_send.is_empty() {
        trace!(
            "Broadcasting {} movement updates to clients",
            events_to_send.len()
        );
        for c_net_client in &clients {
            send_outgoing_event_next_tick_batch(&sr, c_net_client.0, &events_to_send);
        }
    }
}
