use avian3d::prelude::LinearVelocity;
use bevy::{camera::visibility::NoFrustumCulling, prelude::*};
use shared::{
    event::{
        MyNetEntParentId, NetEntId, UDPacketEvent,
        client::{DespawnUnit2, PlayerDisconnected, UpdateUnit2},
    },
    net_components::{NetComponent, ours::ControlledBy},
};

use crate::{game_state::NetworkGameState, notification::Notification};

/// Marker component for remote player camera entities (not our local camera)
#[derive(Component)]
pub struct RemotePlayerCamera;

/// Marker component for the visual model (G-Toilet) representing a remote player
#[derive(Component)]
pub struct RemotePlayerModel;

/// Marker component for player name labels (2D UI nodes)
#[derive(Component)]
pub struct NameLabel {
    pub target_entity: Entity,
}

/// Marker component for scenes that need NoFrustumCulling applied to all their meshes
#[derive(Component)]
pub struct ApplyNoFrustumCulling;

pub struct RemotePlayersPlugin;

impl Plugin for RemotePlayersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_update_unit,
                handle_despawn_unit,
                handle_player_disconnect,
                update_name_label_positions,
                apply_player_color_tint,
                apply_no_frustum_culling_to_scene_meshes,
            )
                .chain()
                .run_if(in_state(NetworkGameState::ClientConnected)),
        );
    }
}

/// Handle updating unit transforms (mainly for player cameras)
fn handle_update_unit(
    mut commands: Commands,
    mut update_events: UDPacketEvent<UpdateUnit2>,
    mut remote_unit: Query<
        (&NetEntId, &mut Transform),
        With<shared::net_components::ents::SendNetworkTranformUpdates>,
    >,
    mut remote_unit2: Query<
        (&NetEntId, &mut LinearVelocity),
        With<shared::net_components::ents::SendNetworkTranformUpdates>,
    >,
    // TODO should we add a tag here to restrict to only certain entities?
    mut new_component_units: Query<
        (Entity, &NetEntId),
    >
) {
    for update in update_events.read() {
        // Find the entity with this NetEntId
        'a1: for (net_id, mut transform) in &mut remote_unit {
            if net_id == &update.event.net_ent_id {
                // Update the transform from components
                for component in &update.event.changed_components {
                    if let NetComponent::Foreign(foreign) = component {
                        if let shared::net_components::foreign::NetComponentForeign::Transform(
                            tfm,
                        ) = foreign
                        {
                            *transform = *tfm;
                        }
                    }
                }
                break 'a1;
            }
        }

        'a2: for (net_id, mut velocity) in &mut remote_unit2 {
            if net_id == &update.event.net_ent_id {
                // Update the velocity from components
                for component in &update.event.changed_components {
                    if let NetComponent::Foreign(foreign) = component {
                        if let shared::net_components::foreign::NetComponentForeign::LinearVelocity(lv) = foreign {
                            *velocity = *lv;
                        }
                    }
                }
                break 'a2;
            }
        }

        'a3: for (ent, net_id) in &mut new_component_units {
            if net_id == &update.event.net_ent_id {
                // Add any new components
                for component in &update.event.new_component {
                    let mut ec = commands.entity(ent);
                    component.clone().insert_components(&mut ec);
                }
                break 'a3;
            }
        }
    }
}

/// Handle despawning units
fn handle_despawn_unit(
    mut despawn_events: UDPacketEvent<DespawnUnit2>,
    remote_entities: Query<(Entity, &NetEntId)>,
    remote_entities_2: Query<(Entity, &MyNetEntParentId)>,
    mut commands: Commands,
) {
    for despawn in despawn_events.read() {
        'rem1: for (entity, net_id) in &remote_entities {
            if net_id == &despawn.event.net_ent_id {
                info!("Despawning remote entity {:?}", despawn.event.net_ent_id);
                commands.entity(entity).despawn();
                break 'rem1;
            }
        }

        'rem2: for (entity, parent_net_id) in &remote_entities_2 {
            if parent_net_id.0 == despawn.event.net_ent_id.0 {
                info!(
                    "Despawning remote entity child {:?}",
                    despawn.event.net_ent_id
                );
                commands.entity(entity).despawn();
                break 'rem2;
            }
        }
    }
}

/// Handle player disconnections
fn handle_player_disconnect(
    mut disconnect_events: UDPacketEvent<PlayerDisconnected>,
    remote_cameras: Query<(Entity, &ControlledBy, &RemotePlayerCamera)>,
    mut commands: Commands,
    mut notif: MessageWriter<Notification>,
) {
    for event in disconnect_events.read() {
        let player_id = event.event.id;

        info!("Player disconnected: {:?}", player_id);

        // Find and despawn all entities belonging to this player
        for (entity, controlled_by, _remote_cam) in &remote_cameras {
            if *controlled_by.players == [player_id] {
                info!(
                    "Despawning remote player camera and model for {:?}",
                    player_id
                );
                // Despawn (automatically despawns children in Bevy 0.17)
                commands.entity(entity).despawn();
            }
        }

        notif.write(Notification("Player disconnected".to_string()));
    }
}

/// Update 2D UI name label positions based on 3D world positions
fn update_name_label_positions(
    mut labels: Query<(&mut Node, &NameLabel)>,
    targets: Query<&GlobalTransform>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::LocalCamera>>,
) {
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };

    for (mut node, name_label) in labels.iter_mut() {
        // Get the target entity's world position
        if let Ok(target_transform) = targets.get(name_label.target_entity) {
            // Calculate position above the player (3 units up)
            let world_pos = target_transform.translation() + Vec3::new(0.0, 3.0, 0.0);

            // Project 3D world position to 2D screen position
            if let Ok(screen_pos) = camera.world_to_viewport(camera_transform, world_pos) {
                // Center the label on the screen position
                node.left = Val::Px(screen_pos.x - 50.0); // Offset to center text
                node.top = Val::Px(screen_pos.y - 10.0);
            } else {
                // If position is behind camera or off-screen, hide it
                node.left = Val::Px(-1000.0);
                node.top = Val::Px(-1000.0);
            }
        }
    }
}

/// Marker component to track that color has been applied
#[derive(Component)]
struct ColorApplied;

/// Apply color tint to remote player models based on their PlayerColor component
fn apply_player_color_tint(
    models: Query<
        (Entity, &shared::net_components::ours::PlayerColor),
        (With<RemotePlayerModel>, Without<ColorApplied>),
    >,
    mut commands: Commands,
) {
    for (model_entity, player_color) in &models {
        // Add a colored point light as a child to tint the model
        let tint_color = Color::hsl(player_color.hue, 1.0, 0.6);
        commands.entity(model_entity).with_children(|parent| {
            parent.spawn((
                PointLight {
                    color: tint_color,
                    intensity: 500000.0, // Increased intensity
                    range: 8.0,          // Increased range
                    radius: 1.0,         // Larger radius for softer light
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.5, 0.0), // Positioned lower, more central
            ));
        });

        // Mark as processed
        commands.entity(model_entity).insert(ColorApplied);
    }
}

/// Apply NoFrustumCulling to all mesh entities within scenes marked with ApplyNoFrustumCulling
/// This prevents individual mesh components from being culled when they shouldn't be (self-occlusion)
fn apply_no_frustum_culling_to_scene_meshes(
    // Find all scene roots marked for NoFrustumCulling
    scene_roots: Query<(Entity, &Children), With<ApplyNoFrustumCulling>>,
    // Find all mesh entities (they have a Mesh3d component)
    all_children: Query<&Children>,
    mesh_entities: Query<Entity, (With<Mesh3d>, Without<NoFrustumCulling>)>,
    mut commands: Commands,
) {
    // For each marked scene root
    for (_scene_root, children) in &scene_roots {
        // Recursively find all descendant mesh entities
        let mut to_process: Vec<Entity> = children.iter().collect();

        while let Some(entity) = to_process.pop() {
            // If this entity is a mesh without NoFrustumCulling, add it
            if mesh_entities.contains(entity) {
                commands.entity(entity).insert(NoFrustumCulling);
            }

            // Add children to processing queue
            if let Ok(entity_children) = all_children.get(entity) {
                to_process.extend(entity_children.iter());
            }
        }
    }
}
