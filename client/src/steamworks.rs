use bevy::prelude::*;
use shared::event::PlayerId;

pub struct SteamworksPlugin {
    appid: bevy_steamworks::AppId,
    tokio_runtime: std::sync::Arc<tokio::runtime::Runtime>,
}

impl SteamworksPlugin {
    pub fn new(
        app_id: bevy_steamworks::AppId,
        runtime: std::sync::Arc<tokio::runtime::Runtime>,
    ) -> Self {
        SteamworksPlugin {
            appid: app_id,
            tokio_runtime: runtime,
        }
    }
}

impl Plugin for SteamworksPlugin {
    fn build(&self, app: &mut App) {
        // Initialize Steamworks here
        app.add_plugins(bevy_steamworks::SteamworksPlugin::init_app(self.appid).unwrap());
        app.insert_resource(shared::tokio_udp::TokioRuntimeResource(
            self.tokio_runtime.clone(),
        ));
        app.add_systems(
            Startup,
            |client: Res<bevy_steamworks::Client>,
             mut next_steam_login_state: ResMut<NextState<SteamLoginState>>| {
                let app_owner = client.apps().app_owner();
                info!("App Owner Steam ID: {:?}", app_owner);
                next_steam_login_state.set(SteamLoginState::TryLogin);
                //for friend in client.friends().get_friends(FriendFlags::IMMEDIATE) {
                //info!(
                //"Friend: {} = {:?} {:?}",
                //friend.name(),
                //friend.id(),
                //friend.state()
                //);
                //}
            },
        );

        app.insert_state(SteamLoginState::NotLoggedIn);
        app.add_systems(OnEnter(SteamLoginState::TryLogin), try_steam_login);
        app.add_systems(
            Update,
            check_steam_login_response
                .run_if(in_state(SteamLoginState::SentSteamLoginRequestToAuthServer))
                .run_if(resource_exists::<LoginReceiverResource>),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
pub enum SteamLoginState {
    NotLoggedIn,
    TryLogin,
    SentSteamLoginRequestToAuthServer,
    LoggedIn,
    LoginFailed,
}

#[derive(Resource)]
pub struct LoginReceiverResource {
    pub receiver: tokio::sync::oneshot::Receiver<Result<serde_json::Value, String>>,
}

fn try_steam_login(
    mut commands: Commands,
    tokio_runtime: Res<shared::tokio_udp::TokioRuntimeResource>,
    clap_args: Res<crate::ClapArgs>,
    client: Res<bevy_steamworks::Client>,
    mut next_steam_login_state: ResMut<NextState<SteamLoginState>>,
) {
    let (oneshot_tx, oneshot_rx) = tokio::sync::oneshot::channel();
    commands.insert_resource(LoginReceiverResource {
        receiver: oneshot_rx,
    });

    let app_owner = client.apps().app_owner();
    let steam_id = format!("{:?}", app_owner);

    let login_server = clap_args.login_server_steam.to_string();
    tokio_runtime.spawn(async move {
        let cl = reqwest::Client::new();
        let req = cl
            .post(&login_server)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&serde_json::json!({
                "steam_player_id": steam_id,
            }));

        info!("Sent Steam login request to auth server {}", login_server);

        let res_val = req.send().await;

        let resp = match res_val {
            Ok(resp) => resp,
            Err(_e) => {
                // TODO retry
                return;
            }
        };

        let text = resp.text().await.unwrap();
        let res_val = serde_json::from_str::<serde_json::Value>(&text.clone())
            .map_err(|e| format!("Failed to parse Steam login response JSON: {}", e));
        info!("Received Steam login response from auth server: {:?}", text);

        oneshot_tx.send(res_val).unwrap();
    });

    next_steam_login_state.set(SteamLoginState::SentSteamLoginRequestToAuthServer);
}

// TODO rewrite this to be a bit cleaner
fn check_steam_login_response(
    mut commands: Commands,
    mut next_steam_login_state: ResMut<NextState<SteamLoginState>>,
    mut login_receiver_res: ResMut<LoginReceiverResource>,
) {
    println!("Checking Steam login response...");
    if let Ok(res_val) = login_receiver_res.receiver.try_recv() {
        if let Err(e) = res_val {
            error!("Error receiving Steam login response: {}", e);
            next_steam_login_state.set(SteamLoginState::LoginFailed);
            commands.remove_resource::<LoginReceiverResource>();
            return;
        }

        let res_val = res_val.unwrap();

        info!("Received Steam login response: {:?}", res_val);
        if res_val.get("success").and_then(|v| v.as_bool()) == Some(true) {
            info!("Steam login successful!");
            next_steam_login_state.set(SteamLoginState::LoggedIn);
            let login_token = res_val
                .get("login_token")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let player_id = res_val
                .get("player_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            commands.insert_resource(crate::login::LoginServerResource {
                player_id: PlayerId(player_id),
                temp_auth_token: login_token.unwrap_or_default(),
            });
        } else {
            error!("Steam login failed!");
            next_steam_login_state.set(SteamLoginState::LoginFailed);
        }
        commands.remove_resource::<LoginReceiverResource>();
    }
}
