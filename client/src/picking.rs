use avian3d::prelude::*;
use bevy::prelude::*;
use shared::physics::terrain::BoundaryWall;

/// Marker for the cursor indicator orb
#[derive(Component)]
struct CursorIndicator;

/// Resource to track the current pick point
#[derive(Resource, Default)]
struct PickPoint {
    position: Option<Vec3>,
    normal: Option<Vec3>,
}

pub struct PickingPlugin;

impl Plugin for PickingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PickPoint::default())
            .add_systems(Startup, setup_cursor_indicator)
            .add_systems(Update, (update_pick_point, handle_click_impulse).chain());
    }
}

/// Spawn the translucent glowing yellow orb indicator
fn setup_cursor_indicator(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.3))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 0.0, 0.5), // Translucent yellow
            emissive: LinearRgba::rgb(5.0, 5.0, 0.0),     // Glowing effect
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_xyz(0.0, -1000.0, 0.0), // Start off-screen
        CursorIndicator,
    ));
}

/// Update the pick point by raycasting from the cursor
fn update_pick_point(
    mut pick_point: ResMut<PickPoint>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    windows: Query<&Window>,
    spatial_query: SpatialQuery,
    mut indicator_query: Query<&mut Transform, With<CursorIndicator>>,
    boundary_walls: Query<Entity, With<BoundaryWall>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    let Some(cursor_position) = window.cursor_position() else {
        // No cursor, hide indicator
        pick_point.position = None;
        pick_point.normal = None;
        if let Ok(mut transform) = indicator_query.single_mut() {
            transform.translation.y = -1000.0; // Move off-screen
        }
        return;
    };

    let Some((camera, camera_transform)) = cameras.iter().find(|(cam, _)| cam.is_active) else {
        return;
    };

    // Convert cursor position to ray
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
        return;
    };

    // Cast ray into the scene, excluding boundary walls
    let max_distance = 1000.0;

    // Collect all boundary wall entities to exclude from raycast
    let excluded_entities: Vec<Entity> = boundary_walls.iter().collect();
    let filter = SpatialQueryFilter::default().with_excluded_entities(excluded_entities);

    if let Some(hit) = spatial_query.cast_ray(
        ray.origin,
        ray.direction.into(),
        max_distance,
        true,
        &filter,
    ) {
        let hit_point = ray.origin + *ray.direction * hit.distance;
        pick_point.position = Some(hit_point);
        pick_point.normal = Some(hit.normal);

        // Update indicator position
        if let Ok(mut transform) = indicator_query.single_mut() {
            transform.translation = hit_point + hit.normal * 0.3; // Slightly above surface
        }
    } else {
        // No hit, hide indicator
        pick_point.position = None;
        pick_point.normal = None;
        if let Ok(mut transform) = indicator_query.single_mut() {
            transform.translation.y = -1000.0; // Move off-screen
        }
    }
}

/// Handle holding left-click to apply continuous force at pick point
fn handle_click_impulse(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    pick_point: Res<PickPoint>,
    mut bodies: Query<(&mut LinearVelocity, &Transform), With<RigidBody>>,
) {
    // Only apply force while mouse button is held down
    if !mouse.pressed(MouseButton::Left) {
        return;
    }

    let Some(click_pos) = pick_point.position else {
        return;
    };

    // Find all dynamic bodies within radius and apply continuous force
    let impulse_radius = 10.0;
    let impulse_strength = 160.0;

    // Scale by delta time for frame-rate independent force
    let force_multiplier = impulse_strength * time.delta_secs();

    for (mut velocity, transform) in bodies.iter_mut() {
        let distance = transform.translation.distance(click_pos);

        if distance < impulse_radius && distance > 0.01 {
            // Calculate direction from click point to body
            let direction = (transform.translation - click_pos).normalize();

            // Apply force with falloff based on distance
            let falloff = 1.0 - (distance / impulse_radius);
            let force = direction * force_multiplier * falloff;

            velocity.0 += force;
        }
    }
}
