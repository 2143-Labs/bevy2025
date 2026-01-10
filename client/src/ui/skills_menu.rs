use crate::{
    assets::get_skill_icon, game_state::OverlayMenuState, network::CurrentThirdPersonControlledUnit, ui::{
        skills_menu::binds::SkillBindOverlayState,
        styles::{menu_button_bundle, menu_button_text},
    }
};

use bevy::prelude::*;
use shared::{
    items::{InventoryItemCache, SkillFromSkillSource},
    net_components::ours::HasInventory,
    skills::SkillSource,
};

pub mod binds;

/// Marker for the paused menu root entity
#[derive(Component)]
pub struct SkillsMenu;

/// Marker for skill buttons
#[derive(Component)]
pub struct SkillButton {
    pub skill: SkillFromSkillSource,
}

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
    images: Res<crate::assets::ImageAssets>,
) {
    info!("Spawning skills menu");
    // spawn outer container
    let mut skills_menu_ent = commands.spawn((
        SkillsMenu,
        Node {
            display: Display::Grid,
            padding: UiRect::all(px(3)),
            width: Val::Percent(80.0),
            height: Val::Percent(80.0),
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

    let skills: Vec<SkillFromSkillSource> = inventory_full.get_equipped_skills();
    info!("Equipped skills: {:?}", skills);

    //spawn a button for each skill
    for equipped_skill in &skills {
        let skill_icon = get_skill_icon(&equipped_skill.skill, &images);
        let skill_name = match equipped_skill.source {
            SkillSource::Item(item_id) => {
                if let Some(item) = inventory_map.get_item(&item_id) {
                    format!(
                        "{:?} (from {:?})",
                        equipped_skill.skill, item.data.item_base
                    )
                } else {
                    format!("{:?} (from item)", equipped_skill.skill)
                }
            }
            _ => {
                format!("{:?}", equipped_skill.skill)
            }
        };

        skills_menu_ent.with_children(|parent| {
            let (node, bg_color, border_color) = menu_button_bundle();
            let (text, font, color) = menu_button_text(skill_name);
            parent
                .spawn((
                    ImageNode {
                        image: skill_icon,
                        ..default()
                    },
                    bg_color,
                    border_color,
                    Interaction::default(),
                    SkillButton {
                        skill: equipped_skill.clone(),
                    },
                ));
                // .with_children(|button| {
                //     button.spawn((text, font, color));
                // });
        });
    }
}

pub fn update_skills_menu() {}

pub fn despawn_skills_menu(mut commands: Commands, menu_query: Query<Entity, With<SkillsMenu>>) {
    info!("Despawning skills menu");
    for menu_entity in menu_query.iter() {
        commands.entity(menu_entity).despawn();
    }
}

pub fn handle_skills_menu_buttons(
    mut interaction_query: Query<(&Interaction, &SkillButton, Entity), Changed<Interaction>>,
    // TODO combine the current_skill_we_are_binding resource with the state
    mut overlay_menu_state: ResMut<NextState<SkillBindOverlayState>>,
    mut commands: Commands,
) {
    for (interaction, SkillButton { skill }, _entity) in interaction_query.iter_mut() {
        if *interaction == Interaction::Pressed {
            info!("Skill button pressed: {:?}", skill);
            commands.insert_resource(binds::CurrentSkillWeAreBinding {
                skill: skill.clone(),
            });
            overlay_menu_state.set(SkillBindOverlayState::Active);
        }
    }
}
