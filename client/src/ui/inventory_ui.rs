use bevy::prelude::*;
use crate::components::{Inventory, Item, Player};
use crate::game_state::GameState;
use crate::ui::styles::heading_text;
use shared::{Config, GameAction};

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<InventoryVisible>()
            .init_resource::<DragState>()
            .add_systems(OnEnter(GameState::Playing), spawn_inventory_ui)
            .add_systems(OnExit(GameState::Playing), despawn_inventory_ui)
            .add_systems(
                Update,
                (
                    handle_inventory_toggle,
                    update_inventory_visibility,
                    initialize_inventory_display,
                    update_inventory_slots,
                    handle_slot_interaction,
                ).run_if(in_state(GameState::Playing)),
            );
    }
}

/// Marker component for the inventory UI container
#[derive(Component)]
pub struct InventoryUI;

/// Marker component for individual inventory slots
#[derive(Component)]
pub struct InventorySlot {
    pub index: usize,
}

/// Marker component for the item image within a slot
#[derive(Component)]
pub struct SlotItemImage {
    pub slot_index: usize,
}

/// Marker component for the item name text within a slot
#[derive(Component)]
pub struct SlotItemText {
    pub slot_index: usize,
}

/// Resource to track inventory visibility
#[derive(Resource, Default)]
pub struct InventoryVisible(pub bool);

/// Resource to track drag and drop state
#[derive(Resource, Default)]
pub struct DragState {
    pub dragging: bool,
    pub source_slot: Option<usize>,
    pub dragged_item: Option<Item>,
}

/// System to handle Tab key input to toggle inventory
pub fn handle_inventory_toggle(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut inventory_visible: ResMut<InventoryVisible>,
    game_state: Res<State<GameState>>,
    config: Res<Config>,
) {
    // Only allow opening inventory in Playing state (not Paused)
    if *game_state.get() != GameState::Playing {
        return;
    }

    // Check if Tab key was just pressed using config bindings
    if config.just_pressed(&keyboard, GameAction::Inventory) {
        inventory_visible.0 = !inventory_visible.0;
        info!("Inventory toggled: {}", inventory_visible.0);
    }
}

/// Spawn the inventory UI
pub fn spawn_inventory_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    info!("Spawning inventory UI");

    // Main container - 80% of screen, centered
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(Color::NONE),
            InventoryUI,
        ))
        .with_children(|parent| {
            // Inner container - 50% width, auto height to fit content
            parent
                .spawn((
                    Node {
                        width: Val::Percent(50.0),
                        max_height: Val::Percent(60.0), // Limit height to 60% of screen
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::FlexStart,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(40.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.95)),
                    BorderColor::all(Color::srgb(0.3, 0.3, 0.3)),
                ))
                .with_children(|parent| {
                    // Title
                    parent.spawn({
                        let (text, font, color) = heading_text("INVENTORY", 48.0);
                        (
                            Node {
                                margin: UiRect::bottom(Val::Px(30.0)),
                                ..default()
                            },
                            text,
                            font,
                            color,
                        )
                    });

                    // Inventory grid container (7 wide x 3 high)
                    parent
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            aspect_ratio: Some(7.0 / 3.0), // 7:3 ratio to match grid
                            display: Display::Grid,
                            grid_template_columns: vec![GridTrack::flex(1.0); 7],
                            grid_template_rows: vec![GridTrack::flex(1.0); 3],
                            row_gap: Val::Px(10.0),
                            column_gap: Val::Px(10.0),
                            ..default()
                        })
                        .with_children(|parent| {
                            // Spawn 21 inventory slots (7 x 3)
                            for i in 0..21 {
                                parent
                                    .spawn((
                                        Node {
                                            width: Val::Percent(100.0),
                                            height: Val::Percent(100.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(Val::Px(2.0)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.8)),
                                        BorderColor::all(Color::srgb(0.4, 0.4, 0.4)),
                                        Interaction::None,
                                        InventorySlot { index: i },
                                    ))
                                    .with_children(|slot_parent| {
                                        // Item image placeholder (will be updated when item is present)
                                        slot_parent.spawn((
                                            Node {
                                                width: Val::Percent(80.0),
                                                height: Val::Percent(80.0),
                                                position_type: PositionType::Absolute,
                                                ..default()
                                            },
                                            ImageNode {
                                                image: Handle::default(),
                                                ..default()
                                            },
                                            Visibility::Hidden,
                                            SlotItemImage { slot_index: i },
                                        ));

                                        // Item name text (will be updated when item is present)
                                        slot_parent.spawn((
                                            Node {
                                                position_type: PositionType::Absolute,
                                                bottom: Val::Px(5.0),
                                                ..default()
                                            },
                                            Text::new(""),
                                            TextFont {
                                                font_size: 14.0, // Small font to fit in square cells
                                                ..default()
                                            },
                                            TextColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
                                            Visibility::Hidden,
                                            SlotItemText { slot_index: i },
                                        ));
                                    });
                            }
                        });
                });
        });
}

/// Despawn the inventory UI
pub fn despawn_inventory_ui(
    mut commands: Commands,
    inventory_query: Query<Entity, With<InventoryUI>>,
) {
    info!("Despawning inventory UI");
    for entity in inventory_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Update inventory UI visibility based on InventoryVisible resource
pub fn update_inventory_visibility(
    inventory_visible: Res<InventoryVisible>,
    player_inventory: Query<&Inventory, With<Player>>,
    mut inventory_query: Query<&mut Visibility, With<InventoryUI>>,
    mut slot_image_query: Query<(&SlotItemImage, &mut Visibility), Without<InventoryUI>>,
    mut slot_text_query: Query<(&SlotItemText, &mut Visibility), (Without<InventoryUI>, Without<SlotItemImage>)>,
) {
    // Only update if visibility changed
    if !inventory_visible.is_changed() {
        return;
    }

    // Update main container visibility
    for mut visibility in inventory_query.iter_mut() {
        *visibility = if inventory_visible.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    // Update slot item visibility based on inventory state
    if let Ok(inventory) = player_inventory.single() {
        // Update slot images
        for (slot_item_image, mut visibility) in slot_image_query.iter_mut() {
            if inventory_visible.0 && inventory.get_slot(slot_item_image.slot_index).is_some() {
                *visibility = Visibility::Inherited;
            } else {
                *visibility = Visibility::Hidden;
            }
        }

        // Update slot text
        for (slot_item_text, mut visibility) in slot_text_query.iter_mut() {
            if inventory_visible.0 && inventory.get_slot(slot_item_text.slot_index).is_some() {
                *visibility = Visibility::Inherited;
            } else {
                *visibility = Visibility::Hidden;
            }
        }
    }
}

/// Initialize inventory slot visuals when UI is first spawned
pub fn initialize_inventory_display(
    player_inventory_query: Query<&Inventory, With<Player>>,
    ui_query: Query<Entity, Added<InventoryUI>>,
    mut slot_image_query: Query<(&SlotItemImage, &mut ImageNode, &mut Visibility)>,
    mut slot_text_query: Query<(&SlotItemText, &mut Text, &mut Visibility), Without<SlotItemImage>>,
) {
    // Only run when UI is first added
    if ui_query.is_empty() {
        return;
    }

    if let Ok(inventory) = player_inventory_query.single() {
        // Update item images
        for (slot_item_image, mut image_node, mut visibility) in slot_image_query.iter_mut() {
            if let Some(item) = inventory.get_slot(slot_item_image.slot_index) {
                image_node.image = item.texture_2d.clone();
                // Items start hidden (inventory closed by default)
                *visibility = Visibility::Hidden;
            } else {
                *visibility = Visibility::Hidden;
            }
        }

        // Update item text
        for (slot_item_text, mut text, mut visibility) in slot_text_query.iter_mut() {
            if let Some(item) = inventory.get_slot(slot_item_text.slot_index) {
                text.0 = item.name.clone();
                // Items start hidden (inventory closed by default)
                *visibility = Visibility::Hidden;
            } else {
                *visibility = Visibility::Hidden;
            }
        }
    }
}

/// Update inventory slot visuals based on player inventory changes (from drag/drop)
pub fn update_inventory_slots(
    inventory_query: Query<&Inventory, (With<Player>, Changed<Inventory>)>,
    inventory_visible: Res<InventoryVisible>,
    mut slot_image_query: Query<(&SlotItemImage, &mut ImageNode, &mut Visibility)>,
    mut slot_text_query: Query<(&SlotItemText, &mut Text, &mut Visibility), Without<SlotItemImage>>,
) {
    for inventory in inventory_query.iter() {
        // Update item images
        for (slot_item_image, mut image_node, mut visibility) in slot_image_query.iter_mut() {
            if let Some(item) = inventory.get_slot(slot_item_image.slot_index) {
                image_node.image = item.texture_2d.clone();
                // Show if inventory is open
                *visibility = if inventory_visible.0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            } else {
                *visibility = Visibility::Hidden;
            }
        }

        // Update item text
        for (slot_item_text, mut text, mut visibility) in slot_text_query.iter_mut() {
            if let Some(item) = inventory.get_slot(slot_item_text.slot_index) {
                text.0 = item.name.clone();
                // Show if inventory is open
                *visibility = if inventory_visible.0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            } else {
                *visibility = Visibility::Hidden;
            }
        }
    }
}

/// Handle slot interactions for drag and drop
pub fn handle_slot_interaction(
    mut interaction_query: Query<(&Interaction, &InventorySlot, &mut BackgroundColor, &mut BorderColor)>,
    mut drag_state: ResMut<DragState>,
    mut inventory_query: Query<&mut Inventory, With<Player>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    // Start drag on mouse press
    if mouse_button.just_pressed(MouseButton::Left) {
        for (interaction, slot, _, _) in interaction_query.iter() {
            if *interaction == Interaction::Pressed {
                if let Ok(inventory) = inventory_query.single() {
                    if let Some(item) = inventory.get_slot(slot.index) {
                        // Start dragging
                        drag_state.dragging = true;
                        drag_state.source_slot = Some(slot.index);
                        drag_state.dragged_item = Some(item.clone());
                        info!("Started dragging item from slot {}", slot.index);
                    }
                }
            }
        }
    }

    // End drag on mouse release
    if mouse_button.just_released(MouseButton::Left) && drag_state.dragging {
        // Find the slot that's currently hovered or pressed
        for (interaction, slot, _, _) in interaction_query.iter() {
            if *interaction == Interaction::Hovered || *interaction == Interaction::Pressed {
                if let Some(source_index) = drag_state.source_slot {
                    if source_index != slot.index {
                        // Swap items
                        if let Ok(mut inventory) = inventory_query.single_mut() {
                            inventory.swap_slots(source_index, slot.index);
                            info!("Swapped items between slots {} and {}", source_index, slot.index);
                        }
                    }
                }
                break; // Only process the first hovered/pressed slot
            }
        }

        // Clear drag state
        info!("Ending drag");
        drag_state.dragging = false;
        drag_state.source_slot = None;
        drag_state.dragged_item = None;
    }

    // Update slot visual feedback (iterate over all slots, not just changed)
    for (interaction, slot, mut bg_color, mut border_color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                bg_color.0 = Color::srgba(0.4, 0.4, 0.4, 0.9);
                *border_color = BorderColor::all(Color::srgb(0.8, 0.8, 0.8));
            }
            Interaction::Hovered => {
                // Highlight if dragging
                if drag_state.dragging && Some(slot.index) != drag_state.source_slot {
                    bg_color.0 = Color::srgba(0.3, 0.5, 0.3, 0.9);
                    *border_color = BorderColor::all(Color::srgb(0.5, 0.8, 0.5));
                } else {
                    bg_color.0 = Color::srgba(0.3, 0.3, 0.3, 0.9);
                    *border_color = BorderColor::all(Color::srgb(0.6, 0.6, 0.6));
                }
            }
            Interaction::None => {
                // Highlight source slot while dragging
                if drag_state.dragging && Some(slot.index) == drag_state.source_slot {
                    bg_color.0 = Color::srgba(0.5, 0.3, 0.3, 0.9);
                    *border_color = BorderColor::all(Color::srgb(0.8, 0.5, 0.5));
                } else {
                    bg_color.0 = Color::srgba(0.2, 0.2, 0.2, 0.8);
                    *border_color = BorderColor::all(Color::srgb(0.4, 0.4, 0.4));
                }
            }
        }
    }
}
