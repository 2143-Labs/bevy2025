use bevy::prelude::*;

pub struct SteamworksPlugin(pub bevy_steamworks::AppId);

impl Plugin for SteamworksPlugin {
    fn build(&self, app: &mut App) {
        // Initialize Steamworks here
        app.add_plugins(bevy_steamworks::SteamworksPlugin::init_app(self.0).unwrap());
    }
}
