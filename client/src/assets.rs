use bevy::prelude::*;
use bevy_asset_loader::prelude::*;


use crate::game_state::GameState;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app
        .add_loading_state(
            LoadingState::new(GameState::AssetLoading)
                .continue_to_state(GameState::MainMenu)
                .load_collection::<ImageAssets>(),
        );
    }
}


#[derive(AssetCollection, Resource)]
pub struct ImageAssets {
    #[asset(path = "images/Logo.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub logo: Handle<Image>,
}
