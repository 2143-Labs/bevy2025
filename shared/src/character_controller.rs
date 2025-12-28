//! A simple kinematic character controller implementation using Avian3D and Bevy.
//! Copied from https://github.com/avianphysics/avian/blob/60ef5cf4/crates/avian2d/examples/kinematic_character_2d/plugin.rs#L39-L140
use avian3d::{math::*, prelude::*};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{event::NetEntId, net_components::{NetComponent, ToNetComponent}};

pub struct CharacterControllerPlugin;

impl Plugin for CharacterControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SpawnDebugBall>()
            .add_message::<UnitChangedMovement>()
            .add_systems(
                Update,
                (
                    apply_gravity,
                    unit_change_movement,
                    update_grounded,
                    movement,
                    apply_movement_damping,
                )
                    .chain(),
            )
            .add_systems(Update, (debug_spawn_collision_ball, remove_old_debug_balls))
            .add_systems(
                // Run collision handling after collision detection.
                //
                // NOTE: The collision implementation here is very basic and a bit buggy.
                //       A collide-and-slide algorithm would likely work better.
                PhysicsSchedule,
                kinematic_controller_collisions.in_set(NarrowPhaseSystems::Last),
            );
    }
}

/// A [`Message`] written for a movement input action.
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct MovementAction {
    pub move_input_dir: Vector2,
    pub camera_yaw: Scalar,
    pub move_speed_modifier: Scalar,
    pub is_jumping: bool
}

impl Default for MovementAction {
    fn default() -> Self {
        Self {
            move_input_dir: Vector2::ZERO,
            camera_yaw: 0.0,
            move_speed_modifier: 1.0,
            is_jumping: false,
        }
    }
}

#[derive(Message)]
pub struct UnitChangedMovement {
    pub net_ent_id: NetEntId,
    pub movement_action: MovementAction,
}

/// A marker component indicating that an entity is using a character controller.
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct CharacterController;

/// A marker component indicating that an entity is on the ground.
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Groundedness(pub bool);

/// The normal vector of the ground surface the character is standing on.
/// This is used to project movement onto slopes.
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct GroundNormal(pub Vector);

/// Tracks when the player last attempted to jump, allowing for a small buffer
/// window to make jumping more forgiving when grounded status flickers.
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct JumpBuffer {
    pub last_jump_attempt_time: f64,
    pub buffer_duration: f64,
}

/// The acceleration used for character movement.
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct MovementAcceleration(pub Scalar);

/// The damping factor used for slowing down movement.
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct MovementDampingFactor(pub Scalar);

/// The strength of a jump.
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct JumpImpulse(pub Scalar);

/// The gravitational acceleration used for a character controller.
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct ControllerGravity(pub Vector);

/// The maximum angle a slope can have for a character controller
/// to be able to climb and jump. If the slope is steeper than this angle,
/// the character will slide down.
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct MaxSlopeAngle(pub Scalar);

/// A bundle that contains the components needed for a basic
/// kinematic character controller.
#[derive(Bundle, Serialize, Deserialize, Clone, Debug)]
pub struct CharacterControllerBundle {
    character_controller: CharacterController,
    body: RigidBody,
    collider: Collider,
    ground_caster: ShapeCaster,
    gravity: ControllerGravity,
    movement: MovementBundle,
    movement_action: MovementAction,
}

impl ToNetComponent for CharacterControllerBundle {
    fn to_net_component(self) -> NetComponent {
        NetComponent::CharacterControllerBundle(Box::new(self))
    }
}

/// A bundle that contains components for character movement.
#[derive(Bundle, Serialize, Deserialize, Clone, Debug)]
pub struct MovementBundle {
    acceleration: MovementAcceleration,
    damping: MovementDampingFactor,
    jump_impulse: JumpImpulse,
    max_slope_angle: MaxSlopeAngle,
    groundedness: Groundedness,
    ground_normal: GroundNormal,
    jump_buffer: JumpBuffer,
}

impl MovementBundle {
    pub const fn new(
        acceleration: Scalar,
        damping: Scalar,
        jump_impulse: Scalar,
        max_slope_angle: Scalar,
    ) -> Self {
        Self {
            acceleration: MovementAcceleration(acceleration),
            damping: MovementDampingFactor(damping),
            jump_impulse: JumpImpulse(jump_impulse),
            max_slope_angle: MaxSlopeAngle(max_slope_angle),
            groundedness: Groundedness(false),
            ground_normal: GroundNormal(Vector::Y),
            jump_buffer: JumpBuffer {
                last_jump_attempt_time: f64::NEG_INFINITY,
                buffer_duration: 0.15, // 150ms buffer window
            },
        }
    }
}

impl Default for MovementBundle {
    fn default() -> Self {
        Self::new(30.0, 0.9, 7.0, PI * 0.45)
    }
}

impl CharacterControllerBundle {
    pub fn new(collider: Collider, gravity: Vector) -> Self {
        // Create shape caster as a slightly smaller version of collider
        let mut caster_shape = collider.clone();
        caster_shape.set_scale(Vector::ONE * 1.05, 10);

        Self {
            character_controller: CharacterController,
            body: RigidBody::Kinematic,
            collider,
            ground_caster: ShapeCaster::new(
                caster_shape,
                Vector::ZERO,
                Quat::from_rotation_y(0.0),
                -Dir3::Y, // Cast downward to detect ground
            )
            .with_max_distance(4.0),
            gravity: ControllerGravity(gravity),
            movement: MovementBundle::default(),
            movement_action: MovementAction::default(),
        }
    }

    pub fn with_movement(
        mut self,
        acceleration: Scalar,
        damping: Scalar,
        jump_impulse: Scalar,
        max_slope_angle: Scalar,
    ) -> Self {
        self.movement = MovementBundle::new(acceleration, damping, jump_impulse, max_slope_angle);
        self
    }
}

/// Updates the [`Grounded`] status for character controllers.
fn update_grounded(
    //mut commands: Commands,
    mut query: Query<
        (
            &mut Groundedness,
            &mut GroundNormal,
            Entity,
            &ShapeHits,
            &Rotation,
            Option<&MaxSlopeAngle>,
        ),
        With<CharacterController>,
    >,
) {
    for (mut groundedness, mut ground_normal, _entity, hits, rotation, max_slope_angle) in &mut query {
        // The character is grounded if the shape caster has a hit with a normal
        // that isn't too steep and is within a reasonable distance.
        let mut found_ground = false;
        let mut best_normal = Vector::Y;
        let mut closest_distance = Scalar::MAX;
        
        // Maximum distance to consider as "grounded" - should be slightly more than
        // the character's radius to account for small gaps
        const GROUND_DETECTION_THRESHOLD: Scalar = 0.15;
        
        for hit in hits.iter() {
            // Check if the hit is close enough to count as ground
            if hit.distance > GROUND_DETECTION_THRESHOLD {
                continue;
            }
            
            let world_normal = rotation * -hit.normal2;
            let is_climbable = if let Some(angle) = max_slope_angle {
                world_normal.angle_between(Vector::Y).abs() <= angle.0
            } else {
                true
            };
            
            // Find the closest valid ground hit
            if is_climbable && hit.distance < closest_distance {
                found_ground = true;
                best_normal = world_normal;
                closest_distance = hit.distance;
            }
        }

        groundedness.0 = found_ground;
        ground_normal.0 = best_normal;
    }
}

fn unit_change_movement(
    mut reader: MessageReader<UnitChangedMovement>,
    // TODO make this smaller?
    mut query: Query<(&mut MovementAction, &NetEntId)>,
) {
    for event in reader.read() {
        for (mut movement_action, net_ent_id) in &mut query {
            if net_ent_id == &event.net_ent_id {
                *movement_action = event.movement_action.clone();
            }
        }
    }
}

/// Responds to [`MovementAction`] events and moves character controllers accordingly.
fn movement(
    time: Res<Time>,
    mut controllers: Query<(
        &MovementAcceleration,
        &MovementAction,
        &JumpImpulse,
        &mut LinearVelocity,
        &Groundedness,
        &GroundNormal,
        &mut JumpBuffer,
    )>,
) {
    // Precision is adjusted so that the example works with
    // both the `f32` and `f64` features. Otherwise you don't need this.
    let delta_time = time.delta_secs_f64().adjust_precision();
    let current_time = time.elapsed_secs_f64();

    for (
        movement_acceleration,
        action,
        jump_impulse,
        mut linear_velocity,
        Groundedness(is_grounded),
        GroundNormal(ground_normal),
        mut jump_buffer,
    ) in &mut controllers
    {
        let input_dir = action.move_input_dir;
        let camera_yaw = action.camera_yaw;
        let speed_modifier = action.move_speed_modifier;

        // Convert input direction to 3D movement direction
        let input_dir_3d = Vector3::new(-input_dir.x, 0.0, input_dir.y);

        // Rotate input direction based on camera yaw
        let rotation = Quat::from_rotation_y(camera_yaw as Scalar);
        let movement_dir = rotation * input_dir_3d;

        // When grounded, project the movement direction onto the slope surface
        // so that movement follows the slope naturally.
        let final_movement_dir = if *is_grounded {
            // Project the horizontal movement direction onto the slope plane.
            // This allows smooth movement up and down slopes.
            movement_dir.reject_from_normalized(*ground_normal).normalize_or_zero()
        } else {
            movement_dir.normalize_or_zero()
        };

        // Apply acceleration to linear velocity
        let acceleration = final_movement_dir
            * movement_acceleration.0
            * speed_modifier
            * delta_time;

        if *is_grounded {
            linear_velocity.0 += acceleration;
        } else {
            linear_velocity.0 += acceleration * 0.25;
        }

        // Check if we can execute a buffered jump
        let time_since_jump_attempt = current_time - jump_buffer.last_jump_attempt_time;
        if time_since_jump_attempt <= jump_buffer.buffer_duration && *is_grounded {
            linear_velocity.y = jump_impulse.0;
            jump_buffer.last_jump_attempt_time = f64::NEG_INFINITY; // Consume the buffered jump
        }

        if action.is_jumping {
            // Record the jump attempt time for buffer window
            jump_buffer.last_jump_attempt_time = current_time;

            // Try to jump immediately if grounded, otherwise rely on buffer
            if *is_grounded {
                linear_velocity.y = jump_impulse.0;
                jump_buffer.last_jump_attempt_time = f64::NEG_INFINITY; // Consume immediately
            }
        }
    }
}

/// Applies [`ControllerGravity`] to character controllers.
fn apply_gravity(
    time: Res<Time>,
    mut controllers: Query<(&ControllerGravity, &mut LinearVelocity)>,
) {
    // Precision is adjusted so that the example works with
    // both the `f32` and `f64` features. Otherwise you don't need this.
    let delta_time = time.delta_secs_f64().adjust_precision();

    for (gravity, mut linear_velocity) in &mut controllers {
        linear_velocity.0 += gravity.0 * delta_time;
    }
}

/// Slows down movement in the X direction.
fn apply_movement_damping(
    time: Res<Time>,
    mut query: Query<(&MovementDampingFactor, &mut LinearVelocity)>,
) {
    // Precision is adjusted so that the example works with
    // both the `f32` and `f64` features. Otherwise you don't need this.
    let delta_time = time.delta_secs_f64().adjust_precision();

    for (damping_factor, mut linear_velocity) in &mut query {
        // We could use `LinearDamping`, but we don't want to dampen movement along the Y axis
        linear_velocity.x *= 1.0 / (1.0 + damping_factor.0 * delta_time);
        linear_velocity.z *= 1.0 / (1.0 + damping_factor.0 * delta_time);
    }
}

#[derive(Component)]
struct DebugCollisionBall {
    time_spawned: f64,
}

#[derive(Message)]
struct SpawnDebugBall {
    position: Vector3,
    color: Color,
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

/// Kinematic bodies do not get pushed by collisions by default,
/// so it needs to be done manually.
///
/// This system handles collision response for kinematic character controllers
/// by pushing them along their contact normals by the current penetration depth,
/// and applying velocity corrections in order to snap to slopes, slide along walls,
/// and predict collisions using speculative contacts.
#[allow(clippy::type_complexity)]
fn kinematic_controller_collisions(
    collisions: Collisions,
    bodies: Query<&RigidBody>,
    collider_rbs: Query<&ColliderOf, Without<Sensor>>,
    mut character_controllers: Query<
        (&mut Position, &mut LinearVelocity, Option<&MaxSlopeAngle>),
        (With<RigidBody>, With<CharacterController>),
    >,
    time: Res<Time>,
    mut debug_ball_writer: MessageWriter<SpawnDebugBall>,
) {
    // Iterate through collisions and move the kinematic body to resolve penetration
    for contacts in collisions.iter() {
        // Get the rigid body entities of the colliders (colliders could be children)
        let Ok([&ColliderOf { body: rb1 }, &ColliderOf { body: rb2 }]) =
            collider_rbs.get_many([contacts.collider1, contacts.collider2])
        else {
            continue;
        };

        // Get the body of the character controller and whether it is the first
        // or second entity in the collision.
        let is_first: bool;

        let character_rb: RigidBody;
        let is_other_dynamic: bool;

        let (mut position, mut linear_velocity, max_slope_angle) =
            if let Ok(character) = character_controllers.get_mut(rb1) {
                is_first = true;
                character_rb = *bodies.get(rb1).unwrap();
                is_other_dynamic = bodies.get(rb2).is_ok_and(|rb| rb.is_dynamic());
                character
            } else if let Ok(character) = character_controllers.get_mut(rb2) {
                is_first = false;
                character_rb = *bodies.get(rb2).unwrap();
                is_other_dynamic = bodies.get(rb1).is_ok_and(|rb| rb.is_dynamic());
                character
            } else {
                continue;
            };

        // This system only handles collision response for kinematic character controllers.
        if !character_rb.is_kinematic() {
            continue;
        }

        // Iterate through contact manifolds and their contacts.
        // Each contact in a single manifold shares the same contact normal.
        for manifold in contacts.manifolds.iter() {
            let normal = if is_first {
                -manifold.normal
            } else {
                manifold.normal
            };

            let mut deepest_penetration: Scalar = Scalar::MIN;

            // Solve each penetrating contact in the manifold.
            for contact in manifold.points.iter() {
                if contact.penetration > 0.0 {
                    position.0 += normal * contact.penetration * 1.01;
                }
                deepest_penetration = deepest_penetration.max(contact.penetration);
            }

            // For now, this system only handles velocity corrections for collisions against static geometry.
            if is_other_dynamic {
                continue;
            }

            // Determine if the slope is climbable or if it's too steep to walk on.
            let slope_angle = normal.angle_between(Vector::Y);
            let climbable = max_slope_angle.is_some_and(|angle| slope_angle.abs() <= angle.0);
            debug!(
                "\
                Slope angle: {:.2} degrees: climbable = {}, deepest penetration = {:.4}
            ",
                slope_angle.to_degrees(),
                climbable,
                deepest_penetration,
            );

            if deepest_penetration > 0.0 {
                // If the slope is climbable, project the velocity onto the slope surface
                // so the character can smoothly walk up and down slopes.
                if climbable {
                    // Project the velocity onto the slope plane (perpendicular to the normal).
                    // This ensures the character moves along the slope surface rather than
                    // trying to move through it or float above it.
                    // The reject_from_normalized function removes the component along the normal,
                    // leaving only the component along the slope surface.
                    let velocity_on_slope = linear_velocity.reject_from_normalized(normal);
                    
                    // Replace the velocity with the projected velocity to follow the slope.
                    // This automatically handles both walking up and down slopes correctly.
                    linear_velocity.0 = velocity_on_slope;
                    debug_ball_writer.write(SpawnDebugBall {
                        position: position.0,
                        //green
                        color: Color::hsv(120.0, 1.0, 1.0),
                    });
                    // spawn a second ball in the impulse direction
                    debug_ball_writer.write(SpawnDebugBall {
                        position: position.0 + Vector3::Y * 0.1,
                        color: Color::hsv(125.0, 1.0, 1.0),
                    });
                } else {
                    // The character is intersecting an unclimbable object, like a wall.
                    // We want the character to slide along the surface, similarly to
                    // a collide-and-slide algorithm.

                    // Don't apply an impulse if the character is moving away from the surface.
                    if linear_velocity.dot(normal) > 0.0 {
                        continue;
                    }

                    // Slide along the surface, rejecting the velocity along the contact normal.
                    let impulse = linear_velocity.reject_from_normalized(normal);
                    linear_velocity.0 = impulse;
                    debug_ball_writer.write(SpawnDebugBall {
                        position: position.0,
                        //red
                        color: Color::hsv(0.0, 1.0, 1.0),
                    });
                    // spawn a second ball in the impulse direction
                    debug_ball_writer.write(SpawnDebugBall {
                        position: position.0 + impulse.normalize_or_zero() * 0.1,
                        color: Color::hsv(5.0, 1.0, 1.0),
                    });
                }
            } else {
                // The character is not yet intersecting the other object,
                // but the narrow phase detected a speculative collision.
                //
                // We need to push back the part of the velocity
                // that would cause penetration within the next frame.

                let normal_speed = linear_velocity.dot(normal);

                // Don't apply an impulse if the character is moving away from the surface.
                if normal_speed > 0.0 {
                    debug_ball_writer.write(SpawnDebugBall {
                        position: position.0,
                        //pink
                        color: Color::hsv(300.0, 1.0, 1.0),
                    });
                    continue;
                }

                // Compute the impulse to apply.
                let impulse_magnitude =
                    normal_speed - (deepest_penetration / time.delta_secs_f64().adjust_precision());
                let mut impulse = impulse_magnitude * normal;

                // Apply the impulse differently depending on the slope angle.
                if climbable {
                    // Project the velocity onto the slope to prevent penetration
                    // while allowing movement along the slope.
                    let velocity_on_slope = linear_velocity.reject_from_normalized(normal);
                    linear_velocity.0 = velocity_on_slope;
                    debug_ball_writer.write(SpawnDebugBall {
                        position: position.0,
                        //blue
                        color: Color::hsv(240.0, 1.0, 1.0),
                    });
                    // spawn a second ball in the impulse direction
                    debug_ball_writer.write(SpawnDebugBall {
                        position: position.0 + velocity_on_slope.normalize_or_zero() * 0.1,
                        color: Color::hsv(245.0, 1.0, 1.0),
                    });
                } else {
                    // Avoid climbing up walls.
                    impulse.y = impulse.y.max(0.0);
                    linear_velocity.0 -= impulse;
                    debug_ball_writer.write(SpawnDebugBall {
                        position: position.0,
                        //yellow
                        color: Color::hsv(60.0, 1.0, 1.0),
                    });
                    // spawn a second ball in the impulse direction
                    debug_ball_writer.write(SpawnDebugBall {
                        position: position.0 + impulse.normalize_or_zero() * 0.1,
                        color: Color::hsv(65.0, 1.0, 1.0),
                    });
                }
            }
        }
    }
}
