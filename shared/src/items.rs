use std::sync::{Arc, RwLock};

use bevy_ecs::resource::Resource;
use serde::{Deserialize, Serialize};

use crate::stats::{HasMods, Mod, PlayerFinalStats};

pub mod diary;
pub mod footwear;
pub mod page {
    pub mod ranger_page;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inventory<ItemRepr> {
    pub id: InventoryId,
    pub items: Vec<ItemInInventory<ItemRepr>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemInInventory<ItemRepr> {
    pub item: ItemRepr,
    pub stacksize: u16,
    pub item_placement: ItemPlacement,
}

impl ItemInInventory<ItemId> {
    pub fn as_full_item(&self, cache: &InventoryItemCache) -> ItemInInventory<Item> {
        let cache_read = cache.item_cache.read().unwrap();
        ItemInInventory {
            stacksize: self.stacksize,
            item_placement: self.item_placement.clone(),
            item: (*cache_read.get(&self.item).unwrap()).as_ref().clone(),
        }
    }
}

impl Inventory<ItemId> {
    pub fn as_full_inventory(&self, cache: &InventoryItemCache) -> Inventory<Item> {
        Inventory {
            id: self.id,
            items: self
                .items
                .iter()
                .map(|item_in_inv| item_in_inv.as_full_item(cache))
                .collect(),
        }
    }
}

impl Inventory<Item> {
    pub fn get_equipped_skills(&self) -> Vec<crate::skills::Skill> {
        let mut skills = vec![];

        for inv_item in &self.items {
            let Some(_equip_slot) =
                inv_item
                    .item
                    .data
                    .item_misc
                    .iter()
                    .find_map(|misc| match misc {
                        ItemMiscModifiers::Equipped(i) => Some(i),
                        _ => None,
                    })
            else {
                // not equipped, skip this item
                continue;
            };

            #[cfg(test)]
            println!("Equipped item found: {:?}", inv_item);

            let item_skills = inv_item.item.grants_skills();

            #[cfg(test)]
            println!("Item grants skills: {:?}", item_skills);

            for skill in item_skills {
                skills.push(skill);
            }
        }

        skills
    }
}

// TODO this is a very basic imlementation, needs to be expanded
impl Inventory<Item> {
    pub fn get_player_stats(&self) -> PlayerFinalStats {
        let mut final_stats = PlayerFinalStats::default();

        let mut delta_max_health = crate::decimal::Decimal::new(0.0);
        let mut delta_movement_speed = crate::decimal::Decimal::new(0.0);
        let mut delta_jump_height = crate::decimal::Decimal::new(0.0);
        let mut delta_damage_reduction = crate::decimal::Decimal::new(0.0);
        let mut delta_health_regen = crate::decimal::Decimal::new(0.0);
        let mut delta_damage_reflection = crate::decimal::Decimal::new(0.0);
        let mut delta_projectile_resistance = crate::decimal::Decimal::new(0.0);
        let mut delta_magic_resistance = crate::decimal::Decimal::new(0.0);

        for inv_item in &self.items {
            let Some(_equip_slot) =
                inv_item
                    .item
                    .data
                    .item_misc
                    .iter()
                    .find_map(|misc| match misc {
                        crate::items::ItemMiscModifiers::Equipped(i) => Some(i),
                        _ => None,
                    })
            else {
                // not equipped, skip this item
                continue;
            };

            let mods = inv_item.item.get_mods();
            for modifier in mods {
                match modifier {
                    Mod::AddsLife { amount } => {
                        delta_max_health += amount;
                    }
                    Mod::AddsMovementSpeed { amount } => {
                        delta_movement_speed += amount;
                    }
                    Mod::AddsJumpHeight { amount } => {
                        delta_jump_height += amount;
                    }
                    Mod::AddsDamageReduction { amount } => {
                        delta_damage_reduction += amount;
                    }
                    Mod::AddsDamageReflection { amount } => {
                        delta_damage_reflection += amount;
                    }
                    Mod::AddsProjectileResistance { amount } => {
                        delta_projectile_resistance += amount;
                    }
                    Mod::AddsMagicResistance { amount } => {
                        delta_magic_resistance += amount;
                    }
                }
            }
        }

        delta_health_regen += crate::decimal::Decimal::new(0.5) * delta_max_health;

        if delta_max_health != crate::decimal::Decimal::new(0.0) {
            final_stats.max_health += delta_max_health;
        }

        if delta_movement_speed != crate::decimal::Decimal::new(0.0) {
            final_stats
                .movement_mods
                .push(crate::stats::MovementModifier::MomementSpeed(
                    delta_movement_speed,
                ));
        }
        if delta_jump_height != crate::decimal::Decimal::new(0.0) {
            final_stats
                .movement_mods
                .push(crate::stats::MovementModifier::JumpHeight(
                    delta_jump_height,
                ));
        }
        if delta_damage_reduction != crate::decimal::Decimal::new(0.0) {
            final_stats
                .defense_mods
                .push(crate::stats::DefenseModifier::DamageReduction(
                    delta_damage_reduction,
                ));
        }
        if delta_health_regen != crate::decimal::Decimal::new(0.0) {
            final_stats
                .defense_mods
                .push(crate::stats::DefenseModifier::HealthRegen(
                    delta_health_regen,
                ));
        }
        if delta_damage_reflection != crate::decimal::Decimal::new(0.0) {
            final_stats
                .defense_mods
                .push(crate::stats::DefenseModifier::DamageReflection(
                    delta_damage_reflection,
                ));
        }
        if delta_projectile_resistance != crate::decimal::Decimal::new(0.0) {
            final_stats.resistance_mods.push(
                crate::stats::ResistanceModifier::ProjectileResistance(delta_projectile_resistance),
            );
        }
        if delta_magic_resistance != crate::decimal::Decimal::new(0.0) {
            final_stats
                .resistance_mods
                .push(crate::stats::ResistanceModifier::MagicResistance(
                    delta_magic_resistance,
                ));
        }

        final_stats
    }
}

// TODO always growing
impl InventoryItemCache {
    pub fn new() -> Self {
        InventoryItemCache {
            ..Default::default()
        }
    }

    pub fn insert_item(&self, item: Item) {
        let mut cache_write = self.item_cache.write().unwrap();
        cache_write.insert(item.item_id, Arc::new(item));
    }

    pub fn insert_inventory(&self, inventory: Inventory<Item>) {
        let mut cache_write = self.inventory_cache.write().unwrap();
        cache_write.insert(inventory.id, Arc::new(inventory));
    }

    pub fn get_inventory(&self, inventory_id: &InventoryId) -> Option<Arc<Inventory<Item>>> {
        let cache_read = self.inventory_cache.read().unwrap();
        cache_read.get(inventory_id).cloned()
    }

    pub fn get_item(&self, item_id: &ItemId) -> Option<Arc<Item>> {
        let cache_read = self.item_cache.read().unwrap();
        cache_read.get(item_id).cloned()
    }

    pub fn clear(&self) {
        let mut cache_write = self.item_cache.write().unwrap();
        cache_write.clear();
        let mut inv_cache_write = self.inventory_cache.write().unwrap();
        inv_cache_write.clear();
    }
}

#[derive(Clone, Default, Resource)]
pub struct InventoryItemCache {
    item_cache: Arc<RwLock<std::collections::HashMap<ItemId, Arc<Item>>>>,
    inventory_cache: Arc<RwLock<std::collections::HashMap<InventoryId, Arc<Inventory<Item>>>>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ItemLayout {
    /// A full rectangular item
    Rectangular { width: u8, height: u8 },
    /// A rectangular item with a custom shape defined by a mask: true = occupied, false = empty,
    /// row-major order (left to right, top to bottom)
    RectWithMask {
        width: u8,
        height: u8,
        mask: Vec<bool>,
    },
    /// Sparse item with individual occupied slots defined by coordinates
    Sparse { occupied_slots: Vec<(u8, u8)> },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ItemPlacement {
    pub flipped: bool,
    pub rotated: u8,
    // max: 13 bits
    pub slot_index: u16,
}

// totally unnecessary optimization to pack into 2 bytes
impl Serialize for ItemPlacement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let flipped_bit = if self.flipped { 1u16 } else { 0 } << 15;
        let rotated_bits = (self.rotated as u16 & 0b11) << 14;
        let slot_index_bits = self.slot_index & (0x3FFF);
        serializer.serialize_u16(flipped_bit | rotated_bits | slot_index_bits)
    }
}

impl<'de> Deserialize<'de> for ItemPlacement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = u16::deserialize(deserializer)?;
        let flipped = (bits & 0x8000) != 0;
        let rotated = ((bits & 0x6000) >> 14) as u8;
        let slot_index = bits & 0x3FFF;
        Ok(ItemPlacement {
            flipped,
            rotated,
            slot_index,
        })
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct InventoryId(pub u128);

impl Default for InventoryId {
    fn default() -> Self {
        let current_unix_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let get_server_id = || -> u16 {
            // TODO get actual server ID
            0u16
        };

        let rng = rand::random_range(0u64..(u64::MAX << 16));
        let inventory_id =
            ((current_unix_time as u128) << 64) | ((get_server_id() as u128) << 48) | (rng as u128);

        InventoryId(inventory_id)
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ItemId(pub u128);

impl Default for ItemId {
    fn default() -> Self {
        Self(InventoryId::default().0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ItemData {
    pub item_base: BaseItem,
    // TODO replace this with a vec of statlines
    pub mods: Vec<Mod>,
    pub item_misc: Vec<ItemMiscModifiers>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Item {
    pub item_id: ItemId,
    pub data: ItemData,
}

impl HasMods for Item {
    fn get_mods(&self) -> Vec<Mod> {
        self.data.get_mods()
    }
    fn grants_skills(&self) -> Vec<crate::skills::Skill> {
        self.data.grants_skills()
    }
}

impl HasMods for ItemData {
    fn get_mods(&self) -> Vec<Mod> {
        let mut all_mods = self.mods.clone();
        let base_mods = self.item_base.get_mods();
        all_mods.extend(base_mods);
        all_mods
    }
    fn grants_skills(&self) -> Vec<crate::skills::Skill> {
        self.item_base.grants_skills()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BaseItem {
    CurrencyPiece,
    DiaryPage(diary::DiaryPage),
    DiaryBook(diary::DiaryBook),
    EnemyDiaryPage(diary::EnemyDiaryPage),
    Footwear(footwear::Footwear),
}

pub trait HasItemLayout {
    fn get_item_layout(&self) -> &ItemLayout;
}

impl HasMods for BaseItem {
    fn get_mods(&self) -> Vec<Mod> {
        match self {
            BaseItem::CurrencyPiece => vec![],
            BaseItem::DiaryPage(page) => page.get_mods(),
            BaseItem::DiaryBook(book) => book.get_mods(),
            BaseItem::EnemyDiaryPage(page) => page.get_mods(),
            BaseItem::Footwear(footwear) => footwear.get_mods(),
        }
    }
    fn grants_skills(&self) -> Vec<crate::skills::Skill> {
        match self {
            BaseItem::CurrencyPiece => vec![],
            BaseItem::DiaryPage(page) => page.grants_skills(),
            BaseItem::DiaryBook(book) => book.grants_skills(),
            BaseItem::EnemyDiaryPage(page) => page.grants_skills(),
            BaseItem::Footwear(footwear) => footwear.grants_skills(),
        }
    }
}

/// These are properties that may affect an item but are not directly related to stats or mods
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ItemMiscModifiers {
    Rarity(ItemRarity),
    /// This item is a container that can hold another inventory
    Container(InventoryId),
    Damaged(DamageLevels),
    Unidentified,
    Equipped(EquipSlot),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EquipSlot {
    Diary,
    Footwear,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ItemRarity {
    Normal,
    Enchanted,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DamageLevels {
    Perfect,
    Scuffed,
    Worn,
    Tattered,
    Shredded,
    Broken,
}

pub fn goblin_drops() -> Inventory<Item> {
    let gold = Item {
        item_id: ItemId::default(),
        data: ItemData {
            item_base: BaseItem::CurrencyPiece,
            mods: vec![],
            item_misc: vec![],
        },
    };

    let goblin_diary_page = Item {
        item_id: ItemId::default(),
        data: ItemData {
            item_base: BaseItem::EnemyDiaryPage(diary::EnemyDiaryPage::Goblin),
            mods: vec![],
            item_misc: vec![
                ItemMiscModifiers::Damaged(DamageLevels::Worn),
            ],
        },
    };

    let boots = Item {
        item_id: ItemId::default(),
        data: ItemData {
            item_base: BaseItem::Footwear(footwear::Footwear::Sandals),
            mods: vec![],
            item_misc: vec![
                ItemMiscModifiers::Damaged(DamageLevels::Tattered),
                ItemMiscModifiers::Equipped(EquipSlot::Footwear),
            ],
        },
    };

    let ranger_page = Item {
        item_id: ItemId::default(),
        data: ItemData {
            item_base: BaseItem::DiaryPage(diary::DiaryPage::Ranger),
            mods: vec![],
            item_misc: vec![
                ItemMiscModifiers::Equipped(EquipSlot::Diary),
            ],
        },
    };

    Inventory {
        id: InventoryId::default(),
        items: vec![
            ItemInInventory {
                item: gold,
                stacksize: 100,
                item_placement: ItemPlacement {
                    flipped: false,
                    rotated: 0,
                    slot_index: 0,
                },
            },
            ItemInInventory {
                item: goblin_diary_page,
                stacksize: 1,
                item_placement: ItemPlacement {
                    flipped: false,
                    rotated: 0,
                    slot_index: 1,
                },
            },
            ItemInInventory {
                item: boots,
                stacksize: 1,
                item_placement: ItemPlacement {
                    flipped: false,
                    rotated: 0,
                    slot_index: 2,
                },
            },
            ItemInInventory {
                item: ranger_page,
                stacksize: 1,
                item_placement: ItemPlacement {
                    flipped: false,
                    rotated: 0,
                    slot_index: 3,
                },
            },
        ],
    }
}

// example: a goblin is kiled and drops:
//    100 gold pieces
//    a diary page of type "goblin"
//    a pair of boots with no mods and misc Tattered
//
// example
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_item_drop_size() {
        let goblin_drops = goblin_drops();

        let as_payload = postcard::to_stdvec(&goblin_drops).unwrap();
        println!("{}", as_payload.len());
        assert!(as_payload.len() < 120); // ensure it's not too large
    }

    #[test]
    fn test_get_equipped_skills() {
        let goblin_drops = goblin_drops();
        let skills = goblin_drops.get_equipped_skills();
        assert!(skills.len() > 1);
    }
}
