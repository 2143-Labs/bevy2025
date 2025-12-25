use super::styles::*;
use crate::{game_state::{GameState, OverlayMenuState}, network::CurrentThirdPersonControlledUnit};
use bevy::prelude::*;
use shared::{items::InventoryItemCache, net_components::ours::HasInventory};

/// Marker for the paused menu root entity
#[derive(Component)]
pub struct InventoryMenu;

/// Spawn the paused menu UI
pub fn spawn_inventory_menu(
    mut commands: Commands,
    our_inv: Query<
        &HasInventory,
        With<CurrentThirdPersonControlledUnit>,
    >,
    inventory_res: Res<InventoryItemCache>,
    mut next_state: ResMut<NextState<OverlayMenuState>>
) {
    let Ok(inventory) = our_inv.single() else {
        warn!("Could not get inventory for current controlled unit");
        next_state.set(OverlayMenuState::Hidden);
        return;
    };

    let inv_id = inventory.inventory_id;
    let Some(inventory_full) = inventory_res.get_inventory(&inv_id) else {
        warn!("Could not get full inventory data for inventory ID: {:?}", inv_id);
        next_state.set(OverlayMenuState::Hidden);
        return;
    };

    info!("Spawning inventory menu for inventory ID: {:?}", inv_id);

    dbg!(inventory_full.items.len());

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(30.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)), // Semi-transparent overlay
            InventoryMenu,
        ))
        .with_children(|parent| {
            // "INVENTORY" title
            parent.spawn({
                let (text, font, color) = heading_text("INVENTORY", 80.0);
                (
                    Node {
                        margin: UiRect::bottom(Val::Px(40.0)),
                        ..default()
                    },
                    text,
                    font,
                    color,
                )
            });

            // List inventory items
            // for now, just print item names into boxes
            for item in &inventory_full.items {
                let item_stacksize = item.stacksize;
                let item_placement = item.item_placement.clone();
                let base_item = format!("{:?}", item.item.data.item_base);
                let item_text = format!("{base_item} x{item_stacksize}");
                let (node, bg_color, border_color) = menu_button_bundle();
                let (text, font, color) = menu_button_text(&item_text);
                parent
                    .spawn((
                        node,
                        bg_color,
                        border_color,
                        Interaction::default(),
                    ))
                    .with_children(|button| {
                        button.spawn((text, font, color));
                    });
            }
        });
}

/// Despawn the paused menu UI
pub fn despawn_inventory_menu(
    mut commands: Commands,
    menu_query: Query<Entity, With<InventoryMenu>>,
) {
    info!("Despawning inventory menu");
    if let Ok(menu_entity) = menu_query.single() {
        commands.entity(menu_entity).despawn();
    }
}

pub fn handle_inventory_menu_buttons(
    //keyboard: Res<ButtonInput<KeyCode>>,
    //mouse: Res<ButtonInput<MouseButton>>,
    //config: Res<crate::config::Config>,
    //mut next_state: ResMut<NextState<OverlayMenuState>>,
) {
    //if config.just_pressed(&keyboard, &mouse, crate::game_action::GameAction::Escape) {
        //next_state.set(OverlayMenuState::Hidden);
    //}
}
