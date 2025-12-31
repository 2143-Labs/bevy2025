use avian3d::math::Vector3;
use bevy::prelude::*;
use shared::character_controller::SpawnDebugBall;

#[derive(Resource)]
pub struct ShowPhysicsDebug;

#[derive(Component)]
pub struct DebugCollisionBall {
    time_spawned: f64,
}

pub struct ClientCharacterControllerPlugin;

impl Plugin for ClientCharacterControllerPlugin {
    fn build(&self, app: &mut App) {
        app
            //.init_resource::<ShowPhysicsDebug>()
            .add_systems(
                Update,
                (debug_spawn_collision_ball, remove_old_debug_balls)
                    .run_if(resource_exists::<ShowPhysicsDebug>),
            );
        // Add systems and resources related to character controller here
    }
}

fn debug_spawn_collision_ball(
    mut message_reader: MessageReader<SpawnDebugBall>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    time: Res<Time>,
) {
    for event in message_reader.read() {
        commands.spawn((
            Transform::from_translation(event.position + Vector3::Y * 2.0),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: event.color,
                unlit: true,
                ..Default::default()
            })),
            Mesh3d(meshes.add(Mesh::from(Sphere { radius: 0.1 }))),
            DebugCollisionBall {
                time_spawned: time.elapsed_secs_f64(),
            },
        ));
    }
}

const OLD_DEBUG_BALL_CLEANUP_INTERVAL: f64 = 5.0;
fn remove_old_debug_balls(
    mut commands: Commands,
    query: Query<(Entity, &DebugCollisionBall)>,
    time: Res<Time>,
) {
    let current_time = time.elapsed_secs_f64();
    for (entity, debug_ball) in &query {
        if current_time - debug_ball.time_spawned > OLD_DEBUG_BALL_CLEANUP_INTERVAL {
            commands.entity(entity).despawn();
        }
    }
}
