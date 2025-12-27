use crate::game_state::OverlayMenuState;
use super::styles::*;

use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};
use shared::{
    event::{UDPacketEvent, client::RequestScoreboardResponse},
    netlib::{ClientNetworkingResources, MainServerEndpoint, send_outgoing_event_now},
};

/// Marker for the paused menu root entity
#[derive(Component)]
pub struct ScoreboardMenu;

#[derive(Component)]
pub struct ScoreboardPlayerContainer;

#[derive(Component)]
pub struct ScoreboardPlayerEntry;

#[derive(Component)]
pub struct ScoreboardMenuLoading;

pub struct ScoreboardMenuPlugin;

impl Plugin for ScoreboardMenuPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(
                OnEnter(OverlayMenuState::Scoreboard),
                (
                    spawn_scoreboard_menu,
                    send_scoreboard_request_packet,
                ),
            )
            .add_systems(
                OnExit(OverlayMenuState::Scoreboard),
                despawn_scoreboard_menu,
            )
            .add_systems(
                Update,
                (
                    handle_scoreboard_menu_buttons,
                    handle_scoreboard_data_packet,
                    update_scoreboard_menu,
                )
                    .run_if(in_state(OverlayMenuState::Scoreboard)),
            )
            .add_systems(
                Update,
                send_scoreboard_request_packet.run_if(
                    in_state(OverlayMenuState::Scoreboard)
                        .and(on_timer(Duration::from_millis(100))),
                ),
            )
            .add_message::<RequestScoreboardResponse>();
    }
}

// send a packet and spawn loading screen
pub fn spawn_scoreboard_menu(mut commands: Commands) {
    info!("Spawning scoreboard menu");
    // spawn loading...
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..Default::default()
            },
            ScoreboardMenuLoading,
        ))
        .with_children(|parent| {
            parent.spawn({
                let (text, font, color) = heading_text("Loading Scoreboard...", 50.0);
                (
                    Node {
                        ..Default::default()
                    },
                    text,
                    font,
                    color,
                )
            });
        });
}

pub fn send_scoreboard_request_packet(
    sr: Res<ClientNetworkingResources>,
    mse: Res<MainServerEndpoint>,
) {
    // send packet
    let event = shared::event::server::RequestScoreboard {};
    send_outgoing_event_now(
        &sr.handler,
        mse.0,
        &shared::netlib::EventToServer::RequestScoreboard(event),
    );
}

pub fn update_scoreboard_menu(
    mut mreader: MessageReader<RequestScoreboardResponse>,
    mut commands: Commands,
    scoreboard_player_container_query: Query<Entity, With<ScoreboardPlayerContainer>>,
) {
    let Some(scoreboard_data) = mreader.read().last() else {
        return;
    };

    //clear the existing entries,
    let Ok(container_entity) = scoreboard_player_container_query.single() else {
        error!("Could not find scoreboard player container entity");
        return;
    };

    commands.entity(container_entity).despawn_children();

    for (player_id, player_name) in &scoreboard_data.player_names {
        let ping = scoreboard_data
            .player_pings
            .get(&player_id)
            .cloned()
            .unwrap();
        let is_local_player = false; // TODO: determine if this is the local player

        let (_bg_color, _border_color) = if is_local_player {
            (
                Color::srgba(0.2, 0.6, 0.2, 0.8),
                Color::srgba(0.1, 0.5, 0.1, 1.0),
            )
        } else {
            (
                Color::srgba(0.2, 0.2, 0.2, 0.8),
                Color::srgba(0.1, 0.1, 0.1, 1.0),
            )
        };

        commands.entity(container_entity).with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(50.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(10.0)),
                        ..Default::default()
                    },
                    ScoreboardPlayerEntry,
                ))
                .with_children(|entry_parent| {
                    // Player name text
                    entry_parent.spawn({
                        let (text, font, color) = menu_button_text(&format!("{player_name}"));
                        (
                            Node {
                                ..Default::default()
                            },
                            text,
                            font,
                            color,
                        )
                    });
                    // Ping text
                    entry_parent.spawn({
                        let (text, font, color) = menu_button_text(&format!(
                            "Ping: {} cl ms / {} sv ms",
                            ping.server_challenged_ping_ms, ping.client_reported_ping_ms
                        ));
                        (
                            Node {
                                ..Default::default()
                            },
                            text,
                            font,
                            color,
                        )
                    });
                });

            info!(
                "Added scoreboard entry for player: {} with ping: {:?}",
                player_name, ping
            );
        });
    }
}

pub fn handle_scoreboard_data_packet(
    mut packets: UDPacketEvent<RequestScoreboardResponse>,
    mut commands: Commands,
    mut mwriter: MessageWriter<RequestScoreboardResponse>,
    menu_loading_query: Query<Entity, With<ScoreboardMenuLoading>>,
) {
    for packet in packets.read() {
        info!("Received scoreboard data packet: {:?}", packet.event);
        // despawn loading screen
        if let Ok(loading_entity) = menu_loading_query.single() {
            commands.entity(loading_entity).despawn();
            // this means we also need to spawn the new scoreboard element
            spawn_scoreboard_menu_base(&mut commands);
        }

        mwriter.write(packet.event.clone());
    }
}

/// The scoreboard is a large box in the center of the screen with a list of players and their
/// pings. Other players have a gray background, while the local player has a highlighted
/// background.
fn spawn_scoreboard_menu_base(commands: &mut Commands) {
    // spawn main container
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(20.0),
                ..Default::default()
            },
            ScoreboardMenu,
        ))
        .with_children(|parent| {
            // Title
            parent.spawn({
                let (text, font, color) = heading_text("SCOREBOARD", 80.0);
                (
                    Node {
                        margin: UiRect::bottom(Val::Px(40.0)),
                        ..Default::default()
                    },
                    text,
                    font,
                    color,
                )
            });
            // Player container
            parent.spawn((
                Node {
                    width: Val::Percent(80.0),
                    height: Val::Percent(60.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Stretch,
                    row_gap: Val::Px(10.0),
                    ..Default::default()
                },
                ScoreboardPlayerContainer,
            ));
        });
}

pub fn despawn_scoreboard_menu(
    mut commands: Commands,
    menu_query: Query<Entity, 
    Or<(With<ScoreboardMenu>, With<ScoreboardMenuLoading>)>>,
) {
    info!("Despawning scoreboard menu");
    if let Ok(menu_entity) = menu_query.single() {
        commands.entity(menu_entity).despawn();
    }
}

pub fn handle_scoreboard_menu_buttons() {}
