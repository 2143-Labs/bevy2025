use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{BASE_TICKS_PER_SECOND, items::ItemId};

pub mod animations;


#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Skill {
    /// Launch 4 projectiles in a random path around you
    Spark,

    Hammerdin,

    Frostbolt,

    IceNova,

    WinterOrb,

    Blink,

    TownPortal,

    SummonTestNPC,

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

pub fn all_skills() -> Vec<Skill> {
    vec![
        Skill::Spark,
        Skill::Hammerdin,
        Skill::Frostbolt,
        Skill::IceNova,
        Skill::WinterOrb,
        Skill::Blink,
        Skill::TownPortal,
        Skill::SummonTestNPC,
        Skill::Heal,
        Skill::Revive,
        Skill::BasicBowAttack,
        Skill::RainOfArrows,
        Skill::HomingArrows,
    ]
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
