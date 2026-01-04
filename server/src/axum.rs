use axum::Router;
use bevy::{prelude::*, time::common_conditions::on_real_timer};
use shared::{
    event::PlayerId, net_components::ours::PlayerName, netlib::ServerNetworkingResources,
};

use crate::{ConnectedPlayer, ServerState, TokioRuntimeResource};

pub struct AxumServerPlugin;

impl Plugin for AxumServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(ServerState::Running), setup_shared_axum_server);
        app.add_systems(
            Update,
            (respond_to_get_players_request,)
                .run_if(in_state(ServerState::Running))
                .run_if(on_real_timer(std::time::Duration::from_millis(100))),
        );

        app.insert_resource(AxumServerResource::default());
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AxumRequestId(pub u64);

#[derive(Debug, Clone)]
pub struct BevyAxumRequestGetPlayers;

use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    // serailze as number doesnt work here because u64
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BevyAxumReplyGetPlayers {
    error: bool,
    players: Vec<PlayerInfo>,
}

use dashmap::DashMap;
use std::sync::Arc;

#[derive(Resource, Clone, Default)]
pub struct AxumServerResource {
    pub get_players_requests: Arc<DashMap<AxumRequestId, BevyAxumRequestGetPlayers>>,
    pub get_players_replys: Arc<DashMap<AxumRequestId, BevyAxumReplyGetPlayers>>,
}

#[derive(Clone)]
struct AxumState {
    _server_networking_resources: ServerNetworkingResources,
    axum_bevy_queues: AxumServerResource,
}

fn setup_shared_axum_server(
    res: Res<ServerNetworkingResources>,
    axum_server_res: Res<AxumServerResource>,
    tokio_runtime: Res<TokioRuntimeResource>,
) {
    let (ip, port) = res.con_str.as_ref().clone();
    info!("Starting shared axum server on {}:{}", ip, port + 1);

    let state = AxumState {
        _server_networking_resources: res.clone(),
        axum_bevy_queues: axum_server_res.clone(),
    };

    let router = Router::new()
        .route("/", axum::routing::get(|| async { "Hello, World!" }))
        .route("/players", axum::routing::get(get_players_endpoint))
        .with_state(state);

    let _x = tokio_runtime.spawn(async move {
        info!("Axum server task started.");
        for i in 0..5 {
            let listener = tokio::net::TcpListener::bind(format!("{}:{}", ip, port + 1))
                .await
                .unwrap();
            let server = axum::serve(listener, router.clone());
            // should run forever unless error
            let err = server.await.unwrap_err();
            error!("Axum server error on attempt {}: {}", i + 1, err);
        }

        error!("Axum server died.");
    });
}

async fn get_players_endpoint(
    state: axum::extract::State<AxumState>,
) -> impl axum::response::IntoResponse {
    let req = AxumRequestId(rand::random());
    state
        .axum_bevy_queues
        .get_players_requests
        .insert(req, BevyAxumRequestGetPlayers);

    for _ in 0..100 {
        if let Some(reply) = state.axum_bevy_queues.get_players_replys.remove(&req) {
            return axum::Json(reply.1);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    warn!("Get players reply timed out for request: {:?}", req);
    axum::Json(BevyAxumReplyGetPlayers {
        players: vec![],
        error: true,
    })
}

fn respond_to_get_players_request(
    connected_player_query: Query<(&PlayerId, &PlayerName), With<ConnectedPlayer>>,
    axum_server_res: Res<AxumServerResource>,
) {
    let mut key_to_remove = vec![];
    for request in axum_server_res.get_players_requests.iter() {
        let mut reply = BevyAxumReplyGetPlayers {
            error: false,
            players: vec![],
        };
        for (player_id, player_name) in connected_player_query.iter() {
            reply.players.push(PlayerInfo {
                id: player_id.0.to_string(),
                name: player_name.name.clone(),
            });
        }

        axum_server_res
            .get_players_replys
            .insert(*request.key(), reply);

        key_to_remove.push(*request.key());
    }
    for key in key_to_remove {
        axum_server_res.get_players_requests.remove(&key);
    }
}
