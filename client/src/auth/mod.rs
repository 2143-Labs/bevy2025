//! Login and game tickets, shared by every build.
//!
//! The authserver (rots-authserver, served under `/auth`) owns accounts and sessions. This module
//! keeps the client's view of the session in [`AuthSession`], and turns [`AuthRequest`]s into
//! HTTP calls. Every connection to a game server needs a [`GameTicket`] fetched for the
//! session's character.
//!
//! Platform backends add login methods and the base URL; they are registered in `main.rs` only:
//! - `native`: tokio runtime, `--auth-url`, session token kept in memory only.
//! - `steam`: "Login with Steam" through a Steamworks Web API ticket.
//! - `web`: same-origin `/auth`, cookie session, OAuth redirects.

pub mod http;
#[cfg(not(feature = "web"))]
pub mod native;
#[cfg(feature = "steam")]
pub mod steam;
#[cfg(feature = "web")]
pub mod web;

use bevy::prelude::*;
use bevy::time::Real;
use serde_json::{Value, json};

use crate::notification::Notification;
use http::{AuthHttp, HttpResult, HttpSlot};

/// Authserver root, no trailing slash, e.g. `https://rots.john2143.com/auth`.
#[derive(Resource, Clone)]
pub struct AuthBaseUrl(pub String);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(
    dead_code,
    reason = "each platform backend constructs only the methods it offers"
)]
pub enum LoginMethod {
    SteamTicket,
    SteamOpenId,
    Discord,
    Au2143,
}

impl LoginMethod {
    pub fn label(self) -> &'static str {
        match self {
            LoginMethod::SteamTicket | LoginMethod::SteamOpenId => "Login with Steam",
            LoginMethod::Discord => "Login with Discord",
            LoginMethod::Au2143 => "Login with au.2143.me",
        }
    }
}

#[derive(Resource, Default, Clone)]
pub struct AvailableLoginMethods(pub Vec<LoginMethod>);

/// What the authserver knows about us. Empty until `/me` or `/guest` answers.
#[derive(Resource, Default, Clone, Debug)]
pub struct AuthSession {
    /// `"guest"` or `"account"`.
    pub kind: Option<String>,
    pub character_id: Option<i64>,
    pub display_name: Option<String>,
    pub claim_code: Option<String>,
    /// Bearer session token, in memory only (native). Web relies on the `rots_session` cookie.
    pub native_token: Option<String>,
}

impl AuthSession {
    pub fn is_account(&self) -> bool {
        self.kind.as_deref() == Some("account")
    }

    /// The request that gets us a ticket: reuse the session if we have (or may have) one,
    /// otherwise become a new guest.
    pub fn ticket_request(&self) -> AuthRequest {
        if self.character_id.is_some() || self.native_token.is_some() {
            AuthRequest::FetchTicket
        } else {
            AuthRequest::PlayNow
        }
    }
}

/// Game ticket for the current session's character, sent in `ConnectRequest`.
#[derive(Resource, Clone)]
pub struct GameTicket(pub String);

/// Where the authserver placed this session's character. Native clients connect to `udp`, web
/// clients to `ws_path` on the page's own host.
#[derive(Resource, Clone)]
pub struct AssignedServer {
    /// `host:port` native clients connect to.
    #[allow(dead_code, reason = "web connects over `ws_path` instead")]
    pub udp: String,
    /// Path on the page's own host that web clients connect to.
    #[allow(dead_code, reason = "native connects to `udp` instead")]
    pub ws_path: String,
    /// The tutorial instance's join code, for slice C's "join someone" box. Not shown yet.
    #[allow(dead_code, reason = "slice C displays the join code")]
    pub join_code: Option<String>,
    /// A migrated world comes back paused; nothing acts on it until slice D adds pausing.
    #[allow(dead_code, reason = "slice D unpauses")]
    pub paused: bool,
}

/// Why `/api/v1/play` did not hand out a server yet: the fleet is starting one, or it has none to
/// give. Both are retried rather than treated as a failed login.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayWait {
    /// `server_starting` / `server_recovering`: an instance for this character is coming up.
    Starting,
    /// `servers_full`: the fleet is at capacity; the authserver frees a slot when one drains.
    Full,
}

impl PlayWait {
    /// The error codes the authserver answers with while a server is not ready.
    fn from_code(code: &str) -> Option<Self> {
        match code {
            "server_starting" | "server_recovering" => Some(Self::Starting),
            "servers_full" => Some(Self::Full),
            _ => None,
        }
    }

    /// What the connecting menu shows while it waits.
    pub fn status_text(self) -> &'static str {
        match self {
            Self::Starting => "Starting a server…",
            Self::Full => "Servers are full, retrying…",
        }
    }

    /// How long to wait between attempts.
    fn retry_every(self) -> f32 {
        match self {
            Self::Starting => 2.0,
            Self::Full => 5.0,
        }
    }

    /// How long to keep trying before giving up.
    fn give_up_after(self) -> f32 {
        match self {
            Self::Starting => 120.0,
            Self::Full => 600.0,
        }
    }

    /// What the player is told if it never becomes available.
    fn gave_up(self) -> &'static str {
        match self {
            Self::Starting => "no game server became ready",
            Self::Full => "the servers are full",
        }
    }
}

/// A `/play` that will be tried again: the authserver has not got a server to hand out yet.
#[derive(Resource, Clone, Copy, Debug)]
pub struct PlayRetry {
    pub wait: PlayWait,
    /// Seconds until the next attempt.
    until_next: f32,
    /// Seconds until the client stops trying.
    left: f32,
}

impl PlayRetry {
    fn new(wait: PlayWait) -> Self {
        Self {
            wait,
            until_next: wait.retry_every(),
            left: wait.give_up_after(),
        }
    }
}

/// Progress of the most recent ticket request. `Ready` means a fresh [`GameTicket`] is waiting to
/// be used by the connecting menu.
#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum AuthState {
    #[default]
    Idle,
    Requesting,
    Ready,
    Failed,
}

#[derive(Message, Clone, Debug)]
pub enum AuthRequest {
    /// Create a guest character, then fetch a ticket for it.
    PlayNow,
    FetchTicket,
    /// Handled by the platform backend that offers the method.
    #[cfg_attr(
        not(any(feature = "web", feature = "steam")),
        allow(dead_code, reason = "no login backend in this build")
    )]
    Login(LoginMethod),
    Logout,
    Claim(String),
    /// Tell the authserver a character's game server stopped answering (`/api/v1/my-server-crashed`),
    /// which makes the coordinator probe and, if it is dead, recover the instance.
    ReportServerCrashed,
}

/// What to do with a finished request.
#[derive(Clone, Copy)]
enum Op {
    Guest,
    /// Ask the authserver to place the session's character (`/api/v1/play`).
    Play {
        /// A 401 means our session is gone: retry once as a fresh guest.
        retry_as_guest: bool,
    },
    Me,
    Claim,
    Logout,
    ServerCrashed,
    #[cfg(feature = "steam")]
    SteamLogin,
}

#[derive(Resource, Default)]
struct PendingAuth(Vec<(Op, HttpSlot)>);

impl PendingAuth {
    fn has(&self, f: impl Fn(&Op) -> bool) -> bool {
        self.0.iter().any(|(op, _)| f(op))
    }
}

pub struct AuthPlugin;

impl Plugin for AuthPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AuthState>()
            .add_message::<AuthRequest>()
            .init_resource::<AvailableLoginMethods>()
            .init_resource::<AuthSession>()
            .init_resource::<PendingAuth>()
            .add_systems(Startup, load_session)
            // Chained: a response consumed by `poll_auth_responses` removes the retry before
            // `retry_play` can fire one more request in the same frame.
            .add_systems(Update, (poll_auth_responses, retry_play).chain())
            // PostUpdate so requests written by UI systems this frame are started before the
            // frame's state transitions run.
            .add_systems(PostUpdate, handle_auth_requests);
    }
}

/// Startup (web only): the browser may already hold a session cookie; ask `/me` about it.
/// Native builds start without a session (Steam builds log in from `auth::steam`).
fn load_session(http: AuthHttp, mut pending: ResMut<PendingAuth>) {
    if cfg!(target_arch = "wasm32") {
        pending.0.push((Op::Me, http.get("/api/v1/me", None)));
    }
}

fn handle_auth_requests(
    mut requests: MessageReader<AuthRequest>,
    http: AuthHttp,
    mut pending: ResMut<PendingAuth>,
    mut session: ResMut<AuthSession>,
    mut auth_state: ResMut<NextState<AuthState>>,
    mut commands: Commands,
) {
    for request in requests.read() {
        match request {
            AuthRequest::PlayNow | AuthRequest::FetchTicket => {
                // The connecting menu waits on this state; a ticket already on its way (repeated
                // clicks, or one left over from a cancelled attempt) is shared, not duplicated.
                auth_state.set(AuthState::Requesting);
                if pending.has(|op| matches!(op, Op::Guest | Op::Play { .. })) {
                    continue;
                }
                let slot = if matches!(request, AuthRequest::PlayNow) {
                    (
                        Op::Guest,
                        http.post("/api/v1/guest", json!({ "client": client_kind() }), None),
                    )
                } else {
                    (
                        Op::Play {
                            retry_as_guest: true,
                        },
                        http.post("/api/v1/play", json!({}), session.native_token.clone()),
                    )
                };
                pending.0.push(slot);
            }
            AuthRequest::Logout => {
                let token = session.native_token.take();
                pending
                    .0
                    .push((Op::Logout, http.post("/api/v1/logout", json!({}), token)));
                *session = AuthSession::default();
                commands.remove_resource::<GameTicket>();
                commands.remove_resource::<PlayRetry>();
                auth_state.set(AuthState::Idle);
            }
            // A crash report is fire-and-forget: the authserver answers 202, and the reconnect
            // keeps polling `/play` regardless.
            AuthRequest::ReportServerCrashed => {
                info!("reporting a crashed game server to the authserver");
                pending.0.push((
                    Op::ServerCrashed,
                    http.post(
                        "/api/v1/my-server-crashed",
                        json!({}),
                        session.native_token.clone(),
                    ),
                ));
            }
            AuthRequest::Claim(code) => {
                pending.0.push((
                    Op::Claim,
                    http.post(
                        "/api/v1/claim",
                        json!({ "claim_code": code }),
                        session.native_token.clone(),
                    ),
                ));
            }
            // Platform backends own their login flows.
            AuthRequest::Login(_) => {}
        }
    }
}

/// Start the Steam Web API ticket login. Called by `steam.rs` once Steam hands over the ticket.
#[cfg(feature = "steam")]
fn start_steam_login(
    http: &AuthHttp,
    pending: &mut PendingAuth,
    session: &AuthSession,
    steam_ticket_hex: String,
) {
    pending.0.push((
        Op::SteamLogin,
        http.post(
            "/api/v1/steam/ticket",
            json!({ "steam_ticket_hex": steam_ticket_hex, "client": "native" }),
            session.native_token.clone(),
        ),
    ));
}

fn str_field(json: &Value, key: &str) -> Option<String> {
    json.get(key).and_then(Value::as_str).map(str::to_string)
}

fn poll_auth_responses(
    http: AuthHttp,
    mut pending: ResMut<PendingAuth>,
    mut session: ResMut<AuthSession>,
    mut auth_state: ResMut<NextState<AuthState>>,
    mut notif: MessageWriter<Notification>,
    mut commands: Commands,
    #[cfg(not(feature = "web"))] mut config: ResMut<crate::Config>,
) {
    if pending.0.is_empty() {
        return;
    }
    let mut finished: Vec<(Op, HttpResult)> = Vec::new();
    pending.0.retain(|(op, slot)| match slot.lock().take() {
        Some(result) => {
            finished.push((*op, result));
            false
        }
        None => true,
    });

    for (op, result) in finished {
        match (op, result) {
            (Op::Guest, Ok(body)) => {
                let token = str_field(&body, "session_token");
                *session = AuthSession {
                    kind: Some("guest".to_string()),
                    character_id: body.get("character_id").and_then(Value::as_i64),
                    display_name: None,
                    claim_code: str_field(&body, "claim_code"),
                    native_token: token,
                };
                if let Some(code) = &session.claim_code {
                    // The only way to keep a guest character: log in later and enter this.
                    info!(
                        "Guest claim code: {code} (log in later and enter it to keep this character)"
                    );
                    notif.write(Notification(format!("Guest claim code: {code}")));
                }
                pending.0.push((
                    Op::Play {
                        retry_as_guest: false,
                    },
                    http.post("/api/v1/play", json!({}), session.native_token.clone()),
                ));
            }
            (Op::Play { .. }, Ok(body)) => {
                commands.remove_resource::<PlayRetry>();
                let assigned = str_field(&body, "ticket").zip(body.get("server")).and_then(
                    |(ticket, server)| {
                        Some((
                            ticket,
                            str_field(server, "udp")?,
                            str_field(server, "ws_path")?,
                            str_field(server, "join_code"),
                            // Absent from an authserver older than slice A: not paused.
                            server
                                .get("paused")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        ))
                    },
                );
                match assigned {
                    Some((ticket, udp, ws_path, join_code, paused)) => {
                        commands.insert_resource(GameTicket(ticket));
                        // Native connects straight to the assigned address; web keeps using
                        // `ws_path` on the page's own host (see `web::game_server_url`).
                        #[cfg(not(feature = "web"))]
                        crate::apply_server_addr(&mut config, &udp);
                        commands.insert_resource(AssignedServer {
                            udp,
                            ws_path,
                            join_code,
                            paused,
                        });
                        auth_state.set(AuthState::Ready);
                    }
                    None => fail(&mut notif, &mut auth_state, "play response missing fields"),
                }
            }
            // A server is coming up, or the fleet is full: retry later instead of failing the
            // login. `AuthState` stays `Requesting`, so the connecting menu keeps showing progress.
            (Op::Play { .. }, Err(code)) if PlayWait::from_code(&code).is_some() => {
                let wait = PlayWait::from_code(&code).expect("guarded above");
                info!("no server yet ({code}): retrying");
                notif.write(Notification(wait.status_text().to_string()));
                commands.insert_resource(PlayRetry::new(wait));
            }
            (
                Op::Play {
                    retry_as_guest: true,
                },
                Err(e),
            ) if e == "no_session" => {
                info!("Session expired, continuing as a new guest");
                *session = AuthSession::default();
                pending.0.push((
                    Op::Guest,
                    http.post("/api/v1/guest", json!({ "client": client_kind() }), None),
                ));
            }
            (Op::Me, Ok(body)) => {
                *session = AuthSession {
                    kind: str_field(&body, "kind"),
                    character_id: body.get("character_id").and_then(Value::as_i64),
                    display_name: str_field(&body, "display_name")
                        .or_else(|| str_field(&body, "character_name")),
                    claim_code: str_field(&body, "claim_code"),
                    native_token: session.native_token.take(),
                };
            }
            (Op::Me, Err(e)) if e == "no_session" => {
                info!("Session is no longer valid");
                *session = AuthSession::default();
            }
            (Op::Me, Err(e)) => warn!("Could not load session: {e}"),
            (Op::Claim, Ok(body)) => {
                let id = body.get("character_id").and_then(Value::as_i64);
                notif.write(Notification(format!(
                    "Claimed character {}",
                    id.map_or_else(|| "?".to_string(), |id| id.to_string())
                )));
            }
            (Op::Claim, Err(e)) => {
                warn!("Claim failed: {e}");
                notif.write(Notification(format!("Claim failed: {e}")));
            }
            (Op::Logout, Ok(_)) => {}
            (Op::Logout, Err(e)) => warn!("Logout request failed: {e}"),
            (Op::ServerCrashed, Ok(_)) => info!("the authserver accepted the crash report"),
            (Op::ServerCrashed, Err(e)) => warn!("Crash report failed: {e}"),
            #[cfg(feature = "steam")]
            (Op::SteamLogin, Ok(body)) => match str_field(&body, "session_token") {
                Some(token) => {
                    session.native_token = Some(token);
                    pending
                        .0
                        .push((Op::Me, http.get("/api/v1/me", session.native_token.clone())));
                }
                None => notify_login_failed(&mut notif, "session_token missing from response"),
            },
            (Op::Guest | Op::Play { .. }, Err(e)) => {
                commands.remove_resource::<PlayRetry>();
                fail(&mut notif, &mut auth_state, &e);
            }
            #[cfg(feature = "steam")]
            (Op::SteamLogin, Err(e)) => notify_login_failed(&mut notif, &e),
        }
    }
}

/// No game ticket: tell the player and let the connecting menu give up.
fn fail(notif: &mut MessageWriter<Notification>, auth_state: &mut NextState<AuthState>, e: &str) {
    notify_login_failed(notif, e);
    auth_state.set(AuthState::Failed);
}

/// Retry a `/play` that the authserver answered `server_starting`, `server_recovering` or
/// `servers_full`, until it hands out a server or the deadline passes.
fn retry_play(
    time: Res<Time<Real>>,
    http: AuthHttp,
    retry: Option<ResMut<PlayRetry>>,
    mut pending: ResMut<PendingAuth>,
    session: Res<AuthSession>,
    mut notif: MessageWriter<Notification>,
    mut auth_state: ResMut<NextState<AuthState>>,
    mut commands: Commands,
) {
    let Some(mut retry) = retry else {
        return;
    };
    let delta = time.delta_secs();
    retry.until_next -= delta;
    retry.left -= delta;
    if retry.left <= 0.0 {
        let message = retry.wait.gave_up();
        commands.remove_resource::<PlayRetry>();
        fail(&mut notif, &mut auth_state, message);
        return;
    }
    if retry.until_next > 0.0 {
        return;
    }
    // A request is still in flight (slow authserver): look again shortly.
    if pending.has(|op| matches!(op, Op::Play { .. })) {
        retry.until_next = 0.5;
        return;
    }
    pending.0.push((
        Op::Play {
            retry_as_guest: true,
        },
        http.post("/api/v1/play", json!({}), session.native_token.clone()),
    ));
    retry.until_next = retry.wait.retry_every();
}

fn notify_login_failed(notif: &mut MessageWriter<Notification>, e: &str) {
    warn!("Login failed: {e}");
    notif.write(Notification(format!("Login failed: {e}")));
}

/// The `client` field the authserver expects: web gets a cookie, native gets a bearer token.
const fn client_kind() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        "web"
    } else {
        "native"
    }
}
