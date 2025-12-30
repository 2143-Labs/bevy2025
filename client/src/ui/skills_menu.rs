use crate::{game_state::OverlayMenuState, network::CurrentThirdPersonControlledUnit};

use bevy::prelude::*;
use shared::{items::InventoryItemCache, net_components::ours::HasInventory};

pub mod binds;

/// Marker for the paused menu root entity
#[derive(Component)]
pub struct SkillsMenu;

pub struct SkillsMenuPlugin;

impl Plugin for SkillsMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(OverlayMenuState::Skills), spawn_skills_menu)
            .add_systems(OnExit(OverlayMenuState::Skills), despawn_skills_menu)
            .add_systems(
                Update,
                (handle_skills_menu_buttons, update_skills_menu)
                    .run_if(in_state(OverlayMenuState::Skills)),
            );
    }
}

// send a packet and spawn loading screen
pub fn spawn_skills_menu(
    mut commands: Commands,
    current_char: Query<&HasInventory, With<CurrentThirdPersonControlledUnit>>,
    inventory_map: Res<InventoryItemCache>,
) {
    info!("Spawning skills menu");
    // spawn outer container
    commands.spawn((
        SkillsMenu,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(30.0),
            ..default()
        },
    ));

    let Ok(current_char_inv) = current_char.single() else {
        error!("No current character found when spawning skills menu");
        return;
    };

    let inv_id = current_char_inv.inventory_id;
    let Some(inventory_full) = inventory_map.get_inventory(&inv_id) else {
        error!(
            "Could not get full inventory data for inventory ID: {:?}",
            inv_id
        );
        return;
    };

    let skills = inventory_full.get_equipped_skills();
    info!("Equipped skills: {:?}", skills);

    //spawn a button for each skill
    for skill in skills {
        info!("Spawning skill button for skill: {:?}", skill);
    }
}

pub fn update_skills_menu() {}

pub fn despawn_skills_menu(mut commands: Commands, menu_query: Query<Entity, With<SkillsMenu>>) {
    info!("Despawning skills menu");
    for menu_entity in menu_query.iter() {
        commands.entity(menu_entity).despawn();
    }
}

pub fn handle_skills_menu_buttons() {}
