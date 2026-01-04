use bevy::prelude::*;
use noise::NoiseFn;
use shared::{
    BASE_TICKS_PER_SECOND, CurrentTick, event::{NetEntId, UDPacketEvent, client::{CastSkillUpdateToClient, SpawnProjectile}}, net_components::ents::SendNetworkTranformUpdates, netlib::{ClientNetworkingResources, MainServerEndpoint, Tick}, physics::terrain::{NOISE_SCALE_FACTOR, TerrainParams}, projectile::ProjectileRealtime, skills::{
        animations::{CastComplete, SharedAnimationPlugin, UnitFinishedSkillCast, UsingSkillSince},
    }
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
