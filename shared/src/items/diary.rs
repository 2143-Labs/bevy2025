use serde::{Deserialize, Serialize};

use crate::{skills::Skill, stats::HasMods};

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

impl HasMods for DiaryBook {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DiaryPage {
    Ranger,
    Melee,
    Spellcasting,

    Scavenger,
    MartialTraining,
    Healing,

    Omniscience,
}

impl HasMods for DiaryPage {
    fn grants_skills(&self) -> Vec<Skill> {
        match self {
            DiaryPage::Ranger => vec![
                Skill::BasicBowAttack,
                Skill::RainOfArrows,
                Skill::HomingArrows,
            ],
            DiaryPage::Melee => vec![],
            DiaryPage::Spellcasting => vec![Skill::Spark],
            DiaryPage::Scavenger => vec![],
            DiaryPage::MartialTraining => vec![],
            DiaryPage::Healing => vec![Skill::Heal, Skill::Revive],
            DiaryPage::Omniscience => crate::skills::all_skills(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EnemyDiaryPage {
    Goblin,
}

impl HasMods for EnemyDiaryPage {}
