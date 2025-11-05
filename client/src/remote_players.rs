use bevy::{camera::visibility::NoFrustumCulling, prelude::*};
use shared::{
    event::{
        client::{DespawnUnit2, PlayerDisconnected, SpawnUnit2, UpdateUnit2},
        NetEntId, ERFE,
    },
    net_components::NetComponent,
};

use crate::{
    assets::{ModelAssets, FontAssets},
    game_state::NetworkGameState,
    notification::Notification,
};

/// Marker component for remote player camera entities (not our local camera)
#[derive(Component)]
pub struct RemotePlayerCamera {
    pub player_net_id: NetEntId,
}

/// Marker component for the visual model (G-Toilet) representing a remote player
#[derive(Component)]
pub struct RemotePlayerModel {
    pub camera_net_id: NetEntId,
}

/// Marker component for player name labels (2D UI nodes)
#[derive(Component)]
pub struct NameLabel {
    /// The entity this label is attached to (the remote player camera)
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
                forward_network_events_to_local,
                handle_spawn_unit,
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

/// Forward network events to local message system for processing
fn forward_network_events_to_local(
    mut spawn_net: ERFE<SpawnUnit2>,
    mut update_net: ERFE<UpdateUnit2>,
    mut despawn_net: ERFE<DespawnUnit2>,
    mut disconnect_net: ERFE<PlayerDisconnected>,
    mut spawn_local: MessageWriter<SpawnUnit2>,
    mut update_local: MessageWriter<UpdateUnit2>,
    mut despawn_local: MessageWriter<DespawnUnit2>,
    mut disconnect_local: MessageWriter<PlayerDisconnected>,
) {
    for event in spawn_net.read() {
        spawn_local.write(event.event.clone());
    }
    for event in update_net.read() {
        update_local.write(event.event.clone());
    }
    for event in despawn_net.read() {
        despawn_local.write(event.event.clone());
    }
    for event in disconnect_net.read() {
        disconnect_local.write(event.event.clone());
    }
}

/// Handle spawning units from the server
fn handle_spawn_unit(
    mut spawn_events: MessageReader<SpawnUnit2>,
    mut commands: Commands,
    model_assets: Res<ModelAssets>,
    font_assets: Res<FontAssets>,
    mut notif: MessageWriter<Notification>,
) {
    for unit in spawn_events.read() {

        // Check if this is a PlayerCamera component
        let mut is_player_camera = false;
        let mut player_name = None;
        let mut player_color_hue = 0.0; // Default red
        let mut transform = Transform::default();
        let mut parent_id: Option<NetEntId> = None;

        for component in &unit.components {
            match component {
                NetComponent::Ents(ents) => {
                    if matches!(ents, shared::net_components::ents::NetComponentEnts::PlayerCamera(_)) {
                        is_player_camera = true;
                    }
                }
                NetComponent::Ours(ours) => {
                    match ours {
                        shared::net_components::ours::NetComponentOurs::PlayerName(name) => {
                            player_name = Some(name.name.clone());
                        }
                        shared::net_components::ours::NetComponentOurs::PlayerColor(color) => {
                            player_color_hue = color.hue;
                        }
                        _ => {}
                    }
                }
                NetComponent::Foreign(foreign) => {
                    if let shared::net_components::foreign::NetComponentForeign::Transform(tfm) = foreign {
                        transform = *tfm;
                    }
                }
                NetComponent::MyNetEntParentId(pid) => {
                    parent_id = Some(NetEntId(pid.0));
                }
                NetComponent::Groups(_) | NetComponent::NetEntId(_) => {
                    // Ignore these for player camera spawning
                }
            }
        }

        if is_player_camera {
            if let Some(name) = player_name {
                // Spawn the camera tracking entity with proper spatial bundle
                let camera_entity = commands.spawn((
                    unit.net_ent_id,
                    RemotePlayerCamera {
                        player_net_id: parent_id.unwrap_or(unit.net_ent_id),
                    },
                    Transform::from_translation(transform.translation)
                        .with_rotation(transform.rotation),
                    GlobalTransform::default(),
                    Visibility::default(),
                    InheritedVisibility::default(),
                    ViewVisibility::default(),
                )).id();

                // Spawn the G-Toilet model as a child, rotated 180° to face forward
                commands.entity(camera_entity).with_children(|parent| {
                    parent.spawn((
                        SceneRoot(model_assets.g_toilet_scene.clone()),
                        Transform::from_xyz(0.0, 0.0, 0.0)
                            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)) // 180° rotation
                            .with_scale(Vec3::splat(0.5)),
                        RemotePlayerModel {
                            camera_net_id: unit.net_ent_id,
                        },
                        shared::net_components::ours::PlayerColor { hue: player_color_hue },
                        ApplyNoFrustumCulling, // Mark this scene to have NoFrustumCulling applied to all meshes
                    ));
                });

                // Spawn 2D UI name label (will be positioned via world-to-screen projection)
                commands.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                    Text::new(name.clone()),
                    TextFont {
                        font: font_assets.regular.clone(),
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    NameLabel {
                        target_entity: camera_entity,
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)), // Semi-transparent background
                ));

                info!("Player {} joined", name);
                notif.write(Notification(format!("{} joined", name)));
            }
        }
    }
}

/// Handle updating unit transforms (mainly for player cameras)
fn handle_update_unit(
    mut update_events: MessageReader<UpdateUnit2>,
    mut remote_cameras: Query<(&NetEntId, &mut Transform), With<RemotePlayerCamera>>,
) {
    for update in update_events.read() {

        // Find the entity with this NetEntId
        for (net_id, mut transform) in &mut remote_cameras {
            if net_id == &update.net_ent_id {
                // Update the transform from components
                for component in &update.components {
                    if let NetComponent::Foreign(foreign) = component {
                        if let shared::net_components::foreign::NetComponentForeign::Transform(tfm) = foreign {
                            *transform = *tfm;
                        }
                    }
                }
                break;
            }
        }
    }
}

/// Handle despawning units
fn handle_despawn_unit(
    mut despawn_events: MessageReader<DespawnUnit2>,
    remote_entities: Query<(Entity, &NetEntId), Or<(With<RemotePlayerCamera>, With<RemotePlayerModel>)>>,
    mut commands: Commands,
) {
    for despawn in despawn_events.read() {

        for (entity, net_id) in &remote_entities {
            if net_id == &despawn.net_ent_id {
                info!("Despawning remote entity {:?}", despawn.net_ent_id);
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Handle player disconnections
fn handle_player_disconnect(
    mut disconnect_events: MessageReader<PlayerDisconnected>,
    remote_cameras: Query<(Entity, &RemotePlayerCamera)>,
    mut commands: Commands,
    mut notif: MessageWriter<Notification>,
) {
    for event in disconnect_events.read() {
        let player_id = event.id;

        info!("Player disconnected: {:?}", player_id);

        // Find and despawn all entities belonging to this player
        for (entity, remote_cam) in &remote_cameras {
            if remote_cam.player_net_id == player_id {
                info!("Despawning remote player camera and model for {:?}", player_id);
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
    targets: Query<&GlobalTransform, With<RemotePlayerCamera>>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::LocalCamera>>,
) {
    let Ok((camera, camera_transform)) = camera.single() else { return };

    for (mut node, label) in &mut labels {
        // Get the target entity's world position
        if let Ok(target_transform) = targets.get(label.target_entity) {
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
        (With<RemotePlayerModel>, Without<ColorApplied>)
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
                    range: 8.0, // Increased range
                    radius: 1.0, // Larger radius for softer light
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

