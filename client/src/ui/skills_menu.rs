use crate::{
    assets::get_skill_icon, game_state::OverlayMenuState, network::CurrentThirdPersonControlledUnit, ui::{
        skills_menu::binds::SkillBindOverlayState,
        styles::{menu_button_bundle},
    }
};

use bevy::prelude::*;
use shared::{
    camel_to_normalized, items::{InventoryItemCache, SkillFromSkillSource}, net_components::ours::HasInventory, skills::SkillSource
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

/// Marker for skill tooltip
#[derive(Component)]
pub struct SkillTooltip;

/// Marker for skill tooltip text
#[derive(Component)]
pub struct SkillTooltipText;

/// Resource to track current tooltip state
#[derive(Resource)]
pub struct TooltipState {
    pub current_skill: Option<SkillFromSkillSource>,
    pub tooltip_entity: Option<Entity>,
}

pub struct SkillsMenuPlugin;

impl Plugin for SkillsMenuPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TooltipState {
                current_skill: None,
                tooltip_entity: None,
            })
            .add_systems(OnEnter(OverlayMenuState::Skills), spawn_skills_menu)
            .add_systems(OnExit(OverlayMenuState::Skills), despawn_skills_menu)
            .add_systems(
                Update,
                (handle_skills_menu_buttons, update_skills_menu, handle_skill_tooltip)
                    .run_if(in_state(OverlayMenuState::Skills)),
            );
    }
}

const GAP: f32 = 10.0;

// Tooltip styling constants
const TOOLTIP_BORDER_COLOR: Color = Color::srgb(0.541, 0.435, 0.188); // #8a6f30
const TOOLTIP_BACKGROUND_COLOR: Color = Color::srgb(0.424, 0.314, 0.161); // #6c5029
const TOOLTIP_TEXT_COLOR: Color = Color::WHITE;

// send a packet and spawn loading screen
pub fn spawn_skills_menu(
    mut commands: Commands,
    current_char: Query<&HasInventory, With<CurrentThirdPersonControlledUnit>>,
    inventory_map: Res<InventoryItemCache>,
    images: Res<crate::assets::ImageAssets>,
) {
    info!("Spawning skills menu");
    
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
    
    // Calculate grid size based on number of skills (next perfect square)
    let skill_count = skills.len();
    let grid_size = ((skill_count as f32).sqrt().ceil() as usize).max(1);
    info!("Grid size for {} skills: {}x{}", skill_count, grid_size, grid_size);

    // spawn outer container with paper background
    let mut skills_menu_ent = commands.spawn((
        SkillsMenu,
        Node {
            width: Val::Vh(80.0), // Square: same as height (80vh)
            height: Val::Vh(80.0), // 80% of viewport height
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            margin: UiRect {
                left: Val::Auto,
                right: Val::Auto,
                top: Val::Vh(10.0), // Center vertically with some top margin
                bottom: Val::Auto,
            },
            ..default()
        },
        ImageNode {
            image: images.paper.clone(),
            ..default()
        },
    ));

    // Create inner skills grid container (takes 2/3 of paper space)
    skills_menu_ent.with_children(|paper_parent| {
        let mut skills_grid = paper_parent.spawn((
            Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::flex(grid_size as u16, 1.0),
                grid_template_rows: RepeatedGridTrack::flex(grid_size as u16, 1.0),
                column_gap: Val::Px(GAP),
                row_gap: Val::Px(GAP),
                width: Val::Percent(66.7), // 2/3 of paper width
                height: Val::Percent(66.7), // 2/3 of paper height
                justify_self: JustifySelf::Center,
                align_self: AlignSelf::Center,
                ..default()
            },
        ));

        // Spawn a button for each skill
        for equipped_skill in &skills {
            let skill_icon = get_skill_icon(&equipped_skill.skill, &images);
            let _skill_name = match equipped_skill.source {
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

            skills_grid.with_children(|grid_parent| {
                let (_node, _bg_color, border_color) = menu_button_bundle();
                grid_parent
                    .spawn((
                        Node{
                            width: Val::Percent(100.0), // Fill grid cell
                            height: Val::Percent(100.0), // Fill grid cell
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        border_color,
                        Interaction::default(),
                        SkillButton {
                            skill: equipped_skill.clone(),
                        },
                    ))
                    .with_children(|button| {
                        button.spawn((
                            ImageNode {
                                image: skill_icon,
                                ..default()
                            },
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                        ));
                    });
            });
        }
    });
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

pub fn handle_skill_tooltip(
    mut commands: Commands,
    skill_buttons: Query<(&Interaction, &SkillButton)>,
    skills_menu: Query<Entity, With<SkillsMenu>>,
    _inventory_map: Res<InventoryItemCache>,
    fonts: Res<crate::assets::FontAssets>,
    mut cursor_events: MessageReader<CursorMoved>,
    mut tooltip_state: ResMut<TooltipState>,
) {
    let cursor_pos = cursor_events.read().last().map(|e| e.position);
    let mut currently_hovered_skill: Option<SkillFromSkillSource> = None;

    // Check what skill is currently being hovered
    for (interaction, skill_button) in skill_buttons.iter() {
        if *interaction == Interaction::Hovered {
            currently_hovered_skill = Some(skill_button.skill.clone());
            break;
        }
    }

    // Check if tooltip state has changed
    let tooltip_changed = tooltip_state.current_skill != currently_hovered_skill;

    if tooltip_changed {
        // Remove existing tooltip if it exists
        if let Some(existing_tooltip) = tooltip_state.tooltip_entity {
            if let Ok(mut entity_commands) = commands.get_entity(existing_tooltip) {
                entity_commands.despawn();
            }
            tooltip_state.tooltip_entity = None;
        }

        // Create new tooltip if hovering over a skill
        if let (Some(hovered_skill), Some(cursor_position), Ok(_skills_menu_entity)) = 
            (currently_hovered_skill.as_ref(), cursor_pos, skills_menu.single()) {
            
            // Spawn new tooltip directly as a root UI element
            let mut tooltip_cmd = commands.spawn((
                SkillTooltip,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(cursor_position.x + 10.0),
                    top: Val::Px(cursor_position.y - 40.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(TOOLTIP_BACKGROUND_COLOR),
            ));
            
            tooltip_cmd.insert(BorderColor::all(TOOLTIP_BORDER_COLOR));
            tooltip_cmd.insert(ZIndex(1000));
            
            let tooltip_entity = tooltip_cmd.with_children(|tooltip_parent| {
                tooltip_parent.spawn((
                    Text::new(camel_to_normalized(&format!("{:?}",hovered_skill.skill))),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(TOOLTIP_TEXT_COLOR),
                    SkillTooltipText,
                ));
            }).id();

            tooltip_state.tooltip_entity = Some(tooltip_entity);
        }

        // Update current skill state
        tooltip_state.current_skill = currently_hovered_skill;
    }
}

