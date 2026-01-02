use axum::Router;
use bevy::{prelude::*, time::common_conditions::on_real_timer};
use shared::{event::PlayerId, net_components::ours::PlayerName, netlib::ServerNetworkingResources};

use crate::{ConnectedPlayer, ServerState, TokioRuntimeResource};

pub struct AxumServerPlugin;

impl Plugin for AxumServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(ServerState::Running),
            setup_shared_axum_server,
        );
        app.add_systems(
            Update,
            (
                respond_to_get_players_request,
            ).run_if(in_state(ServerState::Running)).run_if(on_real_timer(std::time::Duration::from_millis(100)))
        );

        app.insert_resource(AxumServerResource::default());
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AxumRequestId(pub u64);

#[derive(Message, Debug, Clone)]
pub struct BevyAxumRequestGetPlayers;

#[derive(Message)]
pub struct BevyAxumReplyGetPlayers {
    players: Vec<String>,
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
    server_networking_resources: ServerNetworkingResources,
    axum_bevy_queues: AxumServerResource,
}

fn setup_shared_axum_server(
    res: Res<ServerNetworkingResources>,
    axum_server_res: Res<AxumServerResource>,
    tokio_runtime: Res<TokioRuntimeResource>,
) {
    let (ip, port) = res.con_str.as_ref().clone();
    info!("Starting shared axum server on {}:{}", ip, port);

    let state = AxumState {
        server_networking_resources: res.clone(),
        axum_bevy_queues: axum_server_res.clone(),
    };

    let router = Router::new()
        .route("/", axum::routing::get(|| async { "Hello, World!" }))
        .route("/players", axum::routing::get(get_players_endpoint))
        .with_state(state);

    let _x = tokio_runtime.spawn(async move {
        info!("Axum server task started.");
        for i in 0..5 {
            let listener = tokio::net::TcpListener::bind(format!("{}:{}", ip, port)).await.unwrap();
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
    state.axum_bevy_queues.get_players_requests.insert(
        req,
        BevyAxumRequestGetPlayers,
    );

    // Wait for reply (in a real implementation, consider using a more robust method)
    loop {
        if let Some(reply) = state.axum_bevy_queues.get_players_replys.remove(&req) {
            return axum::response::Json(serde_json::json!({
                "players": reply.1.players
            }));
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

fn respond_to_get_players_request(
    mut connected_player_query: Query<(&PlayerId, &PlayerName), With<ConnectedPlayer>>,
) {


}
