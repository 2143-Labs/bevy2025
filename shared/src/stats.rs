use crate::{decimal::Decimal, skills::Skill};
use serde::{Deserialize, Serialize};

pub struct PlayerFinalStats {
    pub max_health: Decimal,
    pub movement_mods: Vec<MovementModifier>,
    pub defense_mods: Vec<DefenseModifier>,
    pub resistance_mods: Vec<ResistanceModifier>,
    pub buffs: Vec<Buff>,
    pub defense: Decimal,
}

impl Default for PlayerFinalStats {
    fn default() -> Self {
        PlayerFinalStats {
            max_health: Decimal::new(100.0),
            movement_mods: vec![],
            defense_mods: vec![],
            resistance_mods: vec![],
            buffs: vec![],
            defense: Decimal::new(0.0),
        }
    }
}

pub enum MovementModifier {
    MomementSpeed(Decimal),
    JumpHeight(Decimal),
}

pub enum DefenseModifier {
    DamageReduction(Decimal),
    HealthRegen(Decimal),
    DamageReflection(Decimal),
}

pub enum ResistanceModifier {
    ProjectileResistance(Decimal),
    MagicResistance(Decimal),
}

pub enum Buff {}

pub trait HasMods {
    fn get_mods(&self) -> Vec<Mod> {
        vec![]
    }
    fn grants_skills(&self) -> Vec<Skill> {
        vec![]
    }
}

/// An item mod changes any target for which it is equipped
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Mod {
    AddsLife { amount: Decimal },
    AddsMovementSpeed { amount: Decimal },
    AddsDamageReduction { amount: Decimal },
    AddsProjectileResistance { amount: Decimal },
    AddsMagicResistance { amount: Decimal },
    AddsJumpHeight { amount: Decimal },
    AddsDamageReflection { amount: Decimal },
}
