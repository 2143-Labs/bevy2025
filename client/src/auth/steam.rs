//! "Login with Steam" for Steam builds: a Steamworks Web API ticket (identity `rots-auth`) is
//! traded at the authserver for an account session.

use std::{fmt::Write, sync::Arc};

use bevy::prelude::*;
use bevy_steamworks::{CallbackHandle, Client, SteamworksPlugin, TicketForWebApiResponse};
use parking_lot::Mutex;

use super::{
    AuthRequest, AuthSession, AvailableLoginMethods, LoginMethod, PendingAuth, http::AuthHttp,
};
use crate::notification::Notification;

pub const ROTS_STEAM_APP_ID: u32 = 4346770;

/// Must match the identity the authserver verifies tickets against.
const TICKET_IDENTITY: &str = "rots-auth";

/// Add Steamworks to `app` if Steam is running. Must run before `DefaultPlugins`.
pub fn init(app: &mut App) {
    match SteamworksPlugin::init_app(ROTS_STEAM_APP_ID) {
        Ok(plugin) => {
            app.add_plugins((plugin, SteamLoginPlugin));
        }
        Err(e) => {
            // Logging is not set up until DefaultPlugins; report once it is.
            let msg = format!("Steam unavailable: {e:?}");
            app.add_systems(Startup, move || warn!("{msg}"));
        }
    }
}

struct SteamLoginPlugin;

impl Plugin for SteamLoginPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvailableLoginMethods>()
            .init_resource::<SteamTicketSlot>()
            .add_systems(Startup, register_ticket_callback)
            .add_systems(Update, (request_steam_ticket, send_steam_ticket));
        app.world_mut()
            .resource_mut::<AvailableLoginMethods>()
            .0
            .push(LoginMethod::SteamTicket);
    }
}

type TicketResult = Result<Vec<u8>, String>;

/// Ticket bytes from the `TicketForWebApiResponse` callback, waiting to be sent.
#[derive(Resource, Default)]
struct SteamTicketSlot {
    slot: Arc<Mutex<Option<TicketResult>>>,
    in_flight: bool,
}

/// Keeps the Steam callback registered for the life of the app.
#[derive(Resource)]
struct TicketCallback(#[allow(dead_code)] CallbackHandle);

fn register_ticket_callback(
    mut commands: Commands,
    client: Res<Client>,
    slot: Res<SteamTicketSlot>,
) {
    let slot = slot.slot.clone();
    let handle = client.register_callback(move |resp: TicketForWebApiResponse| {
        let result = resp.result.map_err(|e| format!("{e:?}")).and_then(|()| {
            usize::try_from(resp.ticket_len)
                .ok()
                .and_then(|len| resp.ticket.get(..len))
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("bad ticket length {}", resp.ticket_len))
        });
        *slot.lock() = Some(result);
    });
    commands.insert_resource(TicketCallback(handle));
}

/// Ask Steam for a Web API ticket. `AuthState` is left alone: it tracks game tickets, and a
/// Steam login only changes which session the next game ticket is fetched for.
fn request_steam_ticket(
    mut requests: MessageReader<AuthRequest>,
    client: Res<Client>,
    mut slot: ResMut<SteamTicketSlot>,
) {
    for request in requests.read() {
        if !matches!(request, AuthRequest::Login(LoginMethod::SteamTicket)) || slot.in_flight {
            continue;
        }
        info!("Requesting Steam Web API ticket");
        slot.in_flight = true;
        // The callback delivers the ticket; the handle only matters for cancelling.
        let _ = client
            .user()
            .authentication_session_ticket_for_webapi(TICKET_IDENTITY);
    }
}

fn send_steam_ticket(
    mut slot: ResMut<SteamTicketSlot>,
    http: AuthHttp,
    mut pending: ResMut<PendingAuth>,
    session: Res<AuthSession>,
    mut notif: MessageWriter<Notification>,
) {
    let Some(result) = slot.slot.lock().take() else {
        return;
    };
    slot.in_flight = false;
    match result {
        Ok(ticket) => {
            let hex = ticket
                .iter()
                .fold(String::with_capacity(ticket.len() * 2), |mut s, b| {
                    let _ = write!(s, "{b:02x}");
                    s
                });
            super::start_steam_login(&http, &mut pending, &session, hex);
        }
        Err(e) => super::notify_login_failed(&mut notif, &format!("Steam ticket: {e}")),
    }
}
