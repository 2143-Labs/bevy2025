use avian3d::prelude::{Collider, CollisionEventsEnabled, Sensor};
use bevy::prelude::*;

use crate::{
    BASE_TICKS_PER_SECOND, CurrentTick,
    event::client::SpawnProjectile,
    physics::terrain::TerrainParams,
    skills::{Skill, SkillSource},
};
use serde::{Deserialize, Serialize};

use crate::{event::NetEntId, netlib::Tick};

pub struct ProjectilePlugin;

#[derive(Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjSpawnedAt {
    pub tick: Tick,
}

#[derive(Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjDespawn {
    pub tick: Tick,
}

#[derive(Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectileOrigin {
    pub origin: Vec3,
}

#[derive(Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectileSource {
    pub source_entity: NetEntId,
    pub skill: Skill,
    pub skill_source: SkillSource,
}

#[derive(Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectileRealtime {
    pub spawn_real_time: f64,
}

#[derive(Bundle, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectileBundle {
    pub projectile_origin: ProjectileOrigin,
    pub projectile_type: ProjectileAI,
    pub spawned_at: ProjSpawnedAt,
    pub despawn: ProjDespawn,
    pub source: ProjectileSource,
}

#[derive(Bundle, Clone)]
pub struct ProjectileColliderBundle {
    pub sensor: Sensor,
    pub coll_events_enabled: CollisionEventsEnabled,
    pub collider: Collider,
}

#[derive(Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProjectileAI {
    Straight {
        target: Vec3,
    },
    Homing {
        target_entity: NetEntId,
        turn_rate_deg_per_sec: f32,
    },
    Spark {
        projectile_path_targets: Vec<Vec3>,
    },
    HammerDin {
        init_angle_radians: f32,
        speed: f32,
        spiral_width_modifier: f32,
    },
    Frostbolt {
        target: Vec3,
    },
    WinterOrbMain {
        target: Vec3,
    },
    WinterOrbSub {
        target: Vec3,
    },

    BasicBowAttack {
        direction_vector: Vec3,
    },
    RainOfArrowsSpawner {
        ground_target: Vec3,
        sky_target: Vec3,
    },
    RainOfArrowsArrow {
        ground_target: Vec3,
    },
}

impl ProjectileAI {
    pub fn get_collider(&self) -> Option<Collider> {
        Some(match self {
            ProjectileAI::Spark { .. } => Collider::sphere(0.5),
            ProjectileAI::HammerDin { .. } => Collider::sphere(1.0),
            _ => return None,
        })
    }
}

impl Plugin for ProjectilePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SpawnProjectile>().add_systems(
            Update,
            (update_projectiles, despawn_projectile_after_duration),
        );
    }
}

impl SpawnProjectile {
    pub fn base_bundle(&self, tick: &Tick) -> ProjectileBundle {
        ProjectileBundle {
            projectile_type: self.projectile_type.clone(),
            projectile_origin: ProjectileOrigin {
                origin: self.projectile_origin,
            },
            spawned_at: ProjSpawnedAt { tick: *tick },
            despawn: ProjDespawn {
                tick: *tick + Tick(BASE_TICKS_PER_SECOND as u64 * 3),
            },
            source: self.projectile_source.clone(),
        }
    }
    pub fn collider_bundle(&self) -> Option<ProjectileColliderBundle> {
        self.projectile_type
            .get_collider()
            .map(|collider| ProjectileColliderBundle {
                sensor: Sensor,
                coll_events_enabled: CollisionEventsEnabled,
                collider,
            })
    }
}

fn despawn_projectile_after_duration(
    mut commands: Commands,
    query: Query<(Entity, &ProjDespawn)>,
    tick: Res<CurrentTick>,
) {
    for (ent, despawn) in &query {
        if tick.0.0 >= despawn.tick.0 {
            commands.entity(ent).despawn();
        }
    }
}

fn update_projectiles(
    mut query: Query<(
        Entity,
        &mut Transform,
        &ProjectileOrigin,
        &ProjectileRealtime,
        &ProjectileAI,
        &ProjectileSource,
    )>,
    unit_targets: Query<(&Transform, &NetEntId), Without<ProjectileAI>>,
    tick: Res<CurrentTick>,
    time: Res<Time>,
    mut commands: Commands,
    terrain_info: Res<TerrainParams>,
    mut projectile_spawner: MessageWriter<SpawnProjectile>,
) {
    let noise = terrain_info.perlin();
    for (ent, mut transform, origin, real_spawn_time, projectile_ai, projectile_source) in
        &mut query
    {
        let time_since_spawn = time.elapsed_secs_f64() - real_spawn_time.spawn_real_time;
        let dt = time.delta_secs_f64();

        match projectile_ai {
            ProjectileAI::Spark {
                projectile_path_targets,
            } => {
                // the float index we are targeting
                let path_target = time_since_spawn * 5.0;
                let cur_path_index = path_target as usize;
                let pct_through_current_path = path_target - path_target.floor();

                if cur_path_index + 1 >= projectile_path_targets.len() {
                    // despawn
                    commands.entity(ent).despawn();
                    continue;
                }

                let start_pos = projectile_path_targets[cur_path_index];
                let end_pos = projectile_path_targets[cur_path_index + 1];
                let mut new_pos = start_pos.lerp(end_pos, pct_through_current_path as f32);
                let xz = new_pos.xz();

                let y = noise.sample_height(xz.x, xz.y) as f32 * terrain_info.max_height_delta;
                new_pos.y = y + 1.0;

                transform.translation = new_pos;
                transform.rotation =
                    Quat::from_rotation_arc(Vec3::Z, (end_pos - start_pos).normalize());
            }
            ProjectileAI::HammerDin {
                init_angle_radians,
                speed,
                spiral_width_modifier,
            } => {
                let global_hammer_speed = 7.0;
                let angle = init_angle_radians + (time_since_spawn as f32 * speed);
                let angle = angle * global_hammer_speed;
                let radius = time_since_spawn as f32 * speed * spiral_width_modifier;
                let radius = radius * global_hammer_speed;
                let new_x = origin.origin.x + radius * angle.cos();
                let new_z = origin.origin.z + radius * angle.sin();
                let xz = Vec2::new(new_x, new_z);
                let y = noise.sample_height(xz.x, xz.y) as f32 * terrain_info.max_height_delta;
                let new_pos = Vec3::new(new_x, y + 1.0, new_z);

                transform.translation = new_pos;
                let tangent = Vec3::new(-angle.sin(), 0.0, angle.cos()).normalize();
                transform.rotation = Quat::from_rotation_arc(Vec3::Z, tangent);
            }
            ProjectileAI::Straight { target } => {
                let direction = *target - origin.origin;
                let new_pos = origin.origin + direction * time_since_spawn as f32;

                transform.translation = new_pos;
            }
            ProjectileAI::Homing {
                target_entity,
                turn_rate_deg_per_sec: _,
            } => {
                let Some((target_unit_transform, _)) = unit_targets
                    .iter()
                    .find(|(_, net_id)| net_id.0 == target_entity.0)
                else {
                    // Target unit not found, despawn projectile
                    commands.entity(ent).despawn();
                    continue;
                };
                let toward_unit =
                    (target_unit_transform.translation - transform.translation).normalize();
                let speed = 10.0;

                let delta_pos = toward_unit * speed * dt as f32;
                transform.translation += delta_pos;
            }
            ProjectileAI::Frostbolt { target } => {
                let direction = (*target - origin.origin).normalize();
                let new_pos = origin.origin + direction * time_since_spawn as f32;

                let xz = new_pos.xz();
                let y = noise.sample_height(xz.x, xz.y) as f32 * terrain_info.max_height_delta;
                let new_pos = Vec3::new(xz.x, (y + 1.0).max(new_pos.y), xz.y);
                transform.translation = new_pos;
            }
            ProjectileAI::WinterOrbMain { target } => {
                let direction = (*target - origin.origin).normalize();
                let new_pos = origin.origin + direction * time_since_spawn as f32;

                let xz = new_pos.xz();
                let y = noise.sample_height(xz.x, xz.y) as f32 * terrain_info.max_height_delta;
                let new_pos = Vec3::new(xz.x, (y + 1.0).max(new_pos.y), xz.y);
                transform.translation = new_pos;
            }
            ProjectileAI::WinterOrbSub { target } => {
                let direction = (*target - origin.origin).normalize();
                let new_pos = origin.origin + direction * time_since_spawn as f32;

                transform.translation = new_pos;
            }
            ProjectileAI::BasicBowAttack { direction_vector } => {
                // like the straight projectile, but also has an arc for gravity and wind
                let gravity = -9.81;
                let wind = Vec3::new(0.0, 0.0, 0.0);
                let time_f32 = time_since_spawn as f32;
                let arc_offset = Vec3::new(
                    0.5 * wind.x * time_f32 * time_f32,
                    0.5 * gravity * time_f32 * time_f32 + 0.5 * wind.y * time_f32 * time_f32,
                    0.5 * wind.z * time_f32 * time_f32,
                );

                let new_pos = origin.origin + (*direction_vector * time_f32) + arc_offset;

                transform.translation = new_pos;
            }
            ProjectileAI::RainOfArrowsSpawner {
                ground_target,
                sky_target,
            } => {
                //Fly from the origin to the sky target over 0.5 seconds, then despawn
                let flight_duration = 0.5;
                if time_since_spawn >= flight_duration {
                    // despawn
                    commands.entity(ent).despawn();

                    for x in 0..5 {
                        for y in 0..5 {
                            let rand_offset_x =
                                (rand::random_range(-2.0..2.0)) + (x as f32 * 1.0) - 2.0;
                            let rand_offset_z =
                                (rand::random_range(-2.0..2.0)) + (y as f32 * 1.0) - 2.0;
                            let target_xz = Vec2::new(
                                ground_target.x + rand_offset_x,
                                ground_target.z + rand_offset_z,
                            );
                            let y = noise.sample_height(target_xz.x, target_xz.y) as f32
                                * terrain_info.max_height_delta;

                            let ground_target = Vec3::new(target_xz.x, y, target_xz.y);

                            projectile_spawner.write(SpawnProjectile {
                                projectile_type: ProjectileAI::RainOfArrowsArrow {
                                    ground_target: ground_target,
                                },
                                projectile_origin: *sky_target,
                                projectile_source: projectile_source.clone(),
                                spawn_tick: tick.0,
                            });
                        }
                    }

                    continue;
                }
                let pct = (time_since_spawn / flight_duration) as f32;
                let new_pos = origin.origin.lerp(*sky_target, pct);
                transform.translation = new_pos;
            }
            ProjectileAI::RainOfArrowsArrow { ground_target } => {
                // Fall from the sky to the ground with velocity 30m/s + gravity
                let gravity = -9.81;
                let init_vel_y = -30.0;
                let time_f32 = time_since_spawn as f32;
                // now lerp between origin and ground target based on vertical position
                let y_pos =
                    origin.origin.y + init_vel_y * time_f32 + 0.5 * gravity * time_f32 * time_f32;
                if y_pos <= ground_target.y {
                    // landed, leave it in the ground
                } else {
                    let pct = (origin.origin.y - y_pos) / (origin.origin.y - ground_target.y);
                    let new_x = origin.origin.x.lerp(ground_target.x, pct);
                    let new_z = origin.origin.z.lerp(ground_target.z, pct);
                    let new_pos = Vec3::new(new_x, y_pos, new_z);
                    transform.translation = new_pos;
                }
            }
        }
    }
}
