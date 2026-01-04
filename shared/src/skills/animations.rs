use bevy::prelude::*;

use crate::{event::NetEntId, items::SkillFromSkillSource, netlib::Tick, CurrentTick};

pub struct SharedAnimationPlugin;

impl Plugin for SharedAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (remove_old_skills, check_for_skills_that_will_cast),
        )
        .add_message::<UnitFinishedSkillCast>();
    }
}

#[derive(Clone, Component, Debug)]
pub struct UsingSkillSince {
    pub real_time: f64,
    pub tick: Tick,
    pub skill: SkillFromSkillSource,
}

#[derive(Clone, Component, Debug)]
pub struct CastComplete {}

// TODO: Split this into a client guess and server authoritative version
#[derive(Clone, Message, Debug)]
pub struct UnitFinishedSkillCast {
    pub tick: Tick,
    pub net_ent_id: NetEntId,
    pub skill: SkillFromSkillSource,
}

fn check_for_skills_that_will_cast(
    mut query: Query<(Entity, &UsingSkillSince, &NetEntId), Without<CastComplete>>,
    tick: Res<CurrentTick>,
    mut commands: Commands,
    mut unit_finished_skill_cast_writer: MessageWriter<UnitFinishedSkillCast>,
) {
    for (entity, using_skill, ent_id) in query.iter_mut() {
        let frontswing = using_skill.skill.skill.frontswing();
        let windup = using_skill.skill.skill.windup();
        let total_cast_time = (frontswing + windup) as u64;
        let been_casting_ticks = tick.0 - using_skill.tick;

        if been_casting_ticks.0 >= total_cast_time {
            debug!(?ent_id, ?using_skill.skill.skill, "Skill has finished casting, adding CastComplete component");
            commands.entity(entity).insert(CastComplete {});
            unit_finished_skill_cast_writer.write(UnitFinishedSkillCast {
                tick: tick.0,
                net_ent_id: *ent_id,
                skill: using_skill.skill.clone(),
            });
        }
    }
}

fn remove_old_skills(
    mut query: Query<(Entity, &UsingSkillSince, &NetEntId)>,
    tick: Res<CurrentTick>,
    //time: Res<Time>,
    mut commands: Commands,
) {
    for (entity, using_skill, ent_id) in query.iter_mut() {
        let frontswing = using_skill.skill.skill.frontswing();
        let windup = using_skill.skill.skill.windup();
        let winddown = using_skill.skill.skill.winddown();
        let backswing = using_skill.skill.skill.backswing();

        let total_cast_time = frontswing + windup + winddown + backswing;
        let total_cast_time = total_cast_time.try_into().unwrap_or(0);

        let ticks_since_begin = tick.0 - using_skill.tick;
        if ticks_since_begin.0 >= total_cast_time {
            trace!(?ent_id, ?using_skill.skill.skill, "Removing UsingSkillSince component after skill finished");
            // Skill is finished, remove the component
            // We can safely despawn the component here
            commands.entity(entity).remove::<UsingSkillSince>();
            commands.entity(entity).remove::<CastComplete>();
        }
    }
}
