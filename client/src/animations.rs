use bevy::prelude::*;
use noise::NoiseFn;
use shared::{
    BASE_TICKS_PER_SECOND, CurrentTick,
    event::{NetEntId, UDPacketEvent, client::CastSkillUpdateToClient},
    net_components::ents::SendNetworkTranformUpdates,
    netlib::{ClientNetworkingResources, MainServerEndpoint, Tick},
    physics::terrain::{NOISE_SCALE_FACTOR, TerrainParams},
    skills::{
        ProjDespawn, ProjSpawnedAt, ProjectileAI,
        animations::{CastComplete, SharedAnimationPlugin, UnitFinishedSkillCast, UsingSkillSince},
    },
};

use crate::{
    game_state::NetworkGameState,
    network::{ManHands, ServerTick},
    ui::skills_menu::binds::BeginSkillUse,
};

pub struct CharacterAnimationPlugin;

impl Plugin for CharacterAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SharedAnimationPlugin).add_systems(
            Update,
            (
                // TODO receive new world data at any time?
                our_client_begin_skill_use,
                update_animations,
                on_remove_using_skill_since,
                another_client_begin_skill_use,
                on_unit_finish_cast,
                rotate_and_move_skill_cast_markers,
                on_spawn_projectile,
                update_projectiles,
            )
                .run_if(in_state(NetworkGameState::ClientConnected)),
        );
    }
}

fn our_client_begin_skill_use(
    mut ev_sa: MessageReader<BeginSkillUse>,
    // TODO use another unit query `With`
    mut our_unit: Query<
        (&NetEntId, Entity, Option<&mut UsingSkillSince>),
        With<SendNetworkTranformUpdates>,
    >,
    sr: Res<ClientNetworkingResources>,
    time: Res<Time>,
    tick: Res<CurrentTick>,
    mse: Res<MainServerEndpoint>,
    mut commands: Commands,
) {
    for BeginSkillUse { skill, unit } in ev_sa.read() {
        for (ent_id, entity, maybe_existing_skill) in our_unit.iter_mut() {
            if unit != ent_id {
                continue;
            }

            let new_using_skill = UsingSkillSince {
                real_time: time.elapsed_secs_f64(),
                tick: tick.0,
                skill: skill.clone(),
            };

            // If we are already using a different skill, then we need to send a stop event to the server
            if let Some(mut existing_cast) = maybe_existing_skill {
                if existing_cast.skill == *skill {
                    // Already using this skill, no need to do anything
                    continue;
                }

                let event = shared::netlib::EventToServer::CastSkillUpdate(
                    shared::event::server::CastSkillUpdate {
                        net_ent_id: *ent_id,
                        begin_casting: false,
                        skill: existing_cast.skill.clone(),
                    },
                );
                info!(
                    "Sending stop skill use {:?} for unit {:?} to server",
                    existing_cast.skill, ent_id
                );
                sr.send_outgoing_event_next_tick(mse.0, &event);

                // Finally, we can change this unit to using this skill instead
                *existing_cast = new_using_skill;
            } else {
                commands.entity(entity).insert(new_using_skill);
            }

            let event = shared::netlib::EventToServer::CastSkillUpdate(
                shared::event::server::CastSkillUpdate {
                    net_ent_id: *ent_id,
                    begin_casting: true,
                    skill: skill.clone(),
                },
            );
            info!(
                "Sending begin skill use {:?} for unit {:?} to server",
                skill, ent_id
            );
            sr.send_outgoing_event_next_tick(mse.0, &event);
        }
    }
}

const BASE_LEFT_HAND_POS: Vec3 = Vec3::new(-1.0, 0.0, 1.0);
const BASE_RIGHT_HAND_POS: Vec3 = Vec3::new(1.0, 0.0, 1.0);

fn update_animations(
    mut units: Query<(Entity, &NetEntId, &UsingSkillSince), With<SendNetworkTranformUpdates>>,
    mut children_hands: Query<(&mut Transform, &ChildOf, &ManHands)>,
    time: Res<Time>,
) {
    for (entity, _net_ent_id, using_skill) in units.iter_mut() {
        for (mut hand_transform, child_of, man_hands) in children_hands.iter_mut() {
            if child_of.0 != entity {
                continue;
            }

            let pi = std::f64::consts::PI as f32;
            let rotations_per_second = 2.0;
            let hand_movement_pct = (rotations_per_second
                * (time.elapsed_secs_f64() - using_skill.real_time) as f32
                * 2.0
                * pi)
                .sin()
                / pi
                + 0.5;
            let hand_movement = Vec3::Y * hand_movement_pct;
            let reverse_hand_movement = Vec3::Y - hand_movement;
            if man_hands.is_left {
                let hand_loc = BASE_LEFT_HAND_POS + hand_movement;
                hand_transform.translation = hand_loc;
            } else {
                let hand_loc = BASE_RIGHT_HAND_POS + reverse_hand_movement;
                hand_transform.translation = hand_loc;
            }
        }
    }
}

fn on_remove_using_skill_since(
    mut removed_units: RemovedComponents<UsingSkillSince>,
    mut children_hands: Query<(&mut Transform, &ChildOf, &ManHands)>,
) {
    for entity in removed_units.read() {
        for (mut hand_transform, child_of, man_hands) in children_hands.iter_mut() {
            if child_of.0 != entity {
                continue;
            }
            if man_hands.is_left {
                hand_transform.translation = BASE_LEFT_HAND_POS;
            } else {
                hand_transform.translation = BASE_RIGHT_HAND_POS;
            }
        }
    }
}

/// Take the given input tick, &Time, &ServerTick, and &CurrentTick, return the projected client
/// tick and real client time to spawn the projectile at.
pub fn get_client_tick_from_server_tick(
    input_tick: &Tick,
    time: &Time,
    tick: &CurrentTick,
    server_tick: &ServerTick,
) -> (f64, Tick) {
    // TODO no idea if this math is correct tbh but works?
    let projected_client_tick = input_tick.0 as i64 - server_tick.tick_offset;
    let subtick_offset = time.elapsed_secs_f64() - server_tick.realtime;
    let num_ticks_ago = tick.0.0 as i64 - projected_client_tick;
    if num_ticks_ago < 0 {
        warn!("Received CastSkillUpdateToClient for future tick?");
    }
    if num_ticks_ago > 2 {
        warn!("Received CastSkillUpdateToClient for tick >2 ticks ago?");
    }

    let real_time = time.elapsed_secs_f64()
        - (num_ticks_ago as f64 * BASE_TICKS_PER_SECOND as f64)
        - subtick_offset;

    (real_time, Tick(projected_client_tick as _))
}

fn another_client_begin_skill_use(
    mut reader: UDPacketEvent<CastSkillUpdateToClient>,
    mut our_unit: Query<
        (&NetEntId, Entity, Option<&mut UsingSkillSince>),
        With<SendNetworkTranformUpdates>,
    >,
    //sr: Res<ClientNetworkingResources>,
    time: Res<Time>,
    tick: Res<CurrentTick>,
    server_tick: Res<ServerTick>,
    //mse: Res<MainServerEndpoint>,
    mut commands: Commands,
) {
    for packet in reader.read() {
        for (ent_id, entity, maybe_existing_skill) in our_unit.iter_mut() {
            if packet.event.net_ent_id != *ent_id {
                continue;
            }

            let (real_time, projected_tick) = get_client_tick_from_server_tick(
                &packet.event.begin_casting_tick,
                &time,
                &tick,
                &server_tick,
            );

            let new_using_skill = UsingSkillSince {
                real_time,
                tick: projected_tick,
                skill: packet.event.skill.clone(),
            };

            if let Some(mut existing_cast) = maybe_existing_skill {
                if existing_cast.skill == packet.event.skill {
                    // Already using this skill, no need to do anything
                    continue;
                }

                *existing_cast = new_using_skill;
                commands.entity(entity).remove::<CastComplete>();
            } else {
                commands.entity(entity).insert(new_using_skill);
                commands.entity(entity).remove::<CastComplete>();
            }
        }
    }
}

#[derive(Clone, Component, Debug)]
/// temp flair to show when a user has finished casting
pub struct SkillCastMarker {
    pub spin: f32,
    pub time_created: f64,
    pub speed: f32,
}

fn on_unit_finish_cast(
    mut cast_event_reader: MessageReader<UnitFinishedSkillCast>,
    query: Query<(&Transform, &NetEntId), With<UsingSkillSince>>,
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for UnitFinishedSkillCast {
        tick: _,
        net_ent_id,
        skill: _,
    } in cast_event_reader.read()
    {
        for (transform, ent_id) in &query {
            if net_ent_id != ent_id {
                continue;
            }

            info!(
                ?net_ent_id,
                ?transform.translation,
                "Unit finished skill cast at position"
            );
            //spawn 10 skill cast markers above their head
            for _i in 0..1 {
                commands.spawn((
                    Transform::from_translation(transform.translation + Vec3::Y * 2.0)
                        .with_rotation(Quat::from_axis_angle(
                            Vec3::from_array([
                                rand::random_range(-1.0..1.0),
                                rand::random_range(-1.0..1.0),
                                rand::random_range(-1.0..1.0),
                            ])
                            .normalize(),
                            rand::random_range(0.0..std::f32::consts::PI * 2.0),
                        )),
                    SkillCastMarker {
                        spin: rand::random_range(45.0..400.0),
                        time_created: time.elapsed_secs_f64(),
                        speed: rand::random_range(0.5..2.0),
                    },
                    Mesh3d(meshes.add(Mesh::from(Tetrahedron {
                        vertices: [
                            Vec3::new(0.0, 0.5, 0.0),
                            Vec3::new(-0.5, -0.5, 0.5),
                            Vec3::new(0.5, -0.5, 0.5),
                            Vec3::new(0.0, -0.5, -0.5),
                        ],
                    }))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::linear_rgb(
                            rand::random_range(0.5..1.0),
                            rand::random_range(0.5..1.0),
                            rand::random_range(0.5..1.0),
                        ),
                        unlit: true,
                        ..Default::default()
                    })),
                ));
            }
        }
    }
}

const MAX_TIME_FOR_SKILL_CAST_MARKER: f64 = 3.0;

fn rotate_and_move_skill_cast_markers(
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &SkillCastMarker)>,
    mut commands: Commands,
) {
    for (ent, mut transform, marker) in &mut query {
        if time.elapsed_secs_f64() - marker.time_created > MAX_TIME_FOR_SKILL_CAST_MARKER {
            commands.entity(ent).despawn();
            continue;
        }

        let rot = Quat::from_axis_angle(Vec3::Y, marker.spin * time.delta_secs());
        transform.rotation *= rot;
        transform.translation += rot * Vec3::Y * marker.speed * time.delta_secs();
    }
}

fn on_spawn_projectile(
    mut spawn_event_reader: UDPacketEvent<shared::event::client::SpawnProjectile>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tick: Res<CurrentTick>,
    server_tick: Res<ServerTick>,
    time: Res<Time>,
) {
    for event in spawn_event_reader.read() {
        info!(?event.event.projectile_type, ?event.event.projectile_origin, "Spawning projectile");
        let (real_time, tick) =
            get_client_tick_from_server_tick(&event.event.spawn_tick, &time, &tick, &server_tick);

        let mesh_handle = match event.event.projectile_type {
            ProjectileAI::Spark { .. } => meshes.add(Mesh::from(Tetrahedron {
                vertices: [
                    Vec3::new(0.0, 0.5, 0.0),
                    Vec3::new(-0.5, -0.5, 0.5),
                    Vec3::new(0.5, -0.5, 0.5),
                    Vec3::new(0.0, -0.5, -0.5),
                ],
            })),
            ProjectileAI::HammerDin { .. } => meshes.add(Mesh::from(Sphere { radius: 1.0 })),
            _ => {
                error!("Unknown projectile type for mesh!");
                continue;
            }
        };

        commands.spawn((
            Transform::from_translation(event.event.projectile_origin),
            //ProjectileAI
            event.event.projectile_type.clone(),
            ProjSpawnedAt {
                tick,
                time: real_time,
            },
            ProjDespawn {
                tick: tick + Tick(300), // despawn after 5 seconds
            },
            // Basic equilateral tetrahedron mesh
            Mesh3d(mesh_handle),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::linear_rgb(1.0, 0.5, 0.0),
                unlit: true,
                ..Default::default()
            })),
        ));
    }
}

fn update_projectiles(
    mut query: Query<(
        Entity,
        &mut Transform,
        &ProjectileAI,
        &ProjSpawnedAt,
        &ProjDespawn,
    )>,
    tick: Res<CurrentTick>,
    time: Res<Time>,
    mut commands: Commands,
    terrain_info: Res<TerrainParams>,
) {
    let noise: noise::Perlin = terrain_info.perlin();
    for (ent, mut transform, projectile_ai, spawned_at, despawn) in &mut query {
        let time_since_spawn = time.elapsed_secs_f64() - spawned_at.time;
        if tick.0.0 >= despawn.tick.0 {
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
                center_point,
                init_angle_radians,
                speed,
                spiral_width_modifier,
            } => {
                let global_hammer_speed = 7.0;
                let angle = init_angle_radians + (time_since_spawn as f32 * speed);
                let angle = angle * global_hammer_speed;
                let radius = time_since_spawn as f32 * speed * spiral_width_modifier;
                let radius = radius * global_hammer_speed;
                let new_x = center_point.x + radius * angle.cos();
                let new_z = center_point.z + radius * angle.sin();
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
