//! The game server's link to the authserver's coordinator, over the authserver's internal API.
//!
//! Joins are gated: a connect request is only admitted once the coordinator says the character may
//! play on this slot (which also proves the ticket's login session still exists). Leaves and
//! heartbeats are fire-and-forget and retried until the authserver accepts them.
//!
//! Ordering is load-bearing. One task sends every op, one at a time, in the order the server
//! produced it, because the coordinator removes a slot's players by diffing heartbeat rosters: a
//! heartbeat that overtook a join would report the player who had just joined as gone. At most
//! one heartbeat waits behind the one in flight (see `CoordinatorLink::heartbeat`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bevy::prelude::*;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use shared::event::PlayerId;
use tokio::sync::{mpsc, oneshot};

use crate::{
    ConnectedPlayer, DisconnectedPlayer, GameServerCoordinator, PendingJoins, PlayerSid,
    ReadyToAdmit, ServerState,
};

/// How often the roster is reported. The coordinator treats a slot with no report for
/// `COORDINATOR_STALE_SECS` (30 s by default) as dead and forgets its players.
pub const HEARTBEAT_EVERY: Duration = Duration::from_secs(10);

/// How long a connect request waits for the coordinator before the client is rejected. The client
/// resends `ConnectRequest` while it waits, so this only has to cover a slow authserver.
pub const JOIN_GIVE_UP: Duration = Duration::from_secs(6);

/// Longest pause between retries of a heartbeat or leave. Retries must stay below `JOIN_GIVE_UP`:
/// the queue is strictly serial, so a sleeping heartbeat would hold a queued join past its
/// deadline and reject a player whose coordinator was reachable.
const MAX_RETRY_DELAY: Duration = Duration::from_secs(4);

/// One player as the coordinator knows them: the character id (the ticket's `sub`) and the login
/// session id (`sid`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlayerRef {
    pub character_id: i64,
    pub sid: String,
}

/// Everything the link sends, in production order.
pub enum LinkOp {
    /// The full roster of this slot.
    Heartbeat(Vec<PlayerRef>),
    /// Ask to admit a character; the answer arrives on `reply`.
    Join {
        character_id: i64,
        sid: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// The character is gone from this slot.
    Leave { character_id: i64 },
}

/// Handle to the sender task; dropping the last handle does not stop it (the game server holds it
/// for its whole life).
#[derive(Resource, Clone)]
pub struct CoordinatorLink {
    tx: mpsc::UnboundedSender<LinkOp>,
    /// Set while a heartbeat waits in the queue; the sender clears it when it takes that heartbeat.
    heartbeat_queued: Arc<AtomicBool>,
}

impl CoordinatorLink {
    fn send(&self, op: LinkOp) -> bool {
        self.tx.send(op).is_ok()
    }

    /// Queue a join and return the receiver its answer will arrive on.
    pub fn join(&self, character_id: i64, sid: String) -> oneshot::Receiver<Result<(), String>> {
        let (reply, answer) = oneshot::channel();
        self.send(LinkOp::Join {
            character_id,
            sid,
            reply,
        });
        answer
    }

    /// Queue a roster, unless one is already waiting. During an outage the heartbeat in flight
    /// retries until the authserver is back; without this a new roster would pile up every 10 s,
    /// and after recovery every queued join would wait behind all of them past `JOIN_GIVE_UP`.
    /// Dropping the newer roster is safe: the waiting one lists exactly the players admitted or
    /// pending before it in the queue, and any join or leave since then is queued after it.
    pub fn heartbeat(&self, players: Vec<PlayerRef>) {
        if self.heartbeat_queued.swap(true, Ordering::AcqRel) {
            return;
        }
        if !self.send(LinkOp::Heartbeat(players)) {
            self.heartbeat_queued.store(false, Ordering::Release);
        }
    }

    pub fn leave(&self, character_id: i64) {
        self.send(LinkOp::Leave { character_id });
    }
}

/// Start the sender task on the server's tokio runtime.
pub fn spawn_link(
    runtime: &tokio::runtime::Runtime,
    coordinator: GameServerCoordinator,
) -> CoordinatorLink {
    let (tx, rx) = mpsc::unbounded_channel();
    // Changes on every process start, so the coordinator can tell a restarted server from a
    // server that lost players.
    let boot_id = format!("{:016x}", rand::random::<u64>());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("failed to build the coordinator HTTP client");
    let heartbeat_queued = Arc::new(AtomicBool::new(false));
    runtime.spawn(run_link(
        client,
        coordinator,
        boot_id,
        rx,
        heartbeat_queued.clone(),
    ));
    CoordinatorLink {
        tx,
        heartbeat_queued,
    }
}

#[derive(Serialize)]
struct HeartbeatBody<'a> {
    slot: u32,
    boot_id: &'a str,
    players: &'a [PlayerRef],
}

#[derive(Serialize)]
struct JoinBody<'a> {
    slot: u32,
    boot_id: &'a str,
    character_id: i64,
    sid: &'a str,
}

#[derive(Serialize)]
struct LeaveBody<'a> {
    slot: u32,
    boot_id: &'a str,
    character_id: i64,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: String,
}

async fn run_link(
    client: reqwest::Client,
    coordinator: GameServerCoordinator,
    boot_id: String,
    mut rx: mpsc::UnboundedReceiver<LinkOp>,
    heartbeat_queued: Arc<AtomicBool>,
) {
    while let Some(op) = rx.recv().await {
        match op {
            LinkOp::Join {
                character_id,
                sid,
                reply,
            } => {
                // The caller gave up (connect timeout). The coordinator may still admit the
                // character; the next heartbeat's roster does not contain them, which removes
                // them again.
                if reply.is_closed() {
                    debug!(
                        character_id,
                        "join: caller gave up before the request was sent"
                    );
                    continue;
                }
                let body = JoinBody {
                    slot: coordinator.slot,
                    boot_id: &boot_id,
                    character_id,
                    sid: &sid,
                };
                let answer = send_join(&client, &coordinator, &body).await;
                let _ = reply.send(answer);
            }
            LinkOp::Heartbeat(players) => {
                // Taken off the queue: the next roster may wait behind this one.
                heartbeat_queued.store(false, Ordering::Release);
                let body = HeartbeatBody {
                    slot: coordinator.slot,
                    boot_id: &boot_id,
                    players: &players,
                };
                send_until_accepted(
                    &client,
                    &coordinator,
                    "/internal/v1/servers/heartbeat",
                    &body,
                )
                .await;
            }
            LinkOp::Leave { character_id } => {
                let body = LeaveBody {
                    slot: coordinator.slot,
                    boot_id: &boot_id,
                    character_id,
                };
                send_until_accepted(&client, &coordinator, "/internal/v1/servers/leave", &body)
                    .await;
            }
        }
    }
}

async fn post(
    client: &reqwest::Client,
    coordinator: &GameServerCoordinator,
    path: &str,
    body: &impl Serialize,
) -> Result<reqwest::Response, reqwest::Error> {
    client
        .post(format!(
            "{}{path}",
            coordinator.endpoint.trim_end_matches('/')
        ))
        .bearer_auth(&coordinator.token)
        .json(body)
        .send()
        .await
}

async fn send_join(
    client: &reqwest::Client,
    coordinator: &GameServerCoordinator,
    body: &JoinBody<'_>,
) -> Result<(), String> {
    let response = match post(client, coordinator, "/internal/v1/servers/join", body).await {
        Ok(response) => response,
        Err(e) => {
            error!("coordinator join failed: {e}");
            return Err("coordinator unavailable".to_string());
        }
    };

    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    if status == StatusCode::CONFLICT {
        let reason = match response.json::<ErrorBody>().await {
            Ok(body) if body.error == "session_ended" => "session ended",
            _ => "character already connected on another server",
        };
        info!("coordinator refused the join: {reason}");
        return Err(reason.to_string());
    }
    error!("coordinator join failed: {status}");
    Err("coordinator unavailable".to_string())
}

/// Retry until the authserver accepts. A 400 is a bug in the body (the slot, say): log it and
/// drop it, since retrying cannot fix it.
async fn send_until_accepted(
    client: &reqwest::Client,
    coordinator: &GameServerCoordinator,
    path: &str,
    body: &impl Serialize,
) {
    let mut delay = Duration::from_secs(1);
    let mut attempt = 0;
    loop {
        attempt += 1;
        let failure = match post(client, coordinator, path, body).await {
            Ok(response) if response.status().is_success() => return,
            Ok(response) if response.status() == StatusCode::BAD_REQUEST => {
                error!(
                    "coordinator {path} rejected the request (attempt {attempt}): {} (not retrying)",
                    response.status()
                );
                return;
            }
            Ok(response) => response.status().to_string(),
            Err(e) => e.to_string(),
        };
        error!("coordinator {path} failed (attempt {attempt}): {failure}");
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(MAX_RETRY_DELAY);
    }
}

/// Reports the roster and leaves to the coordinator.
pub struct CoordinatorPlugin;

impl Plugin for CoordinatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(ServerState::Running), send_heartbeat)
            .add_systems(
                Update,
                (
                    // After `admit_players`, so its deferred spawns are applied first: a player
                    // admitted this frame is no longer pending, so the roster would otherwise
                    // miss them, and the coordinator never re-adds players from a known boot.
                    send_heartbeat
                        .run_if(bevy::time::common_conditions::on_timer(HEARTBEAT_EVERY))
                        .after(crate::admit_players),
                    // After both writers of `PlayerDisconnected`, so a disconnect is reported in
                    // the frame it happens; before `on_player_disconnect` strips
                    // `ConnectedPlayer`; and before `vet_connect_requests`, so a reconnect's join
                    // is always queued after the leave of the connection it replaces.
                    report_leaves
                        .after(crate::check_heartbeats)
                        .after(crate::on_disconnect_packet)
                        .before(crate::on_player_disconnect)
                        .before(crate::vet_connect_requests),
                )
                    .run_if(in_state(ServerState::Running)),
            );
    }
}

/// The roster is the connected players, plus the ones whose join is in flight. The second and
/// third groups matter: a join leaves `PendingJoins` the moment it is answered and only becomes a
/// `ConnectedPlayer` when `admit_players` runs, so a heartbeat taken inside that gap would report
/// the player as gone, and the coordinator never re-adds players from a known `boot_id`.
fn send_heartbeat(
    link: Res<CoordinatorLink>,
    connected: Query<(&PlayerId, &PlayerSid), With<ConnectedPlayer>>,
    pending: Res<PendingJoins>,
    ready: Res<ReadyToAdmit>,
) {
    let mut players: Vec<PlayerRef> = connected
        .iter()
        .map(|(id, sid)| PlayerRef {
            character_id: id.0 as i64,
            sid: sid.0.clone(),
        })
        .collect();
    players.extend(
        pending
            .0
            .lock()
            .expect("pending joins lock")
            .iter()
            .filter_map(|join| join.request.player_ref()),
    );
    players.extend(
        ready
            .0
            .lock()
            .expect("ready to admit lock")
            .iter()
            .filter_map(|request| request.player_ref()),
    );
    players.sort_by(|a, b| {
        a.character_id
            .cmp(&b.character_id)
            .then_with(|| a.sid.cmp(&b.sid))
    });
    players.dedup();
    link.heartbeat(players);
}

/// Report players that left (quit, timed out) to the coordinator, which signals their session
/// workflow so the session leaves `in_game`.
fn report_leaves(
    mut disconnects: MessageReader<shared::event::client::PlayerDisconnected>,
    // `DisconnectedPlayer` as well as `ConnectedPlayer`: `on_player_disconnect` may already have
    // run for this frame's messages, but it keeps the entity (and its sid) around.
    players: Query<(&PlayerId, &PlayerSid), Or<(With<ConnectedPlayer>, With<DisconnectedPlayer>)>>,
    link: Res<CoordinatorLink>,
) {
    let mut leaves: Vec<i64> = disconnects
        .read()
        .filter_map(|player| {
            players
                .iter()
                .find(|(id, _)| id.0 == player.id.0)
                .map(|(id, _)| id.0 as i64)
        })
        .collect();
    if leaves.is_empty() {
        return;
    }
    leaves.sort_unstable();
    leaves.dedup();
    for character_id in leaves {
        info!(character_id, "reporting leave to the coordinator");
        link.leave(character_id);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use axum::response::IntoResponse;
    use serde_json::{Value, json};

    use super::*;

    const TOKEN: &str = "sekrit-token";

    #[derive(Debug, Clone)]
    struct Seen {
        path: String,
        auth: Option<String>,
        body: Value,
    }

    /// Stands in for the authserver's internal API: records each request and replays `script`,
    /// answering 204 when the script runs out.
    struct Mock {
        seen: Mutex<Vec<Seen>>,
        script: Mutex<VecDeque<(u16, Value)>>,
    }

    impl Mock {
        fn requests(&self) -> Vec<Seen> {
            self.seen.lock().unwrap().clone()
        }

        fn len(&self) -> usize {
            self.seen.lock().unwrap().len()
        }
    }

    async fn mock_handler(
        axum::extract::State(mock): axum::extract::State<Arc<Mock>>,
        request: axum::extract::Request,
    ) -> axum::response::Response {
        let path = request.uri().path().to_string();
        let auth = request
            .headers()
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = axum::body::to_bytes(request.into_body(), 1 << 20)
            .await
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(Value::Null);
        mock.seen.lock().unwrap().push(Seen { path, auth, body });

        let next = mock.script.lock().unwrap().pop_front();
        match next {
            Some((status, body)) if !body.is_null() => {
                let status = StatusCode::from_u16(status).unwrap();
                (status, axum::Json(body)).into_response()
            }
            Some((status, _)) => StatusCode::from_u16(status).unwrap().into_response(),
            None => StatusCode::NO_CONTENT.into_response(),
        }
    }

    fn start_mock(
        runtime: &tokio::runtime::Runtime,
        script: Vec<(u16, Value)>,
    ) -> (String, Arc<Mock>) {
        let mock = Arc::new(Mock {
            seen: Mutex::new(Vec::new()),
            script: Mutex::new(script.into()),
        });
        let listener =
            runtime.block_on(async { tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap() });
        let address = listener.local_addr().unwrap();
        let app = axum::Router::new()
            .fallback(mock_handler)
            .with_state(mock.clone());
        runtime.spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{address}"), mock)
    }

    fn coordinator(endpoint: &str) -> GameServerCoordinator {
        GameServerCoordinator {
            endpoint: endpoint.to_string(),
            slot: 2,
            token: TOKEN.to_string(),
        }
    }

    fn players() -> Vec<PlayerRef> {
        vec![PlayerRef {
            character_id: 7,
            sid: "0123456789abcdef".to_string(),
        }]
    }

    fn wait_for(mock: &Arc<Mock>, count: usize, timeout: Duration) {
        let start = Instant::now();
        loop {
            let seen = mock.len();
            if seen >= count {
                return;
            }
            assert!(
                start.elapsed() < timeout,
                "waited {timeout:?} for {count} requests, saw {seen}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn retries_then_sends_in_order() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        // The first heartbeat is refused twice before it lands.
        let (endpoint, mock) = start_mock(
            &runtime,
            vec![(503, Value::Null), (503, Value::Null), (204, Value::Null)],
        );
        let link = spawn_link(&runtime, coordinator(&endpoint));

        link.heartbeat(players());
        // The first heartbeat is in flight, so the second one queues behind the leave.
        wait_for(&mock, 1, Duration::from_secs(5));
        link.leave(7);
        link.heartbeat(players());

        wait_for(&mock, 5, Duration::from_secs(15));
        let requests = mock.requests();
        let paths: Vec<&str> = requests.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "/internal/v1/servers/heartbeat",
                "/internal/v1/servers/heartbeat",
                "/internal/v1/servers/heartbeat",
                "/internal/v1/servers/leave",
                "/internal/v1/servers/heartbeat",
            ]
        );
        assert_eq!(requests[2].body["players"], json!(players()));
        assert_eq!(requests[2].body["slot"], 2);
        assert_eq!(
            requests[3].body,
            json!({"slot": 2, "boot_id": requests[3].body["boot_id"], "character_id": 7})
        );
        // One boot id for the whole process.
        assert_eq!(requests[0].body["boot_id"], requests[4].body["boot_id"]);
    }

    #[test]
    fn one_heartbeat_waits_behind_the_one_in_flight() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let roster = |character_id: i64| {
            vec![PlayerRef {
                character_id,
                sid: "0123456789abcdef".to_string(),
            }]
        };
        // An outage: the first heartbeat is refused twice (1 s + 2 s of retries).
        let (endpoint, mock) = start_mock(
            &runtime,
            vec![(503, Value::Null), (503, Value::Null), (204, Value::Null)],
        );
        let link = spawn_link(&runtime, coordinator(&endpoint));

        link.heartbeat(roster(1));
        wait_for(&mock, 1, Duration::from_secs(5));
        link.heartbeat(roster(2)); // waits behind roster 1
        link.heartbeat(roster(3)); // dropped: roster 2 is still waiting
        link.leave(9);
        link.heartbeat(roster(4)); // dropped: roster 2 is still waiting

        wait_for(&mock, 5, Duration::from_secs(15));
        // Nothing else was queued.
        std::thread::sleep(Duration::from_millis(300));
        let requests = mock.requests();
        let summary: Vec<(String, Value)> = requests
            .iter()
            .map(|r| {
                let what = if r.path.ends_with("/leave") {
                    r.body["character_id"].clone()
                } else {
                    r.body["players"][0]["character_id"].clone()
                };
                (r.path.rsplit('/').next().unwrap().to_string(), what)
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("heartbeat".to_string(), json!(1)),
                ("heartbeat".to_string(), json!(1)),
                ("heartbeat".to_string(), json!(1)),
                ("heartbeat".to_string(), json!(2)),
                ("leave".to_string(), json!(9)),
            ]
        );

        // Once the waiting roster is taken, the next one queues again.
        link.heartbeat(roster(5));
        wait_for(&mock, 6, Duration::from_secs(5));
        assert_eq!(mock.requests()[5].body["players"][0]["character_id"], 5);
    }

    #[test]
    fn join_maps_the_authserver_answers() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let (endpoint, mock) = start_mock(
            &runtime,
            vec![
                (200, json!({"result": "admitted"})),
                (409, json!({"error": "already_on_other_server"})),
                (409, json!({"error": "session_ended"})),
                (503, Value::Null),
            ],
        );
        let link = spawn_link(&runtime, coordinator(&endpoint));

        let join = |link: &CoordinatorLink| {
            runtime
                .block_on(link.join(7, "0123456789abcdef".to_string()))
                .unwrap()
        };
        assert_eq!(join(&link), Ok(()));
        // The wire format the authserver's internal API documents.
        assert_eq!(
            mock.requests()[0].body,
            json!({
                "slot": 2,
                "boot_id": mock.requests()[0].body["boot_id"],
                "character_id": 7,
                "sid": "0123456789abcdef",
            })
        );
        assert_eq!(
            join(&link),
            Err("character already connected on another server".to_string())
        );
        assert_eq!(join(&link), Err("session ended".to_string()));
        assert_eq!(join(&link), Err("coordinator unavailable".to_string()));
    }

    #[test]
    fn join_the_caller_gave_up_on_is_not_sent() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let (endpoint, mock) = start_mock(&runtime, vec![]);
        let link = spawn_link(&runtime, coordinator(&endpoint));

        let (reply, answer) = oneshot::channel();
        link.send(LinkOp::Join {
            character_id: 7,
            sid: "0123456789abcdef".to_string(),
            reply,
        });
        drop(answer);
        link.heartbeat(vec![]);

        wait_for(&mock, 1, Duration::from_secs(5));
        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "/internal/v1/servers/heartbeat");
    }

    #[test]
    fn requests_carry_the_token_and_debug_hides_it() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let (endpoint, mock) = start_mock(&runtime, vec![]);
        let coordinator = coordinator(&endpoint);
        let debug = format!("{coordinator:?}");
        assert!(debug.contains("slot: 2"), "{debug}");
        assert!(!debug.contains(TOKEN), "{debug}");

        let link = spawn_link(&runtime, coordinator);
        link.heartbeat(players());
        wait_for(&mock, 1, Duration::from_secs(5));
        assert_eq!(
            mock.requests()[0].auth.as_deref(),
            Some(format!("Bearer {TOKEN}").as_str())
        );
    }
}
