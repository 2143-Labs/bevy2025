use avian3d::prelude::{Collider, CollisionEventsEnabled, Sensor};
use bevy::prelude::*;

use noise::NoiseFn;

use crate::{
    BASE_TICKS_PER_SECOND, CurrentTick, event::client::SpawnProjectile, physics::terrain::{NOISE_SCALE_FACTOR, TerrainParams}, skills::{Skill, SkillSource}
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
            ProjectileAI::Spark { .. } => Collider::sphere(0.2),
            ProjectileAI::HammerDin { .. } => Collider::sphere(0.3),
            _ => return None,
        })
    }
}

impl Plugin for ProjectilePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SpawnProjectile>()
            .add_systems(Update, update_projectiles);
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
        self.projectile_type.get_collider().map(|collider|
            ProjectileColliderBundle {
                sensor: Sensor,
                coll_events_enabled: CollisionEventsEnabled,
                collider,
            }
        )
    }
}

fn update_projectiles(
    mut query: Query<(
        Entity,
        &mut Transform,
        &ProjectileOrigin,
        &ProjectileRealtime,
        &ProjectileAI,
        //&ProjSpawnedAt,
        &ProjDespawn,
    )>,
    tick: Res<CurrentTick>,
    time: Res<Time>,
    mut commands: Commands,
    terrain_info: Res<TerrainParams>,
) {
    let noise: noise::Perlin = terrain_info.perlin();
    for (ent, mut transform, origin, real_spawn_time, projectile_ai, despawn) in
        &mut query
    {
        let time_since_spawn = time.elapsed_secs_f64() - real_spawn_time.spawn_real_time;
        if tick.0 .0 >= despawn.tick.0  {
            // despawn
            commands.entity(ent).despawn();
            continue;
        }

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
                // TODO REFACTOR PAIR TER1
                let y = noise.get([
                    xz.x as f64 * NOISE_SCALE_FACTOR,
                    xz.y as f64 * NOISE_SCALE_FACTOR,
                ]) as f32
                    * terrain_info.max_height_delta;
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
                // TODO REFACTOR PAIR TER1
                let y = noise.get([
                    xz.x as f64 * NOISE_SCALE_FACTOR,
                    xz.y as f64 * NOISE_SCALE_FACTOR,
                ]) as f32
                    * terrain_info.max_height_delta;
                let new_pos = Vec3::new(new_x, y + 1.0, new_z);

                transform.translation = new_pos;
                let tangent = Vec3::new(-angle.sin(), 0.0, angle.cos()).normalize();
                transform.rotation = Quat::from_rotation_arc(Vec3::Z, tangent);
            }
            _ => {
                // TODO
            }
        }
    }
}
