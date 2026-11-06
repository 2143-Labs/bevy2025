use axum::Router;
use bevy::{prelude::*, time::common_conditions::on_real_timer};
use shared::{
    event::PlayerId, net_components::ours::PlayerName, netlib::ServerNetworkingResources,
    tokio_udp::TokioRuntimeResource,
};

use subtle::ConstantTimeEq;

use crate::{ConnectedPlayer, GameServerCoordinator, ServerState};

pub struct AxumServerPlugin;

impl Plugin for AxumServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(ServerState::Running), setup_shared_axum_server);
        // Ungated: a paused or restoring world still runs frames.
        app.add_systems(First, stamp_frame);
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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Resource, Clone, Default)]
pub struct AxumServerResource {
    pub get_players_requests: Arc<DashMap<AxumRequestId, BevyAxumRequestGetPlayers>>,
    pub get_players_replys: Arc<DashMap<AxumRequestId, BevyAxumReplyGetPlayers>>,
    /// Unix seconds of the last frame, read by `/healthz`.
    pub last_frame: Arc<AtomicU64>,
    /// Set by `POST /activate`: the coordinator has claimed this warm-pool instance, so its world
    /// may come online. Ungated on `/healthz` — a pre-game instance answers like any other.
    pub activated: Arc<AtomicBool>,
}

/// `/healthz` fails once the game loop has not run a frame for this long, so a hung loop
/// (heartbeats stop, the tokio side still answers) is probed dead by the authserver and
/// restarted by the kubelet's liveness probe.
const FRAME_STALL_SECS: u64 = 10;

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn stamp_frame(axum_server_res: Res<AxumServerResource>) {
    axum_server_res
        .last_frame
        .store(unix_secs(), Ordering::Relaxed);
}

fn loop_alive(last_frame: u64, now: u64) -> bool {
    now.saturating_sub(last_frame) < FRAME_STALL_SECS
}

#[derive(Clone)]
struct AxumState {
    _server_networking_resources: ServerNetworkingResources,
    axum_bevy_queues: AxumServerResource,
    /// Characters the coordinator moved to another tutorial, drained by `evict_characters`.
    evictions: crate::Evictions,
    /// The token the coordinator link was given; `None` on a standalone server, which nothing can
    /// activate.
    coordinator_token: Option<String>,
}

fn setup_shared_axum_server(
    res: Res<ServerNetworkingResources>,
    axum_server_res: Res<AxumServerResource>,
    evictions: Res<crate::Evictions>,
    tokio_runtime: Res<TokioRuntimeResource>,
    coordinator: Option<Res<GameServerCoordinator>>,
) {
    let (ip, port) = res.con_str.as_ref().clone();
    info!("Starting shared axum server on {}:{}", ip, port + 1);

    let state = AxumState {
        _server_networking_resources: res.clone(),
        axum_bevy_queues: axum_server_res.clone(),
        evictions: evictions.clone(),
        coordinator_token: coordinator.map(|c| c.token.clone()),
    };

    let router = Router::new()
        .route("/gshealthz", axum::routing::get(|| async { "OK" }))
        .route("/healthz", axum::routing::get(healthz))
        .route("/", axum::routing::get(|| async { "Hello, World!" }))
        .route("/players", axum::routing::get(get_players_endpoint))
        .route("/activate", axum::routing::post(activate))
        .route("/evict", axum::routing::post(evict))
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

async fn healthz(state: axum::extract::State<AxumState>) -> (axum::http::StatusCode, &'static str) {
    let last = state.axum_bevy_queues.last_frame.load(Ordering::Relaxed);
    if loop_alive(last, unix_secs()) {
        (axum::http::StatusCode::OK, "OK")
    } else {
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "game loop stalled",
        )
    }
}

/// The coordinator claims this pool instance. Idempotent: a world already online is left alone.
async fn activate(
    state: axum::extract::State<AxumState>,
    headers: axum::http::HeaderMap,
) -> axum::http::StatusCode {
    let presented = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or("");
    if !token_ok(state.coordinator_token.as_deref().unwrap_or(""), presented) {
        return axum::http::StatusCode::UNAUTHORIZED;
    }
    state
        .axum_bevy_queues
        .activated
        .store(true, Ordering::Relaxed);
    info!("activate: the coordinator has claimed this instance");
    axum::http::StatusCode::NO_CONTENT
}

/// The character moved to another tutorial: queue it for `evict_characters`, which takes its
/// units out of the world and tells the other clients.
async fn evict(
    state: axum::extract::State<AxumState>,
    headers: axum::http::HeaderMap,
    axum::Json(body): axum::Json<EvictBody>,
) -> axum::http::StatusCode {
    let presented = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or("");
    if !token_ok(state.coordinator_token.as_deref().unwrap_or(""), presented) {
        return axum::http::StatusCode::UNAUTHORIZED;
    }
    // Idempotent: an unknown character is dropped by `evict_characters`.
    state
        .evictions
        .0
        .lock()
        .expect("evictions lock poisoned")
        .push(body.character_id);
    info!(
        character_id = body.character_id,
        "evict: the character moved to another tutorial"
    );
    axum::http::StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct EvictBody {
    character_id: i64,
}

/// Empty expected token: no activation is accepted (a standalone server has no coordinator).
fn token_ok(expected: &str, presented: &str) -> bool {
    !expected.is_empty() && bool::from(presented.as_bytes().ct_eq(expected.as_bytes()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_needs_the_coordinator_token() {
        // A standalone server has no coordinator: nothing can activate it.
        assert!(!token_ok("", ""));
        assert!(!token_ok("", "anything"));
        assert!(!token_ok("sekrit-token", ""));
        assert!(!token_ok("sekrit-token", "sekrit-toke"));
        assert!(!token_ok("sekrit-token", "sekrit-tokenx"));
        assert!(token_ok("sekrit-token", "sekrit-token"));
    }

    #[test]
    fn healthz_fails_once_the_loop_stalls() {
        assert!(loop_alive(100, 100));
        assert!(loop_alive(100, 100 + FRAME_STALL_SECS - 1));
        assert!(!loop_alive(100, 100 + FRAME_STALL_SECS));
        // Never stamped yet (no frame since start) counts as stalled.
        assert!(!loop_alive(0, 1_700_000_000));
    }
}
