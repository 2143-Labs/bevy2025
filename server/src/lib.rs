use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use avian3d::prelude::{Gravity, LinearVelocity, Rotation};
use bevy::{platform::collections::HashSet, prelude::*};
use rand::RngExt;
use shared::{
    Config, ConfigPlugin, CurrentTick, PlayerPing, PlayerPingAtomic, PlayerPingInteger,
    event::{
        NetEntId, PlayerId, UDPacketEvent,
        client::{
            ConnectRejected, DespawnUnit2, HeartbeatChallenge, HeartbeatResponse,
            PlayerDisconnected, SpawnUnit2, UpdateUnit2, WorldData2,
        },
        server::{ChangeMovement, Heartbeat, HeartbeatChallengeResponse, IWantToDisconnect},
    },
    net_components::{
        ToNetComponent,
        ents::{PlayerCamera, SendNetworkTranformUpdates},
        make_ball,
        ours::{ControlledBy, DespawnOnPlayerDisconnect, PlayerColor, PlayerName},
    },
    netlib::{
        EndpointGeneral, EventToClient, EventToServer, NetworkConnectionTarget,
        ServerNetworkingResources, Tick,
    },
    physics::terrain::TerrainParams,
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

//pub mod chat;
//pub mod game_manager;
pub mod animations;
pub mod axum;
pub mod coordinator;
pub mod gym_beacon;
pub mod projectile;
pub mod spawns;
pub mod terrain;
pub mod ticket;
pub mod websocket;

use ticket::{TicketVerifier, VerifiedTicket};

/// Private internal coordinator for this server (player placement, server lifecycle): the
/// authserver's internal API. `None` = standalone server (no ticket gate, no reporting).
#[derive(Resource, Clone)]
pub struct GameServerCoordinator {
    pub endpoint: String,
    /// This server's slot, matching the `srv` claim of the tickets it accepts.
    pub slot: u32,
    pub token: String,
}

impl std::fmt::Debug for GameServerCoordinator {
    /// Never print the token (it goes to the logs).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameServerCoordinator")
            .field("endpoint", &self.endpoint)
            .field("slot", &self.slot)
            .finish()
    }
}

impl GameServerCoordinator {
    /// `ROTS_COORDINATOR_URL`, unset or empty = standalone. With it set, `ROTS_SERVER_SLOT` and
    /// `ROTS_COORDINATOR_TOKEN` are required.
    pub fn from_env() -> Option<Self> {
        let endpoint = std::env::var("ROTS_COORDINATOR_URL")
            .ok()
            .filter(|v| !v.is_empty())?;
        let slot = std::env::var("ROTS_SERVER_SLOT")
            .expect("ROTS_SERVER_SLOT must be set with ROTS_COORDINATOR_URL")
            .parse::<u32>()
            .expect("ROTS_SERVER_SLOT must be a non-negative integer");
        let token = std::env::var("ROTS_COORDINATOR_TOKEN")
            .expect("ROTS_COORDINATOR_TOKEN must be set with ROTS_COORDINATOR_URL");
        Some(Self {
            endpoint,
            slot,
            token,
        })
    }
}

/// How a server process was launched.
pub struct ServerSettings {
    pub verifier: TicketVerifier,
    /// `None`: bind from config.yaml, overridden by `ROTS_BIND_IP` / `ROTS_PORT`.
    pub bind: Option<NetworkConnectionTarget>,
    /// `None`: random terrain (`TerrainParams::default()`).
    pub terrain: Option<TerrainParams>,
    pub coordinator: Option<GameServerCoordinator>,
    /// `Some(name)`: broadcast a LAN testing gym beacon under this name.
    pub lan_beacon: Option<String>,
}

/// Minimum time per server loop iteration (at most 200 Hz). Bevy's default runner never sleeps
/// and burns a whole core; FixedUpdate keeps its 60 Hz tick regardless.
pub const SERVER_LOOP_WAIT: std::time::Duration = std::time::Duration::from_millis(5);

/// The single server startup path: dedicated servers (server/src/main.rs) and
/// client-hosted testing gyms (host_testing_server). Blocks for the life of the server.
pub fn run_server(
    base_plugins: bevy::app::PluginGroupBuilder,
    tokio_runtime: Arc<tokio::runtime::Runtime>,
    settings: ServerSettings,
) {
    let ServerSettings {
        verifier,
        bind,
        terrain,
        coordinator,
        lan_beacon,
    } = settings;
    do_app(base_plugins, |app| {
        if matches!(verifier, TicketVerifier::Disabled) {
            app.add_systems(Startup, || warn!("ticket verification disabled"));
        }
        app.insert_resource(verifier);
        let link_runtime = tokio_runtime.clone();
        app.insert_resource(shared::tokio_udp::TokioRuntimeResource(tokio_runtime));
        app.add_plugins(axum::AxumServerPlugin);
        match bind {
            Some(target) => {
                app.insert_resource(target);
            }
            None => {
                app.add_systems(Startup, add_network_connection_info_from_config);
            }
        }
        if let Some(terrain) = terrain {
            app.insert_resource(terrain);
        }
        match coordinator {
            Some(c) => {
                info!("coordinator: {:?}", c);
                app.insert_resource(coordinator::spawn_link(&link_runtime, c.clone()));
                app.insert_resource(c);
                app.add_plugins(coordinator::CoordinatorPlugin);
            }
            None => info!("coordinator: none (standalone)"),
        }
        if let Some(name) = lan_beacon {
            app.add_plugins(gym_beacon::GymBeaconPlugin { name });
        }
    });
}

/// Host a gym inside the client process ("Enter testing"). Blocks forever.
/// The client passes the base plugin group because feature unification gives this crate the
/// client's windowing features there: the caller must leave out the window, event loop and
/// logging plugins the client already owns.
pub fn host_testing_server(base_plugins: bevy::app::PluginGroupBuilder, host_name: String) {
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap(),
    );
    let runtime2 = runtime.clone();
    runtime.block_on(async move {
        info!(
            "Hosting testing gym on 0.0.0.0:{}",
            shared::gym::GYM_GAME_PORT
        );
        run_server(
            base_plugins,
            runtime2,
            ServerSettings {
                verifier: TicketVerifier::Disabled,
                bind: Some(NetworkConnectionTarget {
                    ip: "0.0.0.0".to_string(),
                    port: shared::gym::GYM_GAME_PORT,
                }),
                terrain: Some(TerrainParams::gym()),
                coordinator: None,
                lan_beacon: Some(host_name),
            },
        );
    });
}

fn do_app(base_plugins: bevy::app::PluginGroupBuilder, f: impl FnOnce(&mut App)) {
    info!("Main Start");
    let mut app = App::new();

    app.insert_resource(EndpointToPlayerId::default())
        .insert_resource(HeartbeatList::default())
        .init_resource::<PendingJoins>()
        .init_resource::<ReadyToAdmit>()
        .add_message::<PlayerDisconnected>()
        .add_message::<DespawnUnit2>()
        .add_plugins(base_plugins)
        .add_plugins(avian3d::PhysicsPlugins::default())
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
                (vet_connect_requests, poll_pending_joins, admit_players).chain(),
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

/// `ROTS_BIND_IP` / `ROTS_PORT` override config.yaml, which containers cannot edit.
fn add_network_connection_info_from_config(config: Res<Config>, mut commands: Commands) {
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    commands.insert_resource(NetworkConnectionTarget {
        ip: env("ROTS_BIND_IP")
            .unwrap_or_else(|| config.host_ip.as_ref().unwrap_or(&config.ip).clone()),
        port: env("ROTS_PORT").map_or(config.port, |p| {
            p.parse().expect("ROTS_PORT must be a port number")
        }),
    });
}

/// This component is added to each of the meta entities representing a connected player
#[derive(Component)]
pub struct ConnectedPlayer;

#[derive(Component)]
pub struct DisconnectedPlayer {
    pub disconnect_tick: Tick,
}

/// A verified connect request on its way to admission.
pub struct AdmitRequest {
    endpoint: EndpointGeneral,
    verified: VerifiedTicket,
    /// The name the client asked for; guests keep it.
    name: Option<String>,
    my_location: Transform,
    color_hue: f32,
}

impl AdmitRequest {
    /// How the coordinator knows this player, when the ticket carried a login session.
    pub fn player_ref(&self) -> Option<coordinator::PlayerRef> {
        self.verified
            .sid
            .as_ref()
            .map(|sid| coordinator::PlayerRef {
                character_id: self.verified.player_id.0 as i64,
                sid: sid.clone(),
            })
    }
}

pub struct PendingJoin {
    pub request: AdmitRequest,
    since: Instant,
    answer: tokio::sync::oneshot::Receiver<Result<(), String>>,
}

/// Joins sent to the coordinator whose answer has not arrived yet.
#[derive(Resource, Default)]
pub struct PendingJoins(pub Mutex<Vec<PendingJoin>>);

/// Connect requests cleared to admit; `admit_players` admits them in the same frame.
#[derive(Resource, Default)]
pub struct ReadyToAdmit(pub Mutex<Vec<AdmitRequest>>);

/// The login session a connected player signed in with. Absent for unverified connects (testing
/// gym, `ROTS_AUTH_DISABLED=1`).
#[derive(Component)]
pub struct PlayerSid(pub String);

/// Verify each `ConnectRequest`, then either queue it for the coordinator (gate) or clear it to
/// admit right away (standalone servers and testing gyms).
fn vet_connect_requests(
    mut new_players: UDPacketEvent<shared::event::server::ConnectRequest>,
    world: &World,
    link: Option<Res<coordinator::CoordinatorLink>>,
    pending: Res<PendingJoins>,
    ready: Res<ReadyToAdmit>,
) {
    let sr = world.resource::<ServerNetworkingResources>().clone();
    let verifier = world.resource::<TicketVerifier>();
    // Character -> the endpoint they are connected, joining or waiting to be admitted from.
    // Updated as requests pass vetting so two requests for one character in the same frame
    // cannot both get through.
    let mut known: HashMap<PlayerId, EndpointGeneral> = world
        .try_query_filtered::<(&PlayerId, &PlayerEndpoint), With<ConnectedPlayer>>()
        .map(|mut q| q.iter(world).map(|(id, ep)| (*id, ep.0)).collect())
        .unwrap_or_default();
    for join in pending.0.lock().expect("pending joins lock").iter() {
        known
            .entry(join.request.verified.player_id)
            .or_insert(join.request.endpoint);
    }
    for request in ready.0.lock().expect("ready to admit lock").iter() {
        known
            .entry(request.verified.player_id)
            .or_insert(request.endpoint);
    }

    for player in new_players.read() {
        info!("Got packet");
        let verified = match verifier.verify(&player.event.ticket) {
            Ok(verified) => verified,
            Err(reason) => {
                reject_connection(&sr, player.endpoint, reason);
                continue;
            }
        };

        match known.get(&verified.player_id) {
            // The client resends ConnectRequest every second until WorldData2 arrives; a resend
            // from the connection we already accepted is not a second login.
            Some(endpoint) if *endpoint == player.endpoint => {
                debug!(who = ?player.endpoint, "Ignoring repeated ConnectRequest");
                continue;
            }
            Some(_) => {
                reject_connection(
                    &sr,
                    player.endpoint,
                    "character already connected".to_string(),
                );
                continue;
            }
            None => {}
        }
        known.insert(verified.player_id, player.endpoint);

        let request = AdmitRequest {
            endpoint: player.endpoint,
            verified,
            name: player.event.name.clone(),
            my_location: player.event.my_location,
            color_hue: player.event.color_hue,
        };
        // The authserver's coordinator decides joins; a server without one admits immediately.
        match (&link, request.player_ref()) {
            (Some(link), Some(player_ref)) => {
                let answer = link.join(player_ref.character_id, player_ref.sid);
                pending
                    .0
                    .lock()
                    .expect("pending joins lock")
                    .push(PendingJoin {
                        request,
                        since: Instant::now(),
                        answer,
                    });
            }
            _ => ready.0.lock().expect("ready to admit lock").push(request),
        }
    }
}

/// Clear joins the coordinator accepted and reject the ones it refused, gave no answer for, or
/// cannot be told about.
fn poll_pending_joins(
    pending: Res<PendingJoins>,
    ready: Res<ReadyToAdmit>,
    sr: Res<ServerNetworkingResources>,
) {
    use tokio::sync::oneshot::error::TryRecvError;

    enum Answer {
        Wait,
        Admit,
        Reject(String),
        GiveUp,
    }

    let mut pending = pending.0.lock().expect("pending joins lock");
    let mut index = 0;
    while index < pending.len() {
        let answer = {
            let join = &mut pending[index];
            match join.answer.try_recv() {
                Ok(Ok(())) => Answer::Admit,
                Ok(Err(reason)) => Answer::Reject(reason),
                Err(TryRecvError::Empty) => {
                    if join.since.elapsed() > coordinator::JOIN_GIVE_UP {
                        Answer::GiveUp
                    } else {
                        Answer::Wait
                    }
                }
                Err(TryRecvError::Closed) => Answer::GiveUp,
            }
        };
        match answer {
            Answer::Wait => index += 1,
            Answer::Admit => {
                let join = pending.remove(index);
                ready
                    .0
                    .lock()
                    .expect("ready to admit lock")
                    .push(join.request);
            }
            Answer::Reject(reason) => {
                let join = pending.remove(index);
                reject_connection(&sr, join.request.endpoint, reason);
            }
            Answer::GiveUp => {
                let join = pending.remove(index);
                error!(
                    who = ?join.request.endpoint,
                    "join: no answer from the coordinator"
                );
                reject_connection(
                    &sr,
                    join.request.endpoint,
                    "coordinator unavailable".to_string(),
                );
            }
        }
    }
}

/// Admit the connect requests cleared in this frame: spawn the player and send them the world.
fn admit_players(ready: Res<ReadyToAdmit>, world: &World, mut commands: Commands) {
    let requests: Vec<AdmitRequest> =
        std::mem::take(&mut *ready.0.lock().expect("ready to admit lock"));
    if requests.is_empty() {
        return;
    }
    let sr = world.resource::<ServerNetworkingResources>().clone();
    let terrain = world.resource::<TerrainParams>().clone();
    // Requests admitted earlier in this frame are not `ConnectedPlayer`s yet (that needs the
    // commands to apply), so keep our own map to reject a second connection for one character.
    let mut connected: HashMap<PlayerId, EndpointGeneral> = world
        .try_query_filtered::<(&PlayerId, &PlayerEndpoint), With<ConnectedPlayer>>()
        .map(|mut q| q.iter(world).map(|(id, ep)| (*id, ep.0)).collect())
        .unwrap_or_default();
    for request in requests {
        match connected.get(&request.verified.player_id) {
            Some(endpoint) if *endpoint == request.endpoint => {
                debug!(who = ?request.endpoint, "Ignoring repeated ConnectRequest");
                continue;
            }
            Some(_) => {
                reject_connection(
                    &sr,
                    request.endpoint,
                    "character already connected".to_string(),
                );
                continue;
            }
            None => {}
        }
        connected.insert(request.verified.player_id, request.endpoint);
        admit(request, &sr, &terrain, world, &mut commands);
    }
}

fn admit(
    request: AdmitRequest,
    sr: &ServerNetworkingResources,
    terrain: &TerrainParams,
    world: &World,
    commands: &mut Commands,
) {
    let AdmitRequest {
        endpoint,
        verified,
        name: requested_name,
        my_location,
        color_hue,
    } = request;
    // Account tickets carry the character name; guests keep the name they asked for.
    let name = verified
        .name
        .or_else(|| requested_name)
        .unwrap_or_else(|| format!("Player #{}", rand::rng().random_range(1..10000)));

    let spawn_location = my_location;
    let player_color = PlayerColor { hue: color_hue };

    let new_player_id = verified.player_id;

    // Spawn player entity as ConnectedPlayer
    // SPAWN A
    let mut player_entity = commands.spawn((
        PlayerName { name: name.clone() },
        player_color.clone(),
        new_player_id,
        PlayerEndpoint(endpoint),
        ConnectedPlayer,
    ));
    if let Some(sid) = &verified.sid {
        player_entity.insert(PlayerSid(sid.clone()));
    }

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
    let ent = spawn_camera_unit.clone().spawn_entity(commands);
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
            let ciids = component_info.map(|(id, _)| id).collect::<HashSet<_>>();
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
                    shared::net_components::NetComponent::from_type_id_ptr(type_id, *component_ptr)
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
        .insert(endpoint, new_player_id);

    // Finally, tell the client all this info.
    let world_data = WorldData2 {
        your_player_id: new_player_id,
        your_camera_unit_id: spawn_camera_unit.net_ent_id,
        terrain_params: terrain.clone(),
        units: unit_list_to_new_client,
    };

    // send initial world data
    info!(
        who = ?endpoint,
        "Player connected - sending {} existing units",
        world_data.units.len()
    );
    let event = EventToClient::WorldData2(world_data);
    sr.send_outgoing_event_next_tick(endpoint, &event);

    // send remaining world data in batches
    let events = large_unit_list_to_send
        .iter()
        .map(|u| EventToClient::SpawnUnit2(u.clone()))
        .collect::<Vec<_>>();

    sr.send_outgoing_event_next_tick_batch(endpoint, &events);
}

fn reject_connection(sr: &ServerNetworkingResources, endpoint: EndpointGeneral, reason: String) {
    info!(who = ?endpoint, %reason, "Rejected connection");
    sr.send_outgoing_event_now(
        endpoint,
        &EventToClient::ConnectRejected(ConnectRejected { reason }),
        None,
    );
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
        info!("Received disconnect packet from {:?}", dp.endpoint);
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
        sr.send_outgoing_event_now(net_client.0, &event, None);
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

        info!("Player {:?} disconnected: {}", player.id, player.reason);

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
                sr.send_outgoing_event_now(hb.endpoint, &event, None);
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
        sr.send_outgoing_event_now(req.endpoint, &event, None);
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
