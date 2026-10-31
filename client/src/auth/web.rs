//! Web auth backend: the authserver is same-origin under `/auth`, the session is the
//! `rots_session` cookie, and provider logins are full-page OAuth/OpenID redirects that come back
//! to `/` (or `/?login_error=<code>`).

use bevy::prelude::*;
use serde_json::Value;

use super::{
    AuthBaseUrl, AuthRequest, AvailableLoginMethods, LoginMethod,
    http::{AuthHttp, HttpSlot},
};
use crate::notification::Notification;

pub struct WebAuthPlugin;

impl Plugin for WebAuthPlugin {
    fn build(&self, app: &mut App) {
        let origin = web_sys::window()
            .expect("no window")
            .location()
            .origin()
            .expect("no location origin");
        app.insert_resource(AuthBaseUrl(format!("{origin}/auth")))
            .add_systems(Startup, (fetch_providers, show_login_error))
            .add_systems(
                Update,
                (
                    receive_providers.run_if(resource_exists::<ProvidersRequest>),
                    start_provider_login,
                ),
            );
    }
}

#[derive(Resource)]
struct ProvidersRequest(HttpSlot);

fn fetch_providers(mut commands: Commands, http: AuthHttp) {
    commands.insert_resource(ProvidersRequest(http.get("/api/v1/providers", None)));
}

fn receive_providers(
    mut commands: Commands,
    request: Res<ProvidersRequest>,
    mut methods: ResMut<AvailableLoginMethods>,
) {
    let Some(result) = request.0.lock().take() else {
        return;
    };
    commands.remove_resource::<ProvidersRequest>();
    match result {
        Ok(body) => {
            methods.0 = body
                .get("providers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|p| match p.as_str()? {
                    "steam_openid" => Some(LoginMethod::SteamOpenId),
                    "discord" => Some(LoginMethod::Discord),
                    "au2143" => Some(LoginMethod::Au2143),
                    _ => None,
                })
                .collect();
        }
        Err(e) => warn!("Could not load login providers: {e}"),
    }
}

fn start_provider_login(mut requests: MessageReader<AuthRequest>, base: Res<AuthBaseUrl>) {
    for request in requests.read() {
        let provider = match request {
            AuthRequest::Login(LoginMethod::SteamOpenId) => "steam",
            AuthRequest::Login(LoginMethod::Discord) => "discord",
            AuthRequest::Login(LoginMethod::Au2143) => "au2143",
            _ => continue,
        };
        let url = format!("{}/api/v1/{provider}/login?return_to=/", base.0);
        if let Err(e) = web_sys::window()
            .expect("no window")
            .location()
            .set_href(&url)
        {
            warn!("Could not start {provider} login: {e:?}");
        }
    }
}

/// The authserver redirects failed logins to `/?login_error=<code>`.
fn show_login_error(mut notif: MessageWriter<Notification>) {
    let window = web_sys::window().expect("no window");
    let Ok(search) = window.location().search() else {
        return;
    };
    let Some(code) = search
        .trim_start_matches('?')
        .split('&')
        .find_map(|kv| kv.strip_prefix("login_error="))
    else {
        return;
    };
    warn!("Login failed: {code}");
    notif.write(Notification(format!("Login failed: {code}")));
    if let Ok(history) = window.history() {
        let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some("/"));
    }
}
