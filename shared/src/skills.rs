use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{event::NetEntId, items::ItemId, netlib::Tick, BASE_TICKS_PER_SECOND};

pub mod animations;

#[derive(Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjSpawnedAt {
    pub tick: Tick,
    pub time: f64,
}

#[derive(Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjDespawn {
    pub tick: Tick,
}

#[derive(Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProjectileAI {
    Straight {
        direction_vector: Vec3,
    },
    Homing {
        target_entity: NetEntId,
        turn_rate_deg_per_sec: f32,
    },
    Spark {
        projectile_path_targets: Vec<Vec3>,
    },
    HammerDin {
        center_point: Vec3,
        init_angle_radians: f32,
        speed: f32,
        spiral_width_modifier: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Skill {
    /// Launch 4 projectiles in a random path around you
    Spark,

    Hammerdin,

    /// Heal Targeted ally
    Heal,

    /// Revive target ally
    Revive,

    /// Fire an arrow from a bow: Hold down left click to charge and release to fire
    BasicBowAttack,

    /// Fire a volley of arrows in an AOE
    RainOfArrows,

    /// After hitting a target, fire homing bolts for up to 5 seconds
    HomingArrows,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SkillSource {
    Item(ItemId),
    Other,
}

fn ticks_from_secs(secs: f32) -> i16 {
    (secs * BASE_TICKS_PER_SECOND as f32) as i16
}

// TODO look into these timings
impl Skill {
    /// Start of skill, cancellable
    /// Returns the duration of the skill in ticks
    pub fn frontswing(&self) -> i16 {
        match self {
            Skill::Spark => ticks_from_secs(0.25),
            Skill::Heal => ticks_from_secs(3.0),
            Skill::Revive => ticks_from_secs(5.0),
            Skill::BasicBowAttack => ticks_from_secs(0.3),
            Skill::RainOfArrows => ticks_from_secs(1.5),
            Skill::HomingArrows => ticks_from_secs(0.7),
            _ => ticks_from_secs(0.1),
        }
    }

    /// Start of skill, now cannot be cancelled
    /// Returns the duration of the skill in ticks
    pub fn windup(&self) -> i16 {
        match self {
            Skill::Spark => ticks_from_secs(0.1),
            Skill::Heal => ticks_from_secs(1.0),
            Skill::Revive => ticks_from_secs(2.0),
            Skill::BasicBowAttack => ticks_from_secs(0.2),
            Skill::RainOfArrows => ticks_from_secs(1.0),
            Skill::HomingArrows => ticks_from_secs(0.3),
            _ => ticks_from_secs(0.1),
        }
    }

    /// Skill effect just occured, now you are locked into this animation for this time
    /// Returns the duration of the skill in ticks
    pub fn winddown(&self) -> i16 {
        match self {
            Skill::Spark => ticks_from_secs(0.2),
            Skill::Heal => ticks_from_secs(1.0),
            Skill::Revive => ticks_from_secs(1.0),
            Skill::BasicBowAttack => ticks_from_secs(0.2),
            Skill::RainOfArrows => ticks_from_secs(0.5),
            Skill::HomingArrows => ticks_from_secs(0.4),
            _ => ticks_from_secs(0.1),
        }
    }

    /// Skill effect fully over, can act again
    /// Returns the duration of the skill in ticks
    pub fn backswing(&self) -> i16 {
        match self {
            Skill::Spark => ticks_from_secs(0.1),
            Skill::Heal => ticks_from_secs(0.5),
            Skill::Revive => ticks_from_secs(0.5),
            Skill::BasicBowAttack => ticks_from_secs(0.1),
            Skill::RainOfArrows => ticks_from_secs(0.3),
            Skill::HomingArrows => ticks_from_secs(0.2),
            _ => ticks_from_secs(0.1),
        }
    }
}
