use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use avian3d::prelude::{Gravity, LinearVelocity, Physics, PhysicsTime, Rotation};
use bevy::{platform::collections::HashSet, prelude::*};
use rand::RngExt;
use shared::{
    Config, ConfigPlugin, CurrentTick, PlayerPing, PlayerPingAtomic, PlayerPingInteger,
    event::{
        NetEntId, PlayerId, UDPacketEvent,
        client::{
            BeginThirdpersonControllingUnit, ConnectRejected, DespawnUnit2, HeartbeatChallenge,
            HeartbeatResponse, PlayerDisconnected, SpawnUnit2, UpdateUnit2, WorldData2,
        },
        server::{ChangeMovement, Heartbeat, HeartbeatChallengeResponse, IWantToDisconnect},
    },
    net_components::{
        ToNetComponent,
        ents::{Man, PlayerCamera, SendNetworkTranformUpdates},
        make_ball,
        ours::{ControlledBy, OwnedByPlayer, PlayerColor, PlayerName},
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
pub mod lifecycle;
pub mod projectile;
pub mod spawns;
pub mod statefile;
pub mod terrain;
pub mod ticket;
pub mod websocket;

use ticket::{TicketVerifier, VerifiedTicket};

/// Private internal coordinator for this server (player placement, server lifecycle): the
/// authserver's internal API. `None` = standalone server (no ticket gate, no reporting).
///
/// Every report names the instance it comes from, `<slot, generation>`: the coordinator's
/// `GameServerWorkflow` for this server is `rots-gs-<queue>-<slot>-g<generation>`.
#[derive(Resource, Clone)]
pub struct GameServerCoordinator {
    pub endpoint: String,
    /// This server's slot, matching the `srv` claim of the tickets it accepts.
    pub slot: u32,
    /// The instance generation (`ROTS_SERVER_GENERATION`). A static server, which no provisioner
    /// created, reports the first generation, 1.
    pub generation: u64,
    /// The public UDP port this instance is reachable on (`ROTS_PUBLIC_UDP_PORT`), when the
    /// provisioner set one.
    pub udp_port: Option<u16>,
    /// Set when `ROTS_SERVER_GENERATION` is present: a provisioner created this instance, so it
    /// owns a volume and a workflow, and a report the coordinator does not recognise is fatal
    /// rather than noise. Static servers (prod, local dev) are never in that position.
    pub managed: bool,
    pub token: String,
    /// `ROTS_PRE_GAME=1`: the provisioner created this instance for the warm pool. Its world is
    /// built, its sockets are up and it reports, but it stays paused until the coordinator claims
    /// it with `POST /activate`.
    pub pre_game: bool,
}

impl std::fmt::Debug for GameServerCoordinator {
    /// Never print the token (it goes to the logs).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameServerCoordinator")
            .field("endpoint", &self.endpoint)
            .field("slot", &self.slot)
            .field("generation", &self.generation)
            .field("udp_port", &self.udp_port)
            .field("managed", &self.managed)
            .field("pre_game", &self.pre_game)
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
        let slot = parse_env::<u32>("ROTS_SERVER_SLOT")
            .expect("ROTS_SERVER_SLOT must be set with ROTS_COORDINATOR_URL");
        let generation = parse_env::<u64>("ROTS_SERVER_GENERATION");
        let udp_port = parse_env::<u16>("ROTS_PUBLIC_UDP_PORT");
        let token = std::env::var("ROTS_COORDINATOR_TOKEN")
            .expect("ROTS_COORDINATOR_TOKEN must be set with ROTS_COORDINATOR_URL");
        // Only a provisioned instance is ever activated: a static server has no coordinator to
        // claim it, so it never boots into a world it cannot leave.
        let pre_game = generation.is_some() && parse_env::<u8>("ROTS_PRE_GAME") == Some(1);
        Some(Self {
            endpoint,
            slot,
            generation: generation.unwrap_or(1),
            udp_port,
            managed: generation.is_some(),
            token,
            pre_game,
        })
    }
}

/// A numeric environment variable, read as `None` when unset or empty.
pub(crate) fn parse_env<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse()
                .unwrap_or_else(|_| panic!("{name} must be a number"))
        })
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
    // The sentinel check has to happen before the world exists: a generation that already shut
    // down exits here, so a StatefulSet restarting it does not bring back a dead world.
    if let Some(c) = &coordinator {
        lifecycle::exit_if_already_shut_down(c);
    }
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
                if c.managed {
                    // A provisioned instance owns its own lifecycle: it destroys itself when it
                    // is idle and migrates its players off on SIGTERM. Static servers have no
                    // such env, so none of this exists for them (prod, local dev).
                    app.add_plugins(lifecycle::LifecyclePlugin::new(&c, &link_runtime));
                    // `ROTS_STATE_DIR` gives the instance a world that outlives its process: the
                    // StatefulSet restarts the pod on the same volume and the world comes back.
                    if let Some(statefile) = statefile::StatefilePlugin::from_env() {
                        app.add_plugins(statefile);
                    }
                }
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
        .init_resource::<Paused>()
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
                // Input is applied to the world, so it stops with the world; the packets are
                // still read above.
                on_movement.run_if(world_not_paused),
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
                // A paused world does not tick; the flush below still runs (networking keeps
                // going while a world is paused).
                shared::increment_ticks.run_if(shared::physics_running),
                shared::netlib::flush_outgoing_events_udp::<EventToServer, EventToClient>,
                // The world is not ticking while paused, so neither is the tick stream.
                add_tick_just_happened_packet.run_if(world_not_paused),
            )
                .chain()
                .run_if(in_state(ServerState::Running)),
        )
        .add_systems(Update, apply_pause)
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

/// Why the world is not advancing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseReason {
    /// A restored world, waiting for its players to reconnect (or for the grace period to end).
    Restoring,
    /// Provisioned into the warm pool: the world exists and the server answers, but it is not
    /// online until the coordinator activates it.
    PreGame,
}

/// The world's pause switch.
///
/// A paused world does not tick: physics is paused (`Time<Physics>::pause()`), and the systems
/// that step the simulation are skipped with [`world_not_paused`]. Networking, heartbeats and
/// input handling keep running — which is why `Time<Virtual>` must never be frozen here: the UDP
/// flush, the tick increment and `TickHappened` all run in `FixedPostUpdate`.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct Paused {
    reason: Option<PauseReason>,
}

impl Paused {
    pub fn is_paused(&self) -> bool {
        self.reason.is_some()
    }

    pub fn reason(&self) -> Option<PauseReason> {
        self.reason
    }

    /// Pause the world; unpausing is the caller's job once its condition is met.
    pub fn pause(&mut self, reason: PauseReason) {
        if self.reason != Some(reason) {
            info!(?reason, "pausing the world");
            self.reason = Some(reason);
        }
    }

    pub fn unpause(&mut self) {
        if let Some(reason) = self.reason.take() {
            info!(?reason, "resuming the world");
        }
    }
}

/// Run condition: the world is advancing.
pub fn world_not_paused(paused: Res<Paused>) -> bool {
    !paused.is_paused()
}

/// Keep avian's physics clock in step with [`Paused`]. A paused physics clock stops the simulation
/// from stepping; everything else in `FixedUpdate` keeps whatever schedule it had.
fn apply_pause(paused: Res<Paused>, mut physics: ResMut<Time<Physics>>) {
    if paused.is_paused() == physics.is_paused() {
        return;
    }
    if paused.is_paused() {
        physics.pause();
    } else {
        physics.unpause();
    }
}

/// The world outlives a disconnect: a character whose player disconnects stays in the world as a
/// ghost, keeps its units and camera, and is only released when it dies.
///
/// Set by [`statefile::StatefilePlugin`], so it exists on managed instances that keep their world on
/// a volume. A static server (prod, local dev) has no statefile to restore a world from, so it
/// behaves exactly as before: a disconnect despawns the character's units and reports `leave`.
#[derive(Resource, Debug, Clone, Copy)]
pub struct KeepGhosts;

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

/// A character that disconnected but is still in the world: its player entity, and — unless it
/// died — the camera unit it drives.
struct Ghost {
    player_entity: Entity,
    /// The camera unit the character drives, if it still has one (death despawns it).
    camera: Option<(Entity, NetEntId)>,
}

/// Find the ghost of `player_id`, if this character was left in the world by a disconnect.
///
/// Only a world that keeps ghosts has any: a static server despawned the character's units when it
/// disconnected, so a returning player is admitted fresh, exactly as before.
fn ghost_of(world: &World, player_id: PlayerId) -> Option<Ghost> {
    world.get_resource::<KeepGhosts>()?;
    let player_entity = world
        .try_query_filtered::<(Entity, &PlayerId), With<DisconnectedPlayer>>()
        .and_then(|mut ghosts| {
            ghosts
                .iter(world)
                .find(|(_, id)| **id == player_id)
                .map(|(entity, _)| entity)
        })?;
    let camera = world
        .try_query_filtered::<(Entity, &NetEntId, &OwnedByPlayer), With<PlayerCamera>>()
        .and_then(|mut cameras| {
            cameras
                .iter(world)
                .find(|(_, _, owner)| owner.player_id == player_id)
                .map(|(entity, net_ent_id, _)| (entity, *net_ent_id))
        });
    Some(Ghost {
        player_entity,
        camera,
    })
}

/// The unit a character is driving, if they have taken control of one. A rebound player comes back
/// in third person when they had a `Man`, instead of in freecam.
fn controlled_man(world: &World, player_id: PlayerId) -> Option<NetEntId> {
    world
        .try_query_filtered::<(&NetEntId, &ControlledBy), With<Man>>()
        .and_then(|mut men| {
            men.iter(world)
                .find(|(_, controller)| controller.players.contains(&player_id))
                .map(|(net_ent_id, _)| *net_ent_id)
        })
}

/// Everything a client needs to spawn `entity`: the per-entity component walk of [`WorldData2`],
/// shared with the statefile (which has to bring units back exactly as they were).
pub(crate) fn unit_snapshot(world: &World, entity: Entity) -> Option<SpawnUnit2> {
    let net_ent_id = *world.get::<NetEntId>(entity)?;
    let component_info = world.inspect_entity(entity).ok()?;
    let ciids = component_info.map(|(id, _)| id).collect::<HashSet<_>>();
    let Ok(ents_res) = world.entity(entity).get_by_id(&ciids) else {
        error!("Failed to get components for entity {:?}", entity);
        return None;
    };

    let mut spawn_unit = SpawnUnit2 {
        net_ent_id,
        components: vec![],
    };
    for (component_id, component_ptr) in ents_res.iter() {
        let Some(type_id) = world
            .components()
            .get_info(*component_id)
            .and_then(|info| info.type_id())
        else {
            continue;
        };
        // SAFETY: Trust that bevy gives us a valid type id and pointer from `get_by_id`
        if let Some(net_comp) = unsafe {
            shared::net_components::NetComponent::from_type_id_ptr(type_id, *component_ptr)
        } {
            trace!("Component to send: {:?}", net_comp);
            spawn_unit.components.push(net_comp);
        }
    }
    Some(spawn_unit)
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

    // A character that was left in the world by a disconnect keeps everything they had: the
    // connection is new, the character is not. Rebinding reuses their player entity and camera
    // unit instead of spawning a second copy of both.
    let ghost = ghost_of(world, new_player_id);
    let is_new_character = ghost.is_none();
    let rebind_camera = ghost.as_ref().and_then(|ghost| ghost.camera);

    // The unit to represent the player themselves, unless they already have one.
    // SPAWN B
    let spawn_camera_unit = rebind_camera.is_none().then(|| SpawnUnit2 {
        net_ent_id: NetEntId::random(),
        components: vec![
            PlayerCamera.to_net_component(),
            spawn_location.to_net_component(),
            PlayerName { name: name.clone() }.to_net_component(),
            player_color.clone().to_net_component(),
            ControlledBy::single(new_player_id).to_net_component(),
            SendNetworkTranformUpdates.to_net_component(),
        ],
    });
    let camera_unit_id = rebind_camera
        .map(|(_, net_ent_id)| net_ent_id)
        .unwrap_or_else(|| {
            spawn_camera_unit
                .as_ref()
                .expect("a new camera unit when the character has none")
                .net_ent_id
        });

    // Sent once the client has the world, so it re-enters the body it was driving.
    let mut cam_back: Option<NetEntId> = None;
    match ghost {
        Some(Ghost {
            player_entity,
            camera: Some(_),
        }) => {
            // Back from a disconnect: the player entity and the camera unit are already here.
            let mut rebind = commands.entity(player_entity);
            rebind
                .remove::<DisconnectedPlayer>()
                .insert(ConnectedPlayer)
                .insert(PlayerEndpoint(endpoint));
            if let Some(sid) = &verified.sid {
                rebind.insert(PlayerSid(sid.clone()));
            }
            info!(
                ?new_player_id,
                ?camera_unit_id,
                "rebound a disconnected character"
            );
            cam_back = controlled_man(world, new_player_id);
        }
        ghost => {
            // Rebind (the character died and lost its camera) or admit a character we have
            // never seen, then give both a camera unit.
            match ghost {
                Some(Ghost { player_entity, .. }) => {
                    // The character is here but died: the same player entity gets a new camera.
                    let mut existing = commands.entity(player_entity);
                    existing
                        .remove::<DisconnectedPlayer>()
                        .insert(ConnectedPlayer)
                        .insert(PlayerEndpoint(endpoint));
                    if let Some(sid) = &verified.sid {
                        existing.insert(PlayerSid(sid.clone()));
                    }
                }
                None => {
                    // Spawn player entity as ConnectedPlayer
                    // SPAWN A
                    let mut spawned = commands.spawn((
                        PlayerName { name: name.clone() },
                        player_color.clone(),
                        new_player_id,
                        PlayerEndpoint(endpoint),
                        ConnectedPlayer,
                    ));
                    if let Some(sid) = &verified.sid {
                        spawned.insert(PlayerSid(sid.clone()));
                    }
                }
            }

            let ent = spawn_camera_unit
                .clone()
                .expect("a new camera unit when the character has none")
                .spawn_entity(commands);
            commands.entity(ent).insert(OwnedByPlayer {
                player_id: new_player_id,
            });
        }
    }

    let player_unit = SpawnUnit2 {
        net_ent_id: NetEntId::none(),
        components: vec![
            PlayerName { name: name.clone() }.to_net_component(),
            player_color.clone().to_net_component(),
            new_player_id.to_net_component(),
            //ConnectedPlayer.to_net_component(),
        ],
    };

    // What the other clients need to see of this player. Their disconnect removed the visuals,
    // not the units, so a rebinding character needs its camera announced again — but its player
    // unit never went away.
    let mut announcement = vec![];
    if let Some(camera) = &spawn_camera_unit {
        // SPAWN B
        announcement.push(EventToClient::SpawnUnit2(camera.clone()));
    } else if let Some((entity, _)) = rebind_camera
        && let Some(camera) = unit_snapshot(world, entity)
    {
        announcement.push(EventToClient::SpawnUnit2(camera));
    }
    if is_new_character {
        announcement.push(EventToClient::SpawnUnit2(player_unit));
    }

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
            sr.send_outgoing_event_next_tick_batch(c_net_client.0, &announcement);
        }
    }

    let mut large_unit_list_to_send: Vec<SpawnUnit2> = vec![];

    let units_to_spawn =
        world.try_query_filtered::<(Entity, &NetEntId), Without<ConnectedPlayer>>();

    if let Some(mut units_spawns_thing) = units_to_spawn {
        for (unit_ent, unit_net_ent_id) in units_spawns_thing.iter(world) {
            // The client sets up its own camera from `your_camera_unit_id`.
            if *unit_net_ent_id == camera_unit_id {
                continue;
            }
            info!(
                "Preparing to send existing unit {:?} to new player",
                unit_net_ent_id
            );
            if let Some(unit) = unit_snapshot(world, unit_ent) {
                large_unit_list_to_send.push(unit);
            }
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
        your_camera_unit_id: camera_unit_id,
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

    // A rebound character that was driving a unit comes back in third person; the client starts
    // every connection in freecam. Sent after the world so the unit already exists there.
    if let Some(unit) = cam_back {
        info!(?unit, "re-entering the unit the character was driving");
        sr.send_outgoing_event_next_tick(
            endpoint,
            &EventToClient::BeginThirdpersonControllingUnit(BeginThirdpersonControllingUnit {
                player_id: new_player_id,
                unit: Some(unit),
            }),
        );
    }
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

/// A disconnect ends the connection. In a world that keeps ghosts ([`KeepGhosts`]) the character
/// stays in it too: its units are untouched, and it is released only when it dies (see
/// [`on_unit_despawn`]). A static server has no world to restore, so it behaves as before: the
/// character's units leave with the connection and its `leave` is reported.
fn on_player_disconnect(
    mut pd: MessageReader<PlayerDisconnected>,
    clients: Query<(Entity, &PlayerEndpoint, &PlayerId), With<ConnectedPlayer>>,
    owned_units: Query<(&NetEntId, &OwnedByPlayer)>,
    ghosts: Option<Res<KeepGhosts>>,
    link: Option<Res<crate::coordinator::CoordinatorLink>>,
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

        if ghosts.is_some() {
            info!(
                "Player {:?} disconnected: {} (their units stay in the world)",
                player.id, player.reason
            );
        } else {
            // Static mode: the character goes with the connection, as it did before ghosts.
            info!(
                "Player {:?} disconnected: {} (their units leave the world)",
                player.id, player.reason
            );
            for (owned_ent_id, owner) in &owned_units {
                if owner.player_id == player.id {
                    despawn_unit.write(DespawnUnit2 {
                        net_ent_id: *owned_ent_id,
                    });
                }
            }
            if let Some(link) = link.as_deref() {
                link.leave(player.id.0 as i64);
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
    units: Query<(Entity, &NetEntId, Has<PlayerCamera>, Option<&OwnedByPlayer>)>,
    ghosts: Option<Res<KeepGhosts>>,
    link: Option<Res<crate::coordinator::CoordinatorLink>>,
    mut commands: Commands,
    sr: Res<ServerNetworkingResources>,
) {
    let mut events = vec![];
    for despawn in pd.read() {
        'unit: for (unit_ent, unit_net_ent_id, is_camera, owner) in &units {
            if unit_net_ent_id == &despawn.net_ent_id {
                // A character's camera unit leaving the world is that character leaving it (its
                // death): the coordinator may give its slot to somebody else. Only a world that
                // keeps ghosts has characters that outlive their connection; a static server
                // reports the `leave` of a disconnect above instead.
                if ghosts.is_some()
                    && is_camera
                    && let (Some(owner), Some(link)) = (owner, link.as_deref())
                {
                    info!(
                        character_id = owner.player_id.0,
                        "character left the world: reporting release"
                    );
                    link.leave(owner.player_id.0 as i64);
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::netlib::{NetworkingResources, Tick, WebSocketEndpoint};

    fn networking() -> ServerNetworkingResources {
        NetworkingResources {
            event_list_incoming_udp: Default::default(),
            event_list_incoming_websocket: Default::default(),
            event_list_outgoing_udp: Default::default(),
            event_list_outgoing_websocket: Default::default(),
            reliable_packet_ids_seen: Default::default(),
            networking_stats: Default::default(),
            // Test endpoints are WebSockets: their events are queued for inspection instead of
            // being sent, so no socket is needed.
            handler: None,
            con_str: std::sync::Arc::new(("127.0.0.1".to_string(), 1)),
        }
    }

    /// An app with just the connection-handling systems: no window, no physics, no state.
    ///
    /// `keep_ghosts` is the marker [`statefile::StatefilePlugin`] inserts on an instance that keeps
    /// its world on a volume; without it the app behaves like a static server.
    fn app(keep_ghosts: bool) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TerrainParams::default())
            .insert_resource(networking())
            .init_resource::<HeartbeatList>()
            .init_resource::<EndpointToPlayerId>()
            .init_resource::<PendingJoins>()
            .init_resource::<ReadyToAdmit>()
            .insert_resource(CurrentTick(Tick(11)))
            .add_message::<PlayerDisconnected>()
            .add_message::<DespawnUnit2>()
            .add_systems(
                Update,
                (admit_players, on_player_disconnect, on_unit_despawn).chain(),
            );
        if keep_ghosts {
            app.insert_resource(KeepGhosts);
        }
        app
    }

    fn endpoint(port: u16) -> EndpointGeneral {
        EndpointGeneral::WebSocket(WebSocketEndpoint {
            socket_addr: format!("127.0.0.1:{port}").parse().unwrap(),
        })
    }

    fn request(player_id: u64, endpoint: EndpointGeneral) -> AdmitRequest {
        AdmitRequest {
            endpoint,
            verified: ticket::VerifiedTicket {
                player_id: PlayerId(player_id),
                name: Some("alice".to_string()),
                sid: Some("0123456789abcdef".to_string()),
            },
            name: None,
            my_location: Transform::from_xyz(1.0, 2.0, 3.0),
            color_hue: 10.0,
        }
    }

    /// Clear a connect request for admission and run a frame, so its spawns are applied.
    fn admit_now(app: &mut App, request: AdmitRequest) {
        app.world()
            .resource::<ReadyToAdmit>()
            .0
            .lock()
            .expect("ready to admit lock")
            .push(request);
        app.update();
    }

    fn camera_units(app: &mut App, player_id: u64) -> Vec<NetEntId> {
        let world = app.world_mut();
        let mut query = world.query_filtered::<(&NetEntId, &OwnedByPlayer), With<PlayerCamera>>();
        query
            .iter(world)
            .filter(|(_, owner)| owner.player_id == PlayerId(player_id))
            .map(|(id, _)| *id)
            .collect()
    }

    fn players<F: bevy::ecs::query::QueryFilter>(app: &mut App) -> Vec<PlayerId> {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&PlayerId, F>();
        query.iter(world).copied().collect()
    }

    fn unit_count(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<(), With<NetEntId>>();
        query.iter(world).count()
    }

    fn queued_events(app: &mut App, endpoint: EndpointGeneral) -> Vec<EventToClient> {
        let EndpointGeneral::WebSocket(ws) = endpoint else {
            panic!("test endpoints are WebSockets");
        };
        let outgoing = app
            .world()
            .resource::<ServerNetworkingResources>()
            .event_list_outgoing_websocket
            .clone();
        outgoing
            .get(&ws)
            .map(|events| events.clone())
            .unwrap_or_default()
    }

    fn queued_world_data(app: &mut App, endpoint: EndpointGeneral) -> Option<WorldData2> {
        let EndpointGeneral::WebSocket(ws) = endpoint else {
            panic!("test endpoints are WebSockets");
        };
        let outgoing = app
            .world()
            .resource::<ServerNetworkingResources>()
            .event_list_outgoing_websocket
            .clone();
        let queued = outgoing
            .get(&ws)
            .map(|events| events.clone())
            .unwrap_or_default();
        queued.into_iter().find_map(|event| match event {
            EventToClient::WorldData2(data) => Some(data),
            _ => None,
        })
    }

    /// A disconnect is not a death: the character's units stay in the world, and reconnecting
    /// puts the player back in the same body instead of spawning a second one.
    #[test]
    fn reconnect_rebinds_the_ghost_camera_unit() {
        let mut app = app(true);
        admit_now(&mut app, request(7, endpoint(5001)));

        let cameras = camera_units(&mut app, 7);
        assert_eq!(cameras.len(), 1, "a character has one camera unit");
        let camera = cameras[0];
        let units = unit_count(&mut app);
        // The player entity is not a unit (it has no NetEntId); the camera is.
        assert_eq!(units, 1);
        assert_eq!(
            queued_world_data(&mut app, endpoint(5001))
                .expect("the joining client is sent the world")
                .your_camera_unit_id,
            camera
        );

        // The connection drops.
        app.world_mut().write_message(PlayerDisconnected {
            id: PlayerId(7),
            reason: "Player timeout".to_string(),
        });
        app.update();

        assert_eq!(
            players::<With<DisconnectedPlayer>>(&mut app),
            vec![PlayerId(7)],
            "the character stays in the world as a ghost"
        );
        assert!(players::<With<ConnectedPlayer>>(&mut app).is_empty());
        assert_eq!(
            camera_units(&mut app, 7),
            vec![camera],
            "a disconnect keeps the character's units"
        );
        assert_eq!(unit_count(&mut app), units, "a disconnect despawns nothing");

        // The player comes back on a new connection.
        admit_now(&mut app, request(7, endpoint(5002)));

        assert!(players::<With<DisconnectedPlayer>>(&mut app).is_empty());
        assert_eq!(
            players::<With<ConnectedPlayer>>(&mut app),
            vec![PlayerId(7)],
            "one player entity, rebound"
        );
        assert_eq!(
            camera_units(&mut app, 7),
            vec![camera],
            "the ghost's camera unit is reused, not replaced"
        );
        assert_eq!(
            unit_count(&mut app),
            units,
            "reconnecting spawns no new units"
        );
        assert_eq!(
            queued_world_data(&mut app, endpoint(5002))
                .expect("the reconnecting client is sent the world")
                .your_camera_unit_id,
            camera,
            "the client is told to drive the camera unit it already had"
        );
    }

    /// A character that died (its camera unit left the world) comes back with a new camera.
    #[test]
    fn reconnecting_after_death_spawns_a_new_camera() {
        let mut app = app(true);
        admit_now(&mut app, request(9, endpoint(5003)));
        let camera = camera_units(&mut app, 9)[0];

        app.world_mut().write_message(PlayerDisconnected {
            id: PlayerId(9),
            reason: "Client quit".to_string(),
        });
        app.update();
        app.world_mut()
            .write_message(DespawnUnit2 { net_ent_id: camera });
        app.update();
        assert!(
            camera_units(&mut app, 9).is_empty(),
            "death despawns the camera unit"
        );

        admit_now(&mut app, request(9, endpoint(5004)));
        let cameras = camera_units(&mut app, 9);
        assert_eq!(cameras.len(), 1);
        assert_ne!(
            cameras[0], camera,
            "a dead character gets a new camera unit"
        );
        assert_eq!(
            players::<With<ConnectedPlayer>>(&mut app),
            vec![PlayerId(9)]
        );
    }

    /// A character that was driving a unit comes back in it, not in freecam.
    #[test]
    fn a_rebound_player_is_put_back_in_the_unit_it_was_driving() {
        let mut app = app(true);
        admit_now(&mut app, request(11, endpoint(5007)));

        // The character took control of a man before the connection dropped.
        let man = NetEntId(4242);
        app.world_mut().spawn((
            man,
            Man(3.0),
            ControlledBy::single(PlayerId(11)),
            OwnedByPlayer {
                player_id: PlayerId(11),
            },
            Transform::from_xyz(2.0, 0.0, 2.0),
        ));

        app.world_mut().write_message(PlayerDisconnected {
            id: PlayerId(11),
            reason: "Player timeout".to_string(),
        });
        app.update();

        admit_now(&mut app, request(11, endpoint(5008)));
        let events = queued_events(&mut app, endpoint(5008));
        assert!(
            events.iter().any(|event| matches!(
                event,
                EventToClient::BeginThirdpersonControllingUnit(
                    BeginThirdpersonControllingUnit {
                        player_id,
                        unit: Some(unit),
                    }
                ) if *player_id == PlayerId(11) && *unit == man
            )),
            "the reconnecting client is told to take control of its unit again: {events:?}"
        );
    }

    /// Records the paths the coordinator link posts to, so a test can assert the wire traffic.
    #[derive(Default)]
    struct SeenLog(std::sync::Mutex<Vec<String>>);

    // `::axum`: inside this crate `axum` names the health-listener module.
    async fn record_request(
        ::axum::extract::State(log): ::axum::extract::State<std::sync::Arc<SeenLog>>,
        request: ::axum::extract::Request,
    ) -> ::axum::response::Response {
        log.0
            .lock()
            .expect("seen log")
            .push(request.uri().path().to_string());
        ::axum::response::IntoResponse::into_response(::axum::http::StatusCode::NO_CONTENT)
    }

    /// A coordinator link pointed at a mock that accepts everything and records the paths.
    fn link_with_mock(
        runtime: &tokio::runtime::Runtime,
    ) -> (crate::coordinator::CoordinatorLink, std::sync::Arc<SeenLog>) {
        let log = std::sync::Arc::new(SeenLog::default());
        let listener =
            runtime.block_on(async { tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap() });
        let address = listener.local_addr().unwrap();
        let router = ::axum::Router::new()
            .fallback(record_request)
            .with_state(log.clone());
        runtime.spawn(async move {
            ::axum::serve(listener, router).await.unwrap();
        });
        let link = crate::coordinator::spawn_link(
            runtime,
            GameServerCoordinator {
                endpoint: format!("http://{address}"),
                slot: 0,
                generation: 1,
                udp_port: None,
                managed: false,
                token: "test-token".to_string(),
                pre_game: false,
            },
        );
        (link, log)
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    fn paths_seen(log: &std::sync::Arc<SeenLog>) -> Vec<String> {
        log.0.lock().expect("seen log").clone()
    }

    fn wait_for_path(log: &std::sync::Arc<SeenLog>, suffix: &str) {
        let start = std::time::Instant::now();
        loop {
            let seen = paths_seen(log);
            if seen.iter().any(|path| path.ends_with(suffix)) {
                return;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "waited for {suffix}, saw {seen:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// A static server has no statefile, so it keeps slice A's disconnect semantics: the
    /// character's units leave the world with the connection, and its `leave` is reported.
    #[test]
    fn a_disconnect_removes_the_character_when_ghosts_are_not_kept() {
        let runtime = runtime();
        let (link, seen) = link_with_mock(&runtime);
        let mut app = app(false);
        app.insert_resource(link);

        admit_now(&mut app, request(7, endpoint(5005)));
        assert_eq!(unit_count(&mut app), 1);

        app.world_mut().write_message(PlayerDisconnected {
            id: PlayerId(7),
            reason: "Player timeout".to_string(),
        });
        app.update();

        assert_eq!(
            unit_count(&mut app),
            0,
            "a static server despawns the character's units"
        );
        assert_eq!(
            players::<With<DisconnectedPlayer>>(&mut app),
            vec![PlayerId(7)],
            "the player entity is kept, as it was before ghosts"
        );
        wait_for_path(&seen, "/internal/v1/servers/leave");
    }

    /// A world that keeps ghosts reports nothing on a disconnect: the character is still there.
    #[test]
    fn a_disconnect_keeps_the_character_when_ghosts_are_kept() {
        let runtime = runtime();
        let (link, seen) = link_with_mock(&runtime);
        let mut app = app(true);
        app.insert_resource(link);

        admit_now(&mut app, request(8, endpoint(5006)));
        let camera = camera_units(&mut app, 8)[0];

        app.world_mut().write_message(PlayerDisconnected {
            id: PlayerId(8),
            reason: "Player timeout".to_string(),
        });
        app.update();

        assert_eq!(
            camera_units(&mut app, 8),
            vec![camera],
            "a ghost keeps its camera unit"
        );
        assert_eq!(unit_count(&mut app), 1, "a disconnect despawns nothing");

        // A leave would have been queued and sent by now; nothing is.
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            paths_seen(&seen).is_empty(),
            "a ghost is not released on disconnect: {:?}",
            paths_seen(&seen)
        );
    }
}
