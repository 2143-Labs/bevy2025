use std::sync::Arc;

use crate::{decimal::Decimal, items::{Inventory, Item, SkillFromSkillSource}, stats2::FinalPermanantStats};
use crate::stats::HasMods;

impl SkillFromSkillSource {
    pub fn calc_skill_damage(&self, player_inventory: Arc<Inventory<Item>>, _player_stats: &FinalPermanantStats, _player_buffs: ()) -> Decimal {
        let equipped_items = player_inventory.items.iter().filter(|itemi| itemi.item.is_equipped());
        let base_damage = self.skill.base_damage();

        for itemi in equipped_items {
            for _item_mod in itemi.item.get_mods() {
                // TODO: Apply item mods to damage calculation
            }
        }

        // TODO: Apply player stats and buffs to damage calculation

        base_damage
    }
}

