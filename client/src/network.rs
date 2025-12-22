use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};
use shared::{
    Config,
    event::{
        ERFE, MyNetEntParentId, NetEntId,
        client::{SpawnUnit2, WorldData2},
        server::{ChangeMovement, ConnectRequest, Heartbeat, SpawnCircle, SpawnMan},
    },
    net_components::{
        ents::PlayerCamera,
        groups::SpecialConstructor,
        ours::{PlayerColor, PlayerName},
    },
    netlib::{
        ClientNetworkingResources, EventToClient, EventToServer, MainServerEndpoint,
        NetworkingResources, send_event_to_server_now, send_event_to_server_now_batch,
        setup_client,
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

#[derive(Component)]
pub struct DespawnOnWorldData;

/// Temporary storage for camera NetEntId until camera is spawned
#[derive(Resource)]
struct PendingCameraId(NetEntId);

pub struct NetworkingPlugin;
impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        shared::event::client::register_events(app);
        app.add_systems(
            OnEnter(NetworkGameState::ClientConnecting),
            (
                // Setup the client and immediatly advance the state
                setup_client::<EventToClient>,
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
            (shared::event::client::drain_events, receive_world_data).run_if(
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
                .chain()
                .run_if(in_state(NetworkGameState::ClientConnected))
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(
            Update,
            (
                // TODO receive new world data at any time?
                spawn_networked_unit_forward_local,
                on_general_spawn_network_unit,
                on_special_unit_spawn_remote_camera,
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
        .add_message::<SpawnMan>();
    }
}

fn spawn_networked_unit_forward_local(
    mut unit_spawns: ERFE<SpawnUnit2>,
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

// Given some unit
fn on_general_spawn_network_unit(
    mut unit_spawns: MessageReader<SpawnUnit2>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    //model_assets: Res<ModelAssets>,
    //font_assets: Res<FontAssets>,
    //mut notif: MessageWriter<Notification>,
) {
    use crate::game_state::WorldEntity;

    for spawn in unit_spawns.read() {
        // Spawn ball with physics
        let entity = spawn
            .clone()
            .spawn_entity_client(&mut commands, &mut meshes, &mut materials);

        // Add WorldEntity component to balls so they get cleaned up properly
        commands.entity(entity).insert(WorldEntity);

        info!(
            "Spawned from networked SpawnUnit2, has {} components",
            spawn.components.len()
        );
    }
}

fn on_special_unit_spawn_remote_camera(
    mut commands: Commands,
    mut unit_query: Query<
        (Entity, &NetEntId, &PlayerColor, &PlayerCamera),
        With<SpecialConstructor>,
    >,
    model_assets: Res<ModelAssets>,
    font_assets: Res<FontAssets>,
    notif: MessageWriter<Notification>,
) {
    for (entity, ent_id, player_color, _player_camera) in unit_query.iter_mut() {
        commands
            .entity(entity)
            .insert(RemotePlayerCamera)
            .insert((
                Visibility::default(),
                InheritedVisibility::default(),
                GlobalTransform::default(),
                ViewVisibility::default(),
            ))
            .remove::<SpecialConstructor>()
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
            Text::new("Test"),
            TextFont {
                font: font_assets.regular.clone(),
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::WHITE),
            NameLabel,
            MyNetEntParentId(ent_id.0),
        ));
    }
}

fn send_connect_packet(
    sr: Res<NetworkingResources<EventToClient>>,
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
    send_event_to_server_now(&sr.handler, mse.0, &event);
    info!("Sent connection packet to {}", mse.0);
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
    mut world_data: ERFE<WorldData2>,
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
) {
    for event in world_data.read() {
        game_state.set(NetworkGameState::ClientConnected);
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
                // Skip our own player and camera units - they're already set up locally
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

        //commands.spawn((
        //HPIndicator::HP,
        //TextBundle::from_section(
        //"HP: #",
        //TextStyle {
        //font: asset_server.load("fonts/ttf/JetBrainsMono-Regular.ttf"),
        //font_size: 45.0,
        //color: Color::rgb(0.4, 0.5, 0.75),
        //},
        //)
        //.with_text_justify(JustifyText::Center)
        //.with_style(Style {
        //position_type: PositionType::Absolute,
        //right: Val::Px(10.0),
        //bottom: Val::Px(10.0),
        //..default()
        //}),
        //));
        //commands.spawn((
        //HPIndicator::Deaths,
        //TextBundle::from_section(
        //"",
        //TextStyle {
        //font: asset_server.load("fonts/ttf/JetBrainsMono-Regular.ttf"),
        //font_size: 45.0,
        //color: Color::rgb(0.9, 0.2, 0.2),
        //},
        //)
        //.with_text_justify(JustifyText::Center)
        //.with_style(Style {
        //position_type: PositionType::Absolute,
        //right: Val::Px(10.0),
        //bottom: Val::Px(50.0),
        //..default()
        //}),
        //));
    }
}

fn send_heartbeat(sr: Res<ClientNetworkingResources>, mse: Res<MainServerEndpoint>) {
    let event = EventToServer::Heartbeat(Heartbeat {});
    send_event_to_server_now(&sr.handler, mse.0, &event);
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

        send_event_to_server_now_batch(&sr.handler, mse.0, &events);
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
        send_event_to_server_now(&sr.handler, mse.0, &event);
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
        send_event_to_server_now(&sr.handler, mse.0, &event);
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
