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
            .add_systems(Update, poll_auth_responses)
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
                auth_state.set(AuthState::Idle);
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
                pending.0.push((
                    Op::Play {
                        retry_as_guest: false,
                    },
                    http.post("/api/v1/play", json!({}), session.native_token.clone()),
                ));
            }
            (Op::Play { .. }, Ok(body)) => {
                let assigned = str_field(&body, "ticket").zip(body.get("server")).and_then(
                    |(ticket, server)| {
                        Some((
                            ticket,
                            str_field(server, "udp")?,
                            str_field(server, "ws_path")?,
                        ))
                    },
                );
                match assigned {
                    Some((ticket, udp, ws_path)) => {
                        commands.insert_resource(GameTicket(ticket));
                        // Native connects straight to the assigned address; web keeps using
                        // `ws_path` on the page's own host (see `web::game_server_url`).
                        #[cfg(not(feature = "web"))]
                        crate::apply_server_addr(&mut config, &udp);
                        commands.insert_resource(AssignedServer { udp, ws_path });
                        auth_state.set(AuthState::Ready);
                    }
                    None => fail(&mut notif, &mut auth_state, "play response missing fields"),
                }
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
            (Op::Guest | Op::Play { .. }, Err(e)) => fail(&mut notif, &mut auth_state, &e),
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
