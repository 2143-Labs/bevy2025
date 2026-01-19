use std::sync::Arc;

use crate::{decimal::Decimal, items::{Inventory, Item, SkillFromSkillSource}, stats2::FinalPermanantStats};

impl SkillFromSkillSource {
    fn calc_skill_damage(&self, player_inventory: Arc<Inventory<Item>>, player_stats: &FinalPermanantStats, player_buffs: ()) -> Decimal {
        let equipped_items = player_inventory.items.iter().filter(|itemi| itemi.item.is_equipped());
        let base_damage = self.skill.base_damage();

        for itemi in equipped_items {
            for item_mod in itemi.item.mods() {
            }

        }


    }
}

