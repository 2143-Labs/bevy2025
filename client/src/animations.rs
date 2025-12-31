use bevy::prelude::*;
use shared::{CurrentTick, event::NetEntId, items::SkillFromSkillSource, net_components::ents::SendNetworkTranformUpdates, netlib::{ClientNetworkingResources, MainServerEndpoint, Tick, send_outgoing_event_next_tick}};

use crate::{game_state::NetworkGameState, ui::skills_menu::binds::BeginSkillUse};

pub struct CharacterAnimationPlugin;

impl Plugin for CharacterAnimationPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(
                Update,
                (
                    // TODO receive new world data at any time?
                    our_client_begin_skill_use,
                )
                    .run_if(in_state(NetworkGameState::ClientConnected)),
            );
    }
}


#[derive(Clone, Component, Debug)]
pub struct UsingSkillSince {
    pub real_time: f64,
    pub tick: Tick,
    pub skill: SkillFromSkillSource,
}

fn our_client_begin_skill_use(
    mut ev_sa: MessageReader<BeginSkillUse>,
    // TODO use another unit query `With`
    mut our_unit: Query<(&NetEntId, Entity, Option<&mut UsingSkillSince>), With<SendNetworkTranformUpdates>>,
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

                let event = shared::netlib::EventToServer::CastSkillUpdate(shared::event::server::CastSkillUpdate {
                    net_ent_id: *ent_id,
                    begin_casting: false,
                    skill: existing_cast.skill.clone(),
                });
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

            let event = shared::netlib::EventToServer::CastSkillUpdate(shared::event::server::CastSkillUpdate {
                net_ent_id: *ent_id,
                begin_casting: true,
                skill: skill.clone(),
            });
            info!(
                "Sending begin skill use {:?} for unit {:?} to server",
                skill, ent_id
            );
            send_outgoing_event_next_tick(&sr, mse.0, &event);
        }
    }
}
