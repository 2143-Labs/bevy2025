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
                .load_collection::<ImageAssets>()
                .load_collection::<FontAssets>()
                .load_collection::<ModelAssets>(),
        );
    }
}


#[derive(AssetCollection, Resource)]
pub struct ImageAssets {
    #[asset(path = "images/Logo.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub logo: Handle<Image>,
}

#[derive(AssetCollection, Resource)]
pub struct FontAssets {
    #[asset(path = "fonts/PTSans-Regular.ttf")]
    pub regular: Handle<Font>,
}

#[derive(AssetCollection, Resource)]
pub struct ModelAssets {
    #[asset(path = "models/g-toilet/scene.gltf")]
    pub g_toilet: Handle<Gltf>,

    // If you want to access specific scenes within the GLTF:
    #[asset(path = "models/g-toilet/scene.gltf#Scene0")]
    pub g_toilet_scene: Handle<Scene>,
}
