use serde::{Deserialize, Serialize};

use crate::{
    items::{HasItemLayout, ItemLayout},
    skills::Skill,
    stats::{HasMods, Mod},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Footwear {
    LeatherBoots,
    Sandals,
    Wraps,
}

impl HasMods for Footwear {
    fn get_mods(&self) -> Vec<Mod> {
        match self {
            Footwear::LeatherBoots => vec![
                Mod::AddsJumpHeight {
                    amount: 15.0.into(),
                },
                Mod::AddsDamageReduction {
                    amount: 10.0.into(),
                },
                Mod::AddsMovementSpeed {
                    amount: 20.0.into(),
                },
            ],
            Footwear::Sandals => vec![Mod::AddsMovementSpeed {
                amount: 10.0.into(),
            }],
            Footwear::Wraps => vec![
                Mod::AddsJumpHeight {
                    amount: 25.0.into(),
                },
                Mod::AddsMovementSpeed {
                    amount: 30.0.into(),
                },
            ],
        }
    }

    fn grants_skills(&self) -> Vec<Skill> {
        match self {
            Footwear::LeatherBoots => vec![],
            Footwear::Sandals => vec![Skill::RainOfArrows],
            Footwear::Wraps => vec![],
        }
    }
}

impl HasItemLayout for Footwear {
    fn get_item_layout(&self) -> &ItemLayout {
        match self {
            Footwear::LeatherBoots => &ItemLayout::Rectangular {
                width: 2,
                height: 2,
            },
            Footwear::Sandals => &ItemLayout::Rectangular {
                width: 2,
                height: 2,
            },
            Footwear::Wraps => &ItemLayout::Rectangular {
                width: 2,
                height: 2,
            },
        }
    }
}
