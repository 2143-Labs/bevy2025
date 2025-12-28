// this is a template
use crate::game_state::OverlayMenuState;

use bevy::prelude::*;

/// Marker for the paused menu root entity
#[derive(Component)]
pub struct SkillsMenu;

pub struct SkillsMenuPlugin;

impl Plugin for SkillsMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(OverlayMenuState::Skills),
            spawn_skills_menu,
        )
        .add_systems(
            OnExit(OverlayMenuState::Skills),
            despawn_skills_menu,
        )
        .add_systems(
            Update,
            (
                handle_skills_menu_buttons,
                update_skills_menu,
            )
                .run_if(in_state(OverlayMenuState::Skills)),
        );
    }
}

// send a packet and spawn loading screen
pub fn spawn_skills_menu(mut commands: Commands) {
    info!("Spawning skills menu");
    // spawn loading...
}


pub fn update_skills_menu(
) {
}

pub fn despawn_skills_menu(
    mut commands: Commands,
    menu_query: Query<Entity, With<SkillsMenu>>
) {
    info!("Despawning skills menu");
    for menu_entity in menu_query.iter() {
        commands.entity(menu_entity).despawn();
    }
}

pub fn handle_skills_menu_buttons() {}
