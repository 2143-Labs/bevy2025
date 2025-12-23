use crate::game_state::GameState;
use crate::inventory::{Inventory, Item, ItemId};
use crate::player::Player;
use crate::ui::styles::heading_text;
use bevy::prelude::*;
use shared::{Config, GameAction};

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryVisible>()
            .init_resource::<DragState>()
            .add_systems(OnEnter(GameState::Playing), spawn_inventory_ui)
            .add_systems(
                OnExit(GameState::Playing),
                (despawn_inventory_ui, close_inventory_on_state_exit),
            )
            .add_systems(
                Update,
                (
                    handle_inventory_toggle,
                    update_inventory_visibility,
                    initialize_inventory_display,
                    update_inventory_display,
                    handle_cell_interaction,
                    handle_rotation_input,
                    update_drag_visual_position,
                )
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

/// Marker component for the inventory UI container
#[derive(Component)]
pub struct InventoryUI;

/// Marker for the inventory grid container
#[derive(Component)]
pub struct InventoryGrid;

/// Marker component for individual grid cells
#[derive(Component, Clone, Copy)]
pub struct GridCell {
    pub x: usize,
    pub y: usize,
}

/// Marker for item visuals rendered in the inventory
#[derive(Component)]
pub struct ItemVisual {
    pub item_id: ItemId,
}

/// Marker component for the dragged item visual that follows the cursor
#[derive(Component)]
pub struct DraggedItemVisual;

/// Resource to track inventory visibility
#[derive(Resource, Default)]
pub struct InventoryVisible(pub bool);

/// Resource to track drag and drop state
#[derive(Resource, Default)]
pub struct DragState {
    pub dragging: bool,
    pub dragged_item_id: Option<ItemId>,
    pub dragged_item: Option<Item>,
    pub original_position: Option<(usize, usize)>,
    pub hover_cell: Option<(usize, usize)>,
}

/// System to handle Tab key input to toggle inventory
pub fn handle_inventory_toggle(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut inventory_visible: ResMut<InventoryVisible>,
    config: Res<Config>,
) {
    if config.just_pressed(&keyboard, GameAction::Inventory) {
        inventory_visible.0 = !inventory_visible.0;
    }
}

/// Spawn the inventory UI with 10x8 grid
pub fn spawn_inventory_ui(mut commands: Commands) {
    // Main container - full screen overlay
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
            Visibility::Hidden, // Start hidden
            InventoryUI,
        ))
        .with_children(|parent| {
            // Inner container - 50% width
            parent
                .spawn((
                    Node {
                        width: Val::Percent(50.0),
                        max_height: Val::Percent(70.0),
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

                    // Inventory grid container (10 wide x 8 high)
                    parent
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                display: Display::Grid,
                                grid_template_columns: vec![GridTrack::flex(1.0); 10],
                                grid_template_rows: vec![GridTrack::flex(1.0); 8],
                                row_gap: Val::Px(0.0),
                                column_gap: Val::Px(0.0),
                                ..default()
                            },
                            InventoryGrid,
                        ))
                        .with_children(|parent| {
                            // Spawn 80 grid cells (10 x 8)
                            for y in 0..8 {
                                for x in 0..10 {
                                    parent.spawn((
                                        Node {
                                            width: Val::Percent(100.0),
                                            height: Val::Percent(100.0),
                                            aspect_ratio: Some(1.0), // Square cells
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(Val::Px(1.0)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.8)),
                                        BorderColor::all(Color::srgb(0.4, 0.4, 0.4)),
                                        Interaction::None,
                                        GridCell { x, y },
                                    ));
                                }
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
    for entity in inventory_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Close inventory when exiting Playing state
fn close_inventory_on_state_exit(
    mut commands: Commands,
    mut inventory_visible: ResMut<InventoryVisible>,
    mut drag_state: ResMut<DragState>,
    drag_visual_query: Query<Entity, With<DraggedItemVisual>>,
) {
    inventory_visible.0 = false;

    drag_state.dragging = false;
    drag_state.dragged_item_id = None;
    drag_state.dragged_item = None;
    drag_state.original_position = None;
    drag_state.hover_cell = None;

    for entity in drag_visual_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Update inventory UI visibility based on InventoryVisible resource
pub fn update_inventory_visibility(
    inventory_visible: Res<InventoryVisible>,
    mut inventory_query: Query<&mut Visibility, With<InventoryUI>>,
) {
    if !inventory_visible.is_changed() {
        return;
    }

    for mut visibility in inventory_query.iter_mut() {
        *visibility = if inventory_visible.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Initialize inventory display when UI is first spawned
pub fn initialize_inventory_display(
    mut commands: Commands,
    player_inventory_query: Query<&Inventory, With<Player>>,
    ui_query: Query<Entity, Added<InventoryUI>>,
    grid_query: Query<Entity, With<InventoryGrid>>,
    existing_visuals: Query<Entity, With<ItemVisual>>,
) {
    if ui_query.is_empty() {
        return;
    }

    // Despawn any existing item visuals
    for entity in existing_visuals.iter() {
        commands.entity(entity).despawn();
    }

    let Ok(inventory) = player_inventory_query.single() else {
        return;
    };

    let Ok(grid_entity) = grid_query.single() else {
        return;
    };

    // Spawn item visuals
    spawn_item_visuals(&mut commands, inventory, grid_entity);
}

/// Update inventory display based on inventory changes
pub fn update_inventory_display(
    mut commands: Commands,
    player_inventory_query: Query<&Inventory, (With<Player>, Changed<Inventory>)>,
    grid_query: Query<Entity, With<InventoryGrid>>,
    mut cell_query: Query<(&GridCell, &mut BackgroundColor, &mut BorderColor)>,
    existing_visuals: Query<Entity, With<ItemVisual>>,
) {
    let Ok(inventory) = player_inventory_query.single() else {
        return;
    };

    let Ok(grid_entity) = grid_query.single() else {
        return;
    };

    // Despawn existing item visuals
    for entity in existing_visuals.iter() {
        commands.entity(entity).despawn();
    }

    // Update cell visuals based on occupancy
    for (cell, mut bg_color, mut border_color) in cell_query.iter_mut() {
        if inventory.get_item_id_at(cell.x, cell.y).is_some() {
            // Cell is occupied - hide border
            bg_color.0 = Color::srgba(0.3, 0.3, 0.3, 0.9);
            *border_color = BorderColor::all(Color::NONE);
        } else {
            // Cell is empty - show border
            bg_color.0 = Color::srgba(0.2, 0.2, 0.2, 0.8);
            *border_color = BorderColor::all(Color::srgb(0.4, 0.4, 0.4));
        }
    }

    // Spawn item visuals
    spawn_item_visuals(&mut commands, inventory, grid_entity);
}

/// Helper function to spawn item visuals over the grid
fn spawn_item_visuals(commands: &mut Commands, inventory: &Inventory, grid_entity: Entity) {
    // Calculate cell size as percentage
    let cell_width_percent = 100.0 / inventory.width as f32;
    let cell_height_percent = 100.0 / inventory.height as f32;

    // Track which items we've already rendered (only render each item once at its anchor)
    let mut rendered_items = std::collections::HashSet::new();

    for y in 0..inventory.height {
        for x in 0..inventory.width {
            if let Some(item_id) = inventory.get_item_id_at(x, y) {
                // Check if this is the anchor cell
                if let Some((anchor_x, anchor_y)) = inventory.get_item_anchor(item_id) {
                    if anchor_x == x && anchor_y == y && !rendered_items.contains(&item_id) {
                        rendered_items.insert(item_id);

                        if let Some(item) = inventory.get_item(item_id) {
                            let occupied_cells = item.current_occupied_cells();

                            // Calculate bounding box
                            let max_x = occupied_cells.iter().map(|(x, _)| *x).max().unwrap_or(0);
                            let max_y = occupied_cells.iter().map(|(_, y)| *y).max().unwrap_or(0);

                            let item_width_percent = (max_x + 1) as f32 * cell_width_percent;
                            let item_height_percent = (max_y + 1) as f32 * cell_height_percent;

                            // Find top-left occupied cell for text positioning (first in reading order)
                            let top_left_cell = occupied_cells
                                .iter()
                                .min_by_key(|(x, y)| (*y, *x))
                                .copied()
                                .unwrap_or((0, 0));

                            // Spawn item visual as child of grid
                            commands.entity(grid_entity).with_children(|parent| {
                                parent
                                    .spawn((
                                        Node {
                                            position_type: PositionType::Absolute,
                                            left: Val::Percent(
                                                anchor_x as f32 * cell_width_percent,
                                            ),
                                            top: Val::Percent(
                                                anchor_y as f32 * cell_height_percent,
                                            ),
                                            width: Val::Percent(item_width_percent),
                                            height: Val::Percent(item_height_percent),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            ..default()
                                        },
                                        ImageNode {
                                            image: item.texture_2d.clone(),
                                            ..default()
                                        },
                                        ItemVisual { item_id },
                                    ))
                                    .with_children(|item_parent| {
                                        // Add text at top-left occupied cell with black outline
                                        let text_left_percent =
                                            top_left_cell.0 as f32 * cell_width_percent;
                                        let text_top_percent =
                                            top_left_cell.1 as f32 * cell_height_percent;

                                        // Black outline (shadow)
                                        for (offset_x, offset_y) in
                                            [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                                        {
                                            item_parent.spawn((
                                                Node {
                                                    position_type: PositionType::Absolute,
                                                    left: Val::Percent(text_left_percent),
                                                    top: Val::Percent(text_top_percent),
                                                    padding: UiRect {
                                                        left: Val::Px(2.0 + offset_x),
                                                        top: Val::Px(2.0 + offset_y),
                                                        right: Val::Px(2.0),
                                                        bottom: Val::Px(2.0),
                                                    },
                                                    ..default()
                                                },
                                                Text::new(&item.name),
                                                TextFont {
                                                    font_size: 12.0,
                                                    ..default()
                                                },
                                                TextColor(Color::srgba(0.0, 0.0, 0.0, 1.0)),
                                            ));
                                        }

                                        // White text on top
                                        item_parent.spawn((
                                            Node {
                                                position_type: PositionType::Absolute,
                                                left: Val::Percent(text_left_percent),
                                                top: Val::Percent(text_top_percent),
                                                padding: UiRect::all(Val::Px(2.0)),
                                                ..default()
                                            },
                                            Text::new(&item.name),
                                            TextFont {
                                                font_size: 12.0,
                                                ..default()
                                            },
                                            TextColor(Color::srgba(1.0, 1.0, 1.0, 1.0)),
                                        ));
                                    });
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Handle R key for rotation during drag
fn handle_rotation_input(keyboard: Res<ButtonInput<KeyCode>>, mut drag_state: ResMut<DragState>) {
    if drag_state.dragging && keyboard.just_pressed(KeyCode::KeyR) {
        if let Some(ref mut item) = drag_state.dragged_item {
            item.rotate();
        }
    }
}

/// Handle grid cell interactions for drag and drop
fn handle_cell_interaction(
    mut commands: Commands,
    cell_query: Query<(&Interaction, &GridCell)>,
    mut all_cells_query: Query<(&GridCell, &mut BackgroundColor, &mut BorderColor)>,
    mut drag_state: ResMut<DragState>,
    mut inventory_query: Query<&mut Inventory, With<Player>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    drag_visual_query: Query<Entity, With<DraggedItemVisual>>,
) {
    // Start drag on mouse press
    if mouse_button.just_pressed(MouseButton::Left) {
        for (interaction, cell) in cell_query.iter() {
            if *interaction == Interaction::Pressed {
                if let Ok(inventory) = inventory_query.single() {
                    if let Some(item_id) = inventory.get_item_id_at(cell.x, cell.y) {
                        if let Some(anchor_pos) = inventory.get_item_anchor(item_id) {
                            if let Some(item) = inventory.get_item(item_id) {
                                drag_state.dragging = true;
                                drag_state.dragged_item_id = Some(item_id);
                                drag_state.dragged_item = Some(item.clone());
                                drag_state.original_position = Some(anchor_pos);

                                // Calculate item size for drag visual
                                let item_width = item.current_width();
                                let item_height = item.current_height();
                                let cell_size = 64.0; // Base cell size
                                let visual_width = item_width as f32 * cell_size;
                                let visual_height = item_height as f32 * cell_size;

                                // Spawn drag visual scaled to item size
                                if let Ok(window) = windows.single() {
                                    if let Some(cursor_pos) = window.cursor_position() {
                                        commands.spawn((
                                            Node {
                                                position_type: PositionType::Absolute,
                                                left: Val::Px(cursor_pos.x - visual_width / 2.0),
                                                top: Val::Px(cursor_pos.y - visual_height / 2.0),
                                                width: Val::Px(visual_width),
                                                height: Val::Px(visual_height),
                                                ..default()
                                            },
                                            ImageNode {
                                                image: item.texture_2d.clone(),
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.7)),
                                            ZIndex(1000),
                                            DraggedItemVisual,
                                        ));
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Update cell highlighting during drag
    if drag_state.dragging {
        if let Some(item) = &drag_state.dragged_item {
            if let Ok(inventory) = inventory_query.single() {
                // Find hovered cell
                let mut hover_pos: Option<(usize, usize)> = None;
                for (interaction, cell) in cell_query.iter() {
                    if *interaction == Interaction::Hovered {
                        hover_pos = Some((cell.x, cell.y));
                        break;
                    }
                }

                // Update all cell colors
                for (cell, mut bg_color, mut border_color) in all_cells_query.iter_mut() {
                    let cell_item_id = inventory.get_item_id_at(cell.x, cell.y);

                    // Reset to default colors first
                    if cell_item_id.is_some() {
                        bg_color.0 = Color::srgba(0.3, 0.3, 0.3, 0.9);
                        *border_color = BorderColor::all(Color::NONE);
                    } else {
                        bg_color.0 = Color::srgba(0.2, 0.2, 0.2, 0.8);
                        *border_color = BorderColor::all(Color::srgb(0.4, 0.4, 0.4));
                    }

                    // Highlight cells that would be occupied by the dragged item
                    if let Some((hover_x, hover_y)) = hover_pos {
                        let occupied_cells = item.current_occupied_cells();

                        for (dx, dy) in &occupied_cells {
                            if let (Some(target_x), Some(target_y)) =
                                (hover_x.checked_add(*dx), hover_y.checked_add(*dy))
                            {
                                if target_x == cell.x && target_y == cell.y {
                                    // Check if placement would be valid
                                    let can_place =
                                        if let Some(dragged_id) = drag_state.dragged_item_id {
                                            let mut temp_inventory = inventory.clone();
                                            temp_inventory.remove_item(dragged_id);
                                            temp_inventory.can_place_item(item, hover_x, hover_y)
                                        } else {
                                            inventory.can_place_item(item, hover_x, hover_y)
                                        };

                                    bg_color.0 = if can_place {
                                        Color::srgba(0.3, 0.7, 0.3, 0.9) // Green for valid
                                    } else {
                                        Color::srgba(0.7, 0.3, 0.3, 0.9) // Red for invalid
                                    };
                                    *border_color = BorderColor::all(Color::NONE);
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        // Not dragging - ensure all cells have correct default colors
        if let Ok(inventory) = inventory_query.single() {
            for (cell, mut bg_color, mut border_color) in all_cells_query.iter_mut() {
                let cell_item_id = inventory.get_item_id_at(cell.x, cell.y);

                if cell_item_id.is_some() {
                    bg_color.0 = Color::srgba(0.3, 0.3, 0.3, 0.9);
                    *border_color = BorderColor::all(Color::NONE);
                } else {
                    bg_color.0 = Color::srgba(0.2, 0.2, 0.2, 0.8);
                    *border_color = BorderColor::all(Color::srgb(0.4, 0.4, 0.4));
                }
            }
        }
    }

    // End drag on mouse release
    if mouse_button.just_released(MouseButton::Left) && drag_state.dragging {
        // Find hovered cell
        let mut drop_pos: Option<(usize, usize)> = None;
        for (interaction, cell) in cell_query.iter() {
            if *interaction == Interaction::Hovered || *interaction == Interaction::Pressed {
                drop_pos = Some((cell.x, cell.y));
                break;
            }
        }

        if let (Some((drop_x, drop_y)), Some(item_id), Some(item), Some(original_pos)) = (
            drop_pos,
            drag_state.dragged_item_id,
            &drag_state.dragged_item,
            drag_state.original_position,
        ) {
            if let Ok(mut inventory) = inventory_query.single_mut() {
                // Remove the item from inventory
                inventory.remove_item(item_id);

                // Try to place at new position
                if !inventory.place_item(item.clone(), drop_x, drop_y).is_some() {
                    // Can't place at target, restore to original position
                    inventory.place_item(item.clone(), original_pos.0, original_pos.1);
                }
            }
        }

        // Clean up drag state
        for entity in drag_visual_query.iter() {
            commands.entity(entity).despawn();
        }

        drag_state.dragging = false;
        drag_state.dragged_item_id = None;
        drag_state.dragged_item = None;
        drag_state.original_position = None;
        drag_state.hover_cell = None;
    }
}

/// Update the position of the dragged item visual to follow the cursor
fn update_drag_visual_position(
    windows: Query<&Window>,
    mut drag_visual_query: Query<&mut Node, With<DraggedItemVisual>>,
    drag_state: Res<DragState>,
) {
    if !drag_state.dragging {
        return;
    }

    if let Some(item) = &drag_state.dragged_item {
        if let Ok(window) = windows.single() {
            if let Some(cursor_pos) = window.cursor_position() {
                let item_width = item.current_width();
                let item_height = item.current_height();
                let cell_size = 64.0;
                let visual_width = item_width as f32 * cell_size;
                let visual_height = item_height as f32 * cell_size;

                for mut node in drag_visual_query.iter_mut() {
                    node.left = Val::Px(cursor_pos.x - visual_width / 2.0);
                    node.top = Val::Px(cursor_pos.y - visual_height / 2.0);
                }
            }
        }
    }
}
