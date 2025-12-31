use bevy::prelude::*;
use shared::{
    CurrentTick,
    event::NetEntId,
    net_components::ents::SendNetworkTranformUpdates,
    netlib::{ClientNetworkingResources, MainServerEndpoint, send_outgoing_event_next_tick},
    skills::animations::{SharedAnimationPlugin, UsingSkillSince},
};

use crate::{
    game_state::NetworkGameState, network::ManHands, ui::skills_menu::binds::BeginSkillUse,
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
                send_outgoing_event_next_tick(&sr, mse.0, &event);

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
            send_outgoing_event_next_tick(&sr, mse.0, &event);
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
