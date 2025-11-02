use bevy::app::{App, Plugin};

#[cfg(feature = "inspector")]
use bevy::prelude::*;

#[cfg(feature = "inspector")]
use bevy_inspector_egui::{
    quick::WorldInspectorPlugin,
    bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass},
    egui,
};

pub struct DebugPlugin;

#[cfg(feature = "inspector")]
#[derive(Resource, Default)]
struct EguiReady(bool);

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "inspector")]
        {

            app.add_plugins(EguiPlugin::default())
                .add_plugins(WorldInspectorPlugin::new())
                .init_resource::<EguiReady>()
                .add_systems(First, mark_egui_ready)
                .add_systems(EguiPrimaryContextPass, visibility_toggle_ui.run_if(|ready: Res<EguiReady>| ready.0));
        }

        #[cfg(not(feature = "inspector"))]
        {
            // Debug plugin is disabled - inspector feature not enabled
            let _ = app;
        }
    }
}

/// Mark egui as ready after a short delay to allow initialization
#[cfg(feature = "inspector")]
fn mark_egui_ready(
    mut ready: ResMut<EguiReady>,
    mut frame_count: Local<u32>,
) {
    if !ready.0 {
        *frame_count += 1;
        // Wait a few frames for egui to initialize
        if *frame_count > 3 {
            ready.0 = true;
        }
    }
}

/// Tracks which entities are expanded in the tree view
#[cfg(feature = "inspector")]
#[derive(Resource, Default)]
struct TreeState {
    expanded: std::collections::HashSet<Entity>,
}

/// UI system to toggle visibility of entities with names
#[cfg(feature = "inspector")]
fn visibility_toggle_ui(
    mut contexts: EguiContexts,
    mut query: Query<(Entity, Option<&Name>, &mut Visibility, Option<&Children>)>,
    parent_query: Query<&ChildOf>,
    children_query: Query<&Children>,
    mut tree_state: Local<Option<TreeState>>,
) {
    // Get the context - return early if not available
    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    // Initialize tree state
    let tree_state = tree_state.get_or_insert_with(TreeState::default);

    // Collect all entities with their hierarchy info
    let entity_data: Vec<_> = query
        .iter()
        .map(|(entity, name, visibility, children)| {
            let parent = parent_query.get(entity).ok().map(|p| p.0);
            (
                entity,
                name.map(|n| n.as_str().to_string()),
                *visibility != Visibility::Hidden,
                children.map(|c| c.iter().collect::<Vec<_>>()),
                parent,
            )
        })
        .collect();

    // Find root entities (those without parents or whose parents don't have names)
    let root_entities: Vec<_> = entity_data
        .iter()
        .filter(|(_, name, _, _, parent)| {
            name.is_some()
                && (parent.is_none()
                    || !entity_data
                        .iter()
                        .any(|(e, n, _, _, _)| Some(*e) == *parent && n.is_some()))
        })
        .collect();

    let mut changes: Vec<(Entity, bool)> = Vec::new();
    let mut show_all = false;
    let mut hide_all = false;

    let window_response = egui::Window::new("Entity Visibility")
        .id(egui::Id::new("entity_visibility_window"))
        .default_pos([10.0, 400.0])
        .default_size([300.0, 500.0])
        .resizable(true)
        .movable(true)
        .collapsible(true)
        .show(ctx, |ui| {
            ui.label(format!("Entities found: {}", root_entities.len()));

            ui.horizontal(|ui| {
                if ui.button("Show All").clicked() {
                    println!("Show All clicked!");
                    show_all = true;
                }
                if ui.button("Hide All").clicked() {
                    println!("Hide All clicked!");
                    hide_all = true;
                }
            });

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (entity, name, is_visible, children, _) in &root_entities {
                    if let Some(name_str) = name {
                        render_entity_tree(
                            ui,
                            *entity,
                            name_str,
                            *is_visible,
                            children,
                            &entity_data,
                            tree_state,
                            &mut changes,
                        );
                    }
                }
            });
        });

    // Apply changes after UI is done
    for (entity, is_visible) in changes {
        if let Ok((_, _, mut visibility, children)) = query.get_mut(entity) {
            *visibility = if is_visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };

            // Toggle children recursively
            if let Some(children) = children {
                let children_list: Vec<Entity> = children.iter().collect();
                toggle_children_visibility_list(&children_list, &children_query, &mut query, is_visible);
            }
        }
    }

    if show_all {
        for (_, _, mut visibility, _) in query.iter_mut() {
            *visibility = Visibility::Inherited;
        }
    }
    if hide_all {
        for (_, _, mut visibility, _) in query.iter_mut() {
            *visibility = Visibility::Hidden;
        }
    }
}

/// Render an entity and its children in a tree structure
#[cfg(feature = "inspector")]
fn render_entity_tree(
    ui: &mut egui::Ui,
    entity: Entity,
    name: &str,
    is_visible: bool,
    children: &Option<Vec<Entity>>,
    all_entities: &[(Entity, Option<String>, bool, Option<Vec<Entity>>, Option<Entity>)],
    tree_state: &mut TreeState,
    changes: &mut Vec<(Entity, bool)>,
) {
    let has_children = children.as_ref().map(|c| !c.is_empty()).unwrap_or(false);
    let is_expanded = tree_state.expanded.contains(&entity);

    ui.horizontal(|ui| {
        // Show expand/collapse arrow if entity has children
        if has_children {
            let arrow = if is_expanded { "▼" } else { "▶" };
            if ui.small_button(arrow).clicked() {
                if is_expanded {
                    tree_state.expanded.remove(&entity);
                } else {
                    tree_state.expanded.insert(entity);
                }
            }
        } else {
            ui.add_space(20.0); // Indent for entities without children
        }

        // Visibility checkbox
        let mut current_visible = is_visible;
        let checkbox_response = ui.checkbox(&mut current_visible, name);
        if checkbox_response.changed() {
            println!("Checkbox changed for {}: {}", name, current_visible);
            changes.push((entity, current_visible));
        }

        ui.weak(format!("{:?}", entity));
    });

    // Render children if expanded
    if is_expanded && has_children {
        if let Some(child_list) = children {
            ui.indent(entity, |ui| {
                for &child_entity in child_list {
                    // Find child data
                    if let Some((_, child_name, child_visible, child_children, _)) =
                        all_entities.iter().find(|(e, _, _, _, _)| *e == child_entity)
                    {
                        if let Some(child_name_str) = child_name {
                            render_entity_tree(
                                ui,
                                child_entity,
                                child_name_str,
                                *child_visible,
                                child_children,
                                all_entities,
                                tree_state,
                                changes,
                            );
                        }
                    }
                }
            });
        }
    }
}

/// Recursively toggle visibility of children from a list of entity IDs
#[cfg(feature = "inspector")]
fn toggle_children_visibility_list(
    children: &[Entity],
    children_query: &Query<&Children>,
    visibility_query: &mut Query<(Entity, Option<&Name>, &mut Visibility, Option<&Children>)>,
    is_visible: bool,
) {
    for &child in children {
        // Update this child's visibility
        if let Ok((_, _, mut visibility, _)) = visibility_query.get_mut(child) {
            *visibility = if is_visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }

        // Recursively toggle this child's children
        if let Ok(child_children) = children_query.get(child) {
            let grandchildren: Vec<Entity> = child_children.iter().collect();
            toggle_children_visibility_list(
                &grandchildren,
                children_query,
                visibility_query,
                is_visible,
            );
        }
    }
}