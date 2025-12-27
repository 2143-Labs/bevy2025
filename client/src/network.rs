use std::{f32::consts::PI, time::Duration};

use avian3d::prelude::Collider;
use bevy::{prelude::*, time::common_conditions::on_timer};
use shared::{
    Config,
    character_controller::CharacterControllerBundle,
    event::{
        MyNetEntParentId, NetEntId, PlayerId, UDPacketEvent,
        client::{
            BeginThirdpersonControllingUnit, HeartbeatChallenge, HeartbeatResponse, SpawnUnit2,
            WorldData2,
        },
        server::{
            ChangeMovement, ConnectRequest, Heartbeat, HeartbeatChallengeResponse, SpawnCircle,
            SpawnMan,
        },
    },
    net_components::{
        ents::{Ball, CanAssumeControl, Man, PlayerCamera},
        foreign::ComponentColor,
        ours::{PlayerColor, PlayerName},
    },
    netlib::{
        ClientNetworkingResources, EventToClient, EventToServer, MainServerEndpoint,
        send_outgoing_event_next_tick, send_outgoing_event_now, send_outgoing_event_now_batch,
        setup_incoming_client,
    },
    physics::terrain::TerrainParams,
};

use crate::{
    assets::{FontAssets, ModelAssets},
    camera::LocalCamera,
    game_state::{GameState, NetworkGameState, WorldEntity},
    notification::Notification,
    remote_players::{ApplyNoFrustumCulling, NameLabel, RemotePlayerCamera, RemotePlayerModel},
    terrain::SetupTerrain,
};

pub mod inventory;

#[derive(Component)]
pub struct DespawnOnWorldData;

/// Temporary storage for camera NetEntId until camera is spawned
#[derive(Resource)]
struct PendingCameraId(NetEntId);

#[derive(Resource)]
struct LocalPlayerId(pub PlayerId);

pub struct NetworkingPlugin;
impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(inventory::InventoryNetworkPlugin)
            .add_systems(
                OnEnter(NetworkGameState::ClientConnecting),
                (
                    // Setup the client and immediatly advance the state
                    setup_incoming_client::<EventToClient, EventToServer>,
                    |mut state: ResMut<NextState<NetworkGameState>>| {
                        state.set(NetworkGameState::ClientSendRequestPacket)
                    },
                ),
            )
            // .add_systems(
            //     Update,
            //     (check_connect_button).run_if(in_state(NetworkGameState::MainMenu)),
            // )
            // After sending the first packet, resend it every so often to see if the server comes
            // alive
            .add_systems(
                Update,
                (
                    shared::event::client::drain_incoming_events,
                    receive_world_data,
                )
                    .run_if(
                        in_state(NetworkGameState::ClientSendRequestPacket)
                            .or(in_state(NetworkGameState::ClientConnected)),
                    ),
            )
            .add_systems(
                Update,
                (send_connect_packet)
                    .run_if(on_timer(Duration::from_millis(1000)))
                    .run_if(in_state(NetworkGameState::ClientSendRequestPacket)),
            )
            // Once we are connected, advance normally
            .add_systems(
                Update,
                (
                    // TODO receive new world data at any time?
                    our_client_wants_to_spawn_circle,
                    our_client_wants_to_spawn_man,
                    apply_pending_camera_id,
                )
                    .run_if(in_state(NetworkGameState::ClientConnected)),
            )
            .add_systems(
                FixedUpdate,
                (
                    shared::increment_ticks,
                    on_general_spawn_network_unit,
                    on_begin_controlling_unit,
                )
                    .chain()
                    .run_if(in_state(NetworkGameState::ClientConnected)),
            )
            .add_systems(
                FixedPostUpdate,
                (shared::netlib::flush_outgoing_events::<EventToClient, EventToServer>).run_if(
                    in_state(NetworkGameState::ClientSendRequestPacket)
                        .or(in_state(NetworkGameState::ClientConnected)),
                ),
            )
            //.add_systems(
            //Update,
            //cast_skill_1
            //.run_if(shared::GameAction::Fire1.just_pressed())
            ////.run_if(in_state(ChatState::NotChatting))
            ////.run_if(any_with_component::<Player>),
            //)
            .add_systems(
                FixedUpdate,
                (send_movement_camera)
                    .run_if(in_state(NetworkGameState::ClientConnected))
                    //.run_if(in_state(InputControlState::Freecam))
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (
                    // TODO receive new world data at any time?
                    spawn_networked_unit_forward_local,
                    on_special_unit_spawn_remote_camera,
                    on_special_unit_spawn_ball,
                    on_special_unit_spawn_man,
                    receive_heartbeat,
                    receive_challenge,
                )
                    .run_if(in_state(NetworkGameState::ClientConnected)),
            )
            .add_systems(
                Update,
                send_heartbeat
                    .run_if(on_timer(Duration::from_millis(200)))
                    .run_if(in_state(NetworkGameState::ClientConnected)),
            )
            .add_message::<SpawnUnit2>()
            .add_message::<SpawnCircle>()
            .add_message::<SpawnMan>()
            .insert_resource(LocalLatencyMeasurement { latency: -1.0 });
    }
}

fn spawn_networked_unit_forward_local(
    mut unit_spawns: UDPacketEvent<SpawnUnit2>,
    mut unit_spawn_writer: MessageWriter<SpawnUnit2>,
    //mut commands: Commands,
    //mut meshes: ResMut<Assets<Mesh>>,
    //mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for spawn in unit_spawns.read() {
        // Echo back to server to confirm spawn
        unit_spawn_writer.write(spawn.event.clone());
    }
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct NeedsClientConstruction;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct CurrentThirdPersonControlledUnit;

// Given some unit
fn on_general_spawn_network_unit(
    mut unit_spawns: MessageReader<SpawnUnit2>,
    mut commands: Commands,
) {
    use crate::game_state::WorldEntity;

    for spawn in unit_spawns.read() {
        // Spawn ball with physics
        let entity = spawn.clone().spawn_entity(&mut commands);

        // Add WorldEntity component to balls so they get cleaned up properly
        commands
            .entity(entity)
            .insert(WorldEntity)
            .insert(NeedsClientConstruction);

        info!(
            "Spawned from networked SpawnUnit2, has {} components",
            spawn.components.len()
        );
    }
}

fn on_special_unit_spawn_ball(
    mut commands: Commands,
    mut unit_query: Query<
        (Entity, &NetEntId, &ComponentColor, &Ball),
        With<NeedsClientConstruction>,
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, _ent_id, color, ball) in unit_query.iter_mut() {
        // TODO add ball-specific client setup here
        commands
            .entity(entity)
            .insert((
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: color.0,
                    metallic: 1.0,
                    perceptual_roughness: 0.2,
                    ..default()
                })),
                Mesh3d(meshes.add(Mesh::from(Sphere { radius: ball.0 }))),
            ))
            .remove::<NeedsClientConstruction>();
    }
}

fn on_special_unit_spawn_man(
    mut commands: Commands,
    mut unit_query: Query<(Entity, &NetEntId, &Man), With<NeedsClientConstruction>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, _ent_id, man) in unit_query.iter_mut() {
        // TODO add ball-specific client setup here
        commands
            .entity(entity)
            .insert((
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    metallic: 0.2,
                    perceptual_roughness: 0.4,
                    ..default()
                })),
                Mesh3d(meshes.add(Mesh::from(Cylinder { radius: 1.0, half_height: 2.0 }))),
                CharacterControllerBundle::new(Collider::cylinder(1.0, 4.0), Vec3::NEG_Y * 9.81)
                    .with_movement(45.0, 0.9, 4.0, PI * 0.20),
            ))
            .remove::<NeedsClientConstruction>();
    }
}

fn on_special_unit_spawn_remote_camera(
    mut commands: Commands,
    mut unit_query: Query<
        (Entity, &NetEntId, &PlayerColor, &PlayerCamera, &PlayerName),
        With<NeedsClientConstruction>,
    >,
    model_assets: Res<ModelAssets>,
    font_assets: Res<FontAssets>,
) {
    for (entity, ent_id, player_color, _player_camera, player_name) in unit_query.iter_mut() {
        commands
            .entity(entity)
            .insert(RemotePlayerCamera)
            .insert((
                Visibility::default(),
                InheritedVisibility::default(),
                GlobalTransform::default(),
                ViewVisibility::default(),
            ))
            .remove::<NeedsClientConstruction>()
            .with_children(|parent| {
                // Spawn the remote player camera visuals
                parent.spawn((
                    SceneRoot(model_assets.g_toilet_scene.clone()),
                    Transform::from_xyz(0.0, 0.0, 0.0)
                        .with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                        .with_scale(Vec3::splat(0.5)),
                    RemotePlayerModel,
                    player_color.clone(),
                    ApplyNoFrustumCulling,
                ));
            });

        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Text::new(player_name.name.clone()),
            TextFont {
                font: font_assets.regular.clone(),
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::WHITE),
            NameLabel {
                target_entity: entity,
            },
            MyNetEntParentId(ent_id.0),
        ));
    }
}

fn send_connect_packet(
    sr: Res<ClientNetworkingResources>,
    //args: Res<CliArgs>,
    mse: Res<MainServerEndpoint>,
    config: Res<Config>,
    mut notif: MessageWriter<Notification>,
    local_player: Query<&Transform, With<LocalCamera>>,
) {
    // Use LocalCamera (FreeCam) transform if available, otherwise use default spawn location
    let my_location = local_player
        .single()
        .copied()
        .unwrap_or_else(|_| Transform::from_xyz(0.0, 10.0, 0.0));

    //let name = args.name_override.clone().or(config.name.clone());
    let name = config.name.clone();
    let event = EventToServer::ConnectRequest(ConnectRequest {
        name: name.clone(),
        my_location,
        color_hue: config.player_color_hue,
    });
    notif.write(Notification(format!(
        "Connecting server={} name={name:?}",
        mse.0.addr(),
    )));
    send_outgoing_event_now(&sr.handler, mse.0, &event);
    info!("Sent connection packet to {}", mse.0);
}

fn on_begin_controlling_unit(
    mut commands: Commands,
    mut unit_event: UDPacketEvent<BeginThirdpersonControllingUnit>,
    units: Query<(Entity, &NetEntId), With<CanAssumeControl>>,
    current_thirdperson_unit: Query<(Entity, &NetEntId), With<CurrentThirdPersonControlledUnit>>,
    our_player_id: Res<LocalPlayerId>,
    mut next_control_state: ResMut<NextState<crate::game_state::InputControlState>>,
) {
    for event in unit_event.read() {
        warn!(
            "Received BeginThirdpersonControllingUnit for player {:?}, unit {:?}",
            event.event.player_id, event.event.unit
        );
        if event.event.player_id != our_player_id.0 {
            // Not for us
            warn!(
                "Received BeginThirdpersonControllingUnit for player {:?}, but we are {:?}",
                event.event.player_id, our_player_id.0
            );
            continue;
        }

        let maybe_unit = event.event.unit;

        if let Ok((cur_thirdperson_ent, cur_thirdperson_ent_id)) = current_thirdperson_unit.single()
        {
            if let Some(unit_ent_id) = maybe_unit
                && &unit_ent_id == cur_thirdperson_ent_id
            {
                // Already controlling this unit
                info!(
                    "Already controlling unit {:?}, ignoring BeginThirdpersonControllingUnit",
                    unit_ent_id
                );
                continue;
            }

            // We know its a different unit, so remove the component from this ent
            commands
                .entity(cur_thirdperson_ent)
                .remove::<CurrentThirdPersonControlledUnit>();
        }

        if let Some(unit_ent_id) = maybe_unit {
            // Find the entity with this NetEntId
            for (ent, ent_id) in units.iter() {
                if *ent_id == unit_ent_id {
                    info!("Now controlling unit {:?}", unit_ent_id);
                    commands
                        .entity(ent)
                        .insert(CurrentThirdPersonControlledUnit);
                    next_control_state.set(crate::game_state::InputControlState::ThirdPerson);
                    break;
                }
            }
        } else {
            error!("No unit found locally to control, staying freecam");
            next_control_state.set(crate::game_state::InputControlState::Freecam);
        }
    }
}

//fn build_healthbar(
//s: &mut ChildBuilder,
//meshes: &mut ResMut<Assets<Mesh>>,
//materials: &mut ResMut<Assets<StandardMaterial>>,
//offset: Vec3,
//) {
//let player_id = s.parent_entity();
// spawn their hp bar
//let mut hp_bar = PbrBundle {
//mesh: meshes.add(Mesh::from(Cuboid {
//half_size: Vec3::splat(0.5),
//})),
//material: materials.add(Color::rgb(0.9, 0.3, 0.0)),
//transform: Transform::from_translation(Vec3::new(0.0, 0.4, 0.0) + offset),
//..Default::default()
//};

// make it invisible until it's updated
//hp_bar.transform.scale = Vec3::ZERO;

//s.spawn((hp_bar, crate::network::stats::HPBar(player_id)));
//}

/// This function is called when we receive the world data from the server on first connect
#[allow(unused, clippy::too_many_arguments, clippy::type_complexity)]
fn receive_world_data(
    mut world_data: UDPacketEvent<WorldData2>,
    mut commands: Commands,
    mut notif: MessageWriter<Notification>,
    mut local_player: Query<(Entity, &mut Transform), With<LocalCamera>>,
    mut spawn_units: MessageWriter<SpawnUnit2>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut game_state: ResMut<NextState<NetworkGameState>>,
    asset_server: ResMut<AssetServer>,
    mut terrain_data: ResMut<TerrainParams>,
    ents_to_despawn: Query<Entity, Or<(With<DespawnOnWorldData>, With<WorldEntity>)>>,
    mut msg_terrain_events: MessageWriter<SetupTerrain>,
    mut next_control_state: ResMut<NextState<crate::game_state::InputControlState>>,
) {
    for event in world_data.read() {
        game_state.set(NetworkGameState::ClientConnected);
        commands.insert_resource(LocalPlayerId(event.event.your_player_id));
        // We spawn in freecam
        next_control_state.set(crate::game_state::InputControlState::Freecam);
        info!("Connected to server");

        // Tell the client to update the terrain with the server's seed
        *terrain_data = event.event.terrain_params.clone();
        msg_terrain_events.write(SetupTerrain);

        for ent in ents_to_despawn.iter() {
            commands.entity(ent).despawn();
        }

        let my_id = event.event.your_player_id;
        let my_camera_id = event.event.your_camera_unit_id;

        // Store the camera ID to be applied later when camera is spawned
        commands.insert_resource(PendingCameraId(my_camera_id));

        info!("Received {} units from server", event.event.units.len());
        for unit in &event.event.units {
            if unit.net_ent_id == my_camera_id {
                // Skip our own camera units - they're already set up locally
                info!("  Skipping own unit {:?}", unit.net_ent_id);
            } else {
                info!(
                    "  Processing remote unit {:?} with {} components",
                    unit.net_ent_id,
                    unit.components.len()
                );
                // TOOD do this gracefully?
                for component in &unit.components {
                    if let shared::net_components::NetComponent::Ours(ours) = component {
                        if let shared::net_components::ours::NetComponentOurs::PlayerName(
                            PlayerName { name },
                        ) = ours
                        {
                            notif.write(Notification(format!("Connected: {name}")));
                        }
                    }
                }

                //For any unit that isnt us, spawn it
                spawn_units.write(unit.clone());
            }
        }
    }
}

fn send_heartbeat(
    sr: Res<ClientNetworkingResources>,
    mse: Res<MainServerEndpoint>,
    time: Res<Time>,
) {
    let event = EventToServer::Heartbeat(Heartbeat {
        client_started_time: time.elapsed_secs_f64(),
    });
    send_outgoing_event_now(&sr.handler, mse.0, &event);
}

#[derive(Resource)]
struct LocalLatencyMeasurement {
    pub latency: f64,
}

fn receive_heartbeat(mut heartbeat_events: UDPacketEvent<HeartbeatResponse>, time: Res<Time>, mut latency_res: ResMut<LocalLatencyMeasurement>) {
    for event in heartbeat_events.read() {
        let cur_client_time = time.elapsed_secs_f64();
        let latency = 0.5 * (cur_client_time - event.event.client_started_time);
        latency_res.latency = latency;
    }
}

fn receive_challenge(
    mut heartbeat_challenges: UDPacketEvent<HeartbeatChallenge>,
    local: Res<LocalLatencyMeasurement>,
    sr: Res<ClientNetworkingResources>,
    mse: Res<MainServerEndpoint>,
) {
    for event in heartbeat_challenges.read() {
        let event = EventToServer::HeartbeatChallengeResponse(HeartbeatChallengeResponse {
            server_time: event.event.server_time,
            local_latency_ms: local.latency * 1000.0,
        });
        send_outgoing_event_now(&sr.handler, mse.0, &event);
    }
}

//fn send_interp(
//sr: Res<ServerResources<EventToClient>>,
//mse: Res<MainServerEndpoint>,
//our_transform: Query<&MovementIntention, (With<Player>, Changed<MovementIntention>)>,
//) {
//if let Ok(intent) = our_transform.get_single() {
// TODO add interp for `AttackIntent` here
//let event = EventToServer::ChangeMovement(ChangeMovement::Move2d(intent.0));
//send_event_to_server(&sr.handler, mse.0, &event);
//}
//}

/// Apply pending camera ID to LocalCamera once it's spawned
fn apply_pending_camera_id(
    pending: Option<Res<PendingCameraId>>,
    mut commands: Commands,
    local_cam: Query<Entity, (With<LocalCamera>, Without<PlayerCamera>)>,
) {
    if let Some(pending_id) = pending {
        if let Ok(cam_entity) = local_cam.single() {
            commands
                .entity(cam_entity)
                .insert(pending_id.0)
                .insert(PlayerCamera);

            commands.remove_resource::<PendingCameraId>();
        }
    }
}

fn send_movement_camera(
    sr: Res<ClientNetworkingResources>,
    mse: Res<MainServerEndpoint>,
    our_transform: Query<
        (&Transform, &NetEntId),
        (With<LocalCamera>, With<PlayerCamera>, Changed<Transform>),
    >,
) {
    if let Ok((transform, ent_id)) = our_transform.single() {
        let mut events = vec![];
        events.push(EventToServer::ChangeMovement(ChangeMovement {
            net_ent_id: *ent_id,
            transform: *transform,
        }));

        send_outgoing_event_now_batch(&sr.handler, mse.0, &events);
    }
}

//fn on_disconnect(
//mut dc_info: ERFE<PlayerDisconnected>,
//mut notif: MessageWriter<Notification>,
//mut commands: Commands,
/// TODO what if the server is disconnecting us?
//other_players: Query<(Entity, &NetEntId, &PlayerName), With<OtherPlayer>>,
//) {
//for event in dc_info.read() {
//let disconnected_ent_id = event.event.id;
//for (player_ent, net_ent_id, PlayerName(player_name)) in &other_players {
//if net_ent_id == &disconnected_ent_id {
//notif.send(Notification(format!("{player_name} Disconnected.")));
//commands.entity(player_ent).despawn_recursive();
//}
//}
//info!(?disconnected_ent_id);
//}
//}

//fn on_someone_move(
//mut someone_moved: ERFE<SomeoneMoved>,
//mut other_players: Query<(&NetEntId, &mut Transform, &mut MovementIntention, &mut AttackIntention), With<AnyUnit>>,
///mut other_players: Query<(&NetEntId, &mut Transform, &mut MovementIntention), (With<AnyUnit>, Without<Player>)>,
//) {
//for movement in someone_moved.read() {
//for (ply_net, mut ply_tfm, mut ply_intent, mut ply_attack_intent,) in &mut other_players {
//if &movement.event.id == ply_net {
//match &movement.event.movement {
//ChangeMovement::SetTransform(t) => *ply_tfm = *t,
//ChangeMovement::StandStill => {}
//ChangeMovement::AttackIntent(intent) => {
//*ply_attack_intent = intent.clone();
//}
//ChangeMovement::Move2d(intent) => {
//*ply_intent = MovementIntention(*intent);
//}
//}
//}
//}
//}
//}

//fn go_movement_intents(
//mut other_players: Query<
//(&mut Transform, &MovementIntention),
//(With<AnyUnit>, Without<Player>),
//>,
//time: Res<Time>,
//) {
//for (mut ply_tfm, ply_intent) in &mut other_players {
//ply_tfm.translation +=
//Vec3::new(ply_intent.0.x, 0.0, ply_intent.0.y) * PLAYER_SPEED * time.delta_secs();
//}
//}

//fn on_connect(
//mut c_info: ERFE<SpawnUnit2>,
///mut notif: EventWriter<Notification>,
//mut local_spawn_unit: MessageWriter<SpawnUnit2>,
//) {
//for event in c_info.read() {
///notif.send(Notification(format!("{:?}", event.event)));
//local_spawn_unit.send(event.event.clone());
//}
//}

fn our_client_wants_to_spawn_circle(
    mut ev_sa: MessageReader<SpawnCircle>,
    sr: Res<ClientNetworkingResources>,
    mse: Res<MainServerEndpoint>,
) {
    for thing in ev_sa.read() {
        let event = EventToServer::SpawnCircle(thing.clone());
        info!("Sending spawn circle event to server");
        send_outgoing_event_next_tick(&sr, mse.0, &event);
    }
}

fn our_client_wants_to_spawn_man(
    mut ev_sa: MessageReader<SpawnMan>,
    sr: Res<ClientNetworkingResources>,
    mse: Res<MainServerEndpoint>,
) {
    for thing in ev_sa.read() {
        let event = EventToServer::SpawnMan(thing.clone());
        info!("Sending spawn man event to server");
        send_outgoing_event_next_tick(&sr, mse.0, &event);
    }
}

//fn cast_skill_1(
//keyboard_input: Res<ButtonInput<KeyCode>>,
//config: Res<Config>,
//player: Query<&Transform, With<Player>>,
//aim_dir: Query<&ClientAimDirection>,
//mut ev_sa: EventWriter<StartLocalAnimation>,
//) {
//if config.pressed(&keyboard_input, shared::GameAction::Mod1) {
//let event = Cast::Buff;
//ev_sa.send(StartLocalAnimation(event));
//} else {
//let transform = player.single();

//let aim_dir = aim_dir.single().0;

//let target = transform.translation
//+ Vec3 {
//x: aim_dir.cos(),
//y: 0.0,
//z: -aim_dir.sin(),
//};

//let shooting_data = ShootingData {
//shot_from: transform.translation,
//target,
//};
//let event = Cast::Shoot(shooting_data);
//ev_sa.send(StartLocalAnimation(event));
//}
//}
