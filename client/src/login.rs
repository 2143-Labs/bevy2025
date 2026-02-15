use bevy::prelude::*;
use shared::event::PlayerId;

#[derive(Resource)]
#[allow(unused)]
// TODO use
pub struct LoginServerResource {
    pub player_id: PlayerId,
    pub temp_auth_token: String,
}
