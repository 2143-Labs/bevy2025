use std::{net::SocketAddr, time::Duration};

use bevy::{prelude::*, time::common_conditions::on_timer};
use shared::{
    event::{
        client::{SpawnUnit2, WorldData2}, server::{ChangeMovement, ConnectRequest, Heartbeat, SpawnCircle}, NetEntId, ERFE
    }, net_components::{ents::PlayerCamera, ours::PlayerName}, netlib::{
        send_event_to_server, send_event_to_server_batch, setup_client, ClientNetworkingResources, EventToClient, EventToServer, MainServerEndpoint, NetworkConnectionTarget, NetworkingResources
    }, physics::terrain::TerrainParams, Config
};

use crate::{camera::FreeCam, game_state::NetworkGameState, notification::Notification, terrain::SetupTerrain};

#[derive(Component)]
pub struct DespawnOnWorldData;

pub struct NetworkingPlugin;
impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        shared::event::client::register_events(app);
        app.add_message::<SpawnUnit2>()
            .add_systems(
                Startup,
                (|mut commands: Commands, config: Res<Config>| {
                    // Setup networking resources
                    commands.insert_resource(NetworkConnectionTarget {
                        ip: config.ip.clone(),
                        port: config.port,
                    });
                },
                |mut state: ResMut<NextState<NetworkGameState>>| {
                    state.set(NetworkGameState::ClientConnecting)
                }),
            )
            .add_systems(
                OnEnter(NetworkGameState::ClientConnecting),
                (
                    // Setup the client and immediatly advance the state
                    setup_client::<EventToClient>,
                    |mut state: ResMut<NextState<NetworkGameState>>| {
                        state.set(NetworkGameState::ClientSendRequestPacket)
                    },
                ),
            )
            .add_systems(
                Update,
                (check_connect_button).run_if(in_state(NetworkGameState::MainMenu)),
            )
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
                    spawn_circle,
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
            Update,
                (send_movement)
                .run_if(on_timer(Duration::from_millis(25)))
                .run_if(in_state(NetworkGameState::ClientConnected)),
            )
            .add_systems(
                Update,
                send_heartbeat
                    .run_if(on_timer(Duration::from_millis(200)))
                    .run_if(in_state(NetworkGameState::ClientConnected)),
            )
            .add_message::<SpawnCircle>();
    }
}

fn send_connect_packet(
    sr: Res<NetworkingResources<EventToClient>>,
    //args: Res<CliArgs>,
    mse: Res<MainServerEndpoint>,
    config: Res<Config>,
    mut notif: MessageWriter<Notification>,
    local_player: Query<&Transform, With<FreeCam>>,
) {
    let Ok(&my_location) = local_player.single() else {
        return;
    };
    //let name = args.name_override.clone().or(config.name.clone());
    let name = config.name.clone();
    let event = EventToServer::ConnectRequest(ConnectRequest {
        name: name.clone(),
        my_location,
    });
    notif.write(Notification(format!(
        "Connecting server={} name={name:?}",
        mse.0.addr(),
    )));
    send_event_to_server(&sr.handler, mse.0, &event);
    info!("Sent connection packet to {}", mse.0);
}

//fn build_healthbar(
//s: &mut ChildBuilder,
//meshes: &mut ResMut<Assets<Mesh>>,
//materials: &mut ResMut<Assets<StandardMaterial>>,
//offset: Vec3,
//) {
//let player_id = s.parent_entity();
//// spawn their hp bar
//let mut hp_bar = PbrBundle {
//mesh: meshes.add(Mesh::from(Cuboid {
//half_size: Vec3::splat(0.5),
//})),
//material: materials.add(Color::rgb(0.9, 0.3, 0.0)),
//transform: Transform::from_translation(Vec3::new(0.0, 0.4, 0.0) + offset),
//..Default::default()
//};

//// make it invisible until it's updated
//hp_bar.transform.scale = Vec3::ZERO;

//s.spawn((hp_bar, crate::network::stats::HPBar(player_id)));
//}

#[allow(unused)]
fn receive_world_data(
    mut world_data: ERFE<WorldData2>,
    mut commands: Commands,
    mut notif: MessageWriter<Notification>,
    mut local_player: Query<(Entity, &mut Transform), With<FreeCam>>,
    mut spawn_units: MessageWriter<SpawnUnit2>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut game_state: ResMut<NextState<NetworkGameState>>,
    asset_server: ResMut<AssetServer>,
    mut terrain_data: ResMut<TerrainParams>,
    ents_to_despawn: Query<Entity, With<DespawnOnWorldData>>,
    mut msg_terrain_events: MessageWriter<SetupTerrain>,
) {
    for event in world_data.read() {
        game_state.set(NetworkGameState::ClientConnected);
        info!(?event, "Server has returned world data!");

        // Tell the client to update the terrain with the server's seed
        *terrain_data = event.event.terrain_params.clone();
        msg_terrain_events.write(SetupTerrain);
        info!(?terrain_data, "Updated terrain params from server.");

        for ent in ents_to_despawn.iter() {
            commands.entity(ent).despawn();
        }

        let my_id = event.event.your_unit_id;
        for unit in &event.event.units {
            if unit.net_ent_id == my_id {
                // If so, start aligning the client to it
                let (p_ent, mut p_tfm) = local_player.single_mut().unwrap();
                //TODO this again
                //p_tfm.translation = unit.transform.translation;

                //notif.send(Notification(format!(
                //"Connected to server as {name} {my_id:?}"
                //)));

                //// Add our netentid + name
                //commands
                //.entity(p_ent)
                //.insert(my_id)
                //.insert(PlayerName(name.clone()))
                //.insert(unit.health)
                //.with_children(|s| {
                //build_healthbar(s, &mut meshes, &mut materials, Vec3::ZERO)
                //});

                // if this is us, skip the spawn units call cause we updated a local unit
                // instead. TODO eventually fix this so when we fully despawn the menu
                // player unit
            } else {
                // TOOD do this gracefully?
                for component in &unit.components {
                    match component {
                        shared::net_components::NetComponent::Ours(ours) => match ours {
                            shared::net_components::ours::NetComponentOurs::PlayerName(
                                PlayerName { name },
                            ) => {
                                notif.write(Notification(format!("Connected: {name}")));
                            }
                            _ => {}
                        },
                        _ => {}
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
    send_event_to_server(&sr.handler, mse.0, &event);
}

//fn send_interp(
//sr: Res<ServerResources<EventToClient>>,
//mse: Res<MainServerEndpoint>,
//our_transform: Query<&MovementIntention, (With<Player>, Changed<MovementIntention>)>,
//) {
//if let Ok(intent) = our_transform.get_single() {
//// TODO add interp for `AttackIntent` here
//let event = EventToServer::ChangeMovement(ChangeMovement::Move2d(intent.0));
//send_event_to_server(&sr.handler, mse.0, &event);
//}
//}

fn send_movement(
    sr: Res<ClientNetworkingResources>,
    mse: Res<MainServerEndpoint>,
    our_transform: Query<
        (&Transform, &NetEntId),
        (With<PlayerCamera>, Changed<Transform>),
    >,
) {
    if let Ok((transform, ent_id)) = our_transform.single() {
        let mut events = vec![];
        events.push(EventToServer::ChangeMovement(ChangeMovement {
            net_ent_id: *ent_id,
            transform: *transform,
        }));

        send_event_to_server_batch(&sr.handler, mse.0, &events);
    }
}

//fn on_disconnect(
//mut dc_info: ERFE<PlayerDisconnected>,
//mut notif: MessageWriter<Notification>,
//mut commands: Commands,
//// TODO what if the server is disconnecting us?
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
////mut other_players: Query<(&NetEntId, &mut Transform, &mut MovementIntention), (With<AnyUnit>, Without<Player>)>,
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
////mut notif: EventWriter<Notification>,
//mut local_spawn_unit: MessageWriter<SpawnUnit2>,
//) {
//for event in c_info.read() {
////notif.send(Notification(format!("{:?}", event.event)));
//local_spawn_unit.send(event.event.clone());
//}
//}

pub fn check_connect_button(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    //args: Res<CliArgs>,
    config: Res<Config>,
    mut commands: Commands,
    mut game_state: ResMut<NextState<NetworkGameState>>,
) {
    if !config.just_pressed(&keyboard_input, shared::GameAction::Use) {
        return;
    }

    warn!("Connecting to server due to user pressing connect button!");

    let target = {
        // Split this into ip and port and then connect
        let addr: SocketAddr = "127.0.0.1:22143"
            .parse()
            .expect("--autoconnect was given an invalid ip and port to connect to");

        NetworkConnectionTarget {
            ip: addr.ip().to_string(),
            port: addr.port(),
        }
    };

    info!(
        ?target,
        "Using --autoconnect command line argument to setup connection."
    );

    commands.insert_resource(target);
    game_state.set(NetworkGameState::ClientConnecting);
}

fn spawn_circle(
    mut ev_sa: MessageReader<SpawnCircle>,
    sr: Res<ClientNetworkingResources>,
    mse: Res<MainServerEndpoint>,
) {
    for thing in ev_sa.read() {
        let event = EventToServer::SpawnCircle(thing.clone());
        info!("Sending spawn circle event to server");
        send_event_to_server(&sr.handler, mse.0, &event);
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
