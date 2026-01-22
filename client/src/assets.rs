use bevy::prelude::*;
use bevy_asset_loader::prelude::*;

use crate::game_state::GameState;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_loading_state(
            LoadingState::new(GameState::AssetLoading)
                .continue_to_state(GameState::MainMenu)
                .load_collection::<ImageAssets>()
                .load_collection::<FontAssets>()
                .load_collection::<ModelAssets>()
                .load_collection::<WorldAssets>(),
        );
    }
}

#[derive(AssetCollection, Resource)]
pub struct ImageAssets {
    #[asset(path = "images/Logo.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub logo: Handle<Image>,

    #[asset(path = "images/Paper.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub paper: Handle<Image>,

    // Skill icons 
    #[asset(path = "images/BasicBowAttack.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub basic_bow_attack: Handle<Image>,
    #[asset(path = "images/Blink.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub blink: Handle<Image>,
    #[asset(path = "images/Heal.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub heal: Handle<Image>,
    #[asset(path = "images/HomingArrows.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub homing_arrows: Handle<Image>,
    #[asset(path = "images/IceNova.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub ice_nova: Handle<Image>,
    #[asset(path = "images/RainOfArrows.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub rain_of_arrows: Handle<Image>,
    #[asset(path = "images/Spark.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub spark: Handle<Image>,
    #[asset(path = "images/SummonTestNPC.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub summon_test_npc: Handle<Image>,
    #[asset(path = "images/TownPortal.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub town_portal: Handle<Image>,
    #[asset(path = "images/WinterOrb.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub winter_orb: Handle<Image>,
    #[asset(path = "images/Frostbolt.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub frostbolt: Handle<Image>,
    #[asset(path = "images/Hammerdin.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub hammerdin: Handle<Image>,
    #[asset(path = "images/Revive.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub revive: Handle<Image>,

    #[asset(path = "images/UnknownSkill.png")]
    #[asset(image(sampler(filter = nearest)))]
    pub unknown_skill: Handle<Image>,
}

pub fn get_skill_icon(skill: &shared::skills::Skill, images: &ImageAssets) -> Handle<Image> {
    match skill {
        shared::skills::Skill::BasicBowAttack => images.basic_bow_attack.clone(),
        shared::skills::Skill::Blink => images.blink.clone(),
        shared::skills::Skill::Heal => images.heal.clone(),
        shared::skills::Skill::HomingArrows => images.homing_arrows.clone(),
        shared::skills::Skill::IceNova => images.ice_nova.clone(),
        shared::skills::Skill::RainOfArrows => images.rain_of_arrows.clone(),
        shared::skills::Skill::Spark => images.spark.clone(),
        shared::skills::Skill::SummonTestNPC => images.summon_test_npc.clone(),
        shared::skills::Skill::TownPortal => images.town_portal.clone(),
        shared::skills::Skill::WinterOrb => images.winter_orb.clone(),
        shared::skills::Skill::Frostbolt => images.frostbolt.clone(),
        shared::skills::Skill::Hammerdin => images.hammerdin.clone(),
        shared::skills::Skill::Revive => images.revive.clone(),
        _ => images.unknown_skill.clone(),
    }
}

#[derive(AssetCollection, Resource)]
pub struct FontAssets {
    #[asset(path = "fonts/PTSans-Regular.ttf")]
    pub regular: Handle<Font>,
}

#[derive(AssetCollection, Resource)]
pub struct ModelAssets {
    //#[asset(path = "models/g-toilet/scene.gltf")]
    //pub g_toilet: Handle<Gltf>,

    // If you want to access specific scenes within the GLTF:
    #[asset(path = "models/g-toilet/scene.gltf#Scene0")]
    pub g_toilet_scene: Handle<Scene>,
}

#[derive(AssetCollection, Resource)]
pub struct WorldAssets {
    // If you want to access specific scenes within the GLTF:
    #[asset(path = "models/tower/stone-tower001.gltf#Scene0")]
    pub stone_tower: Handle<Scene>,
}
