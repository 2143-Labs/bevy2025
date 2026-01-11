//! This is the stats componenet module.

use crate::decimal::Decimal;
use serde::{Deserialize, Serialize};
use bevy::prelude::*;

// Ever players
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PermanantStats {
    /// Your stamina- affects how long you can run
    pub stamina: Decimal,
    /// Acuity affects your ability pool for most classes
    pub acuity: Decimal,
    /// Belief affects your regen for most classes
    pub belief: Decimal,
    /// Precision allows you to wield smaller and more complicated weapons
    pub precision: Decimal,
    /// Your capacity to weild heavy weapons and armor
    pub brawn: Decimal,
}

/// Every player will have a base permanat stat component, which plus items + buffs creates
/// [FinalPermanantStats]
#[derive(Component, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BasePermanantStats(pub PermanantStats);

#[derive(Component, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FinalPermanantStats(pub PermanantStats);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stat {
    Stamina,
    Acuity,
    Belief,
    Precision,
    Brawn,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModStat {
    pub stat: Stat,
    pub amount: Decimal,
}

impl std::ops::Add<ModStat> for PermanantStats {
    type Output = PermanantStats;

    fn add(self, rhs: ModStat) -> Self::Output {
        let mut new_stats = self;
        match rhs.stat {
            Stat::Stamina => new_stats.stamina += rhs.amount,
            Stat::Acuity => new_stats.acuity += rhs.amount,
            Stat::Belief => new_stats.belief += rhs.amount,
            Stat::Precision => new_stats.precision += rhs.amount,
            Stat::Brawn => new_stats.brawn += rhs.amount,
        }
        new_stats
    }
}

impl std::ops::Sub<ModStat> for PermanantStats {
    type Output = PermanantStats;

    fn sub(self, rhs: ModStat) -> Self::Output {
        let mut new_stats = self;
        match rhs.stat {
            Stat::Stamina => new_stats.stamina -= rhs.amount,
            Stat::Acuity => new_stats.acuity -= rhs.amount,
            Stat::Belief => new_stats.belief -= rhs.amount,
            Stat::Precision => new_stats.precision -= rhs.amount,
            Stat::Brawn => new_stats.brawn -= rhs.amount,
        }
        new_stats
    }
}

impl Default for PermanantStats {
    fn default() -> Self {
        PermanantStats {
            stamina: Decimal::zero(),
            acuity: Decimal::zero(),
            belief: Decimal::zero(),
            precision: Decimal::zero(),
            brawn: Decimal::zero(),
        }
    }
}
