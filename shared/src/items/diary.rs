use serde::{Deserialize, Serialize};

use crate::stats::HasMods;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DiaryBook {
    /// The first diary you recieve, has 2 page slots
    Basic,
    /// 2 Combat focused pages, one utility
    BasicMartial,
    /// Spellbook, 5x spellcasting pages
    Spellbook1,

    /// 2x mininon slot, 1x any
    SummonerSpellbook,
}

impl HasMods for DiaryBook {
    fn get_mods(&self) -> Vec<crate::stats::Mod> {
        return vec![];
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DiaryPage {
    Ranger,
    Melee,
    Spellcasting,

    Scavenger,
    MartialTraining,
    Healing,
}

impl HasMods for DiaryPage {
    fn get_mods(&self) -> Vec<crate::stats::Mod> {
        return vec![];
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EnemyDiaryPage {
    Goblin,
}

impl HasMods for EnemyDiaryPage {
    fn get_mods(&self) -> Vec<crate::stats::Mod> {
        return vec![];
    }
}
