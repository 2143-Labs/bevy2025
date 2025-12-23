use bevy::prelude::*;

/// Marker component for the local player entity
#[derive(Component)]
pub struct Player;

/// Represents an item that can be stored in inventory
#[derive(Component, Clone, Debug)]
pub struct Item {
    /// Display name of the item
    pub name: String,
    /// 2D texture for UI rendering (in inventory slots)
    pub texture_2d: Handle<Image>,
    /// Optional 3D model/texture for world rendering
    pub texture_3d: Option<Handle<Image>>,
}

impl Item {
    pub fn new(name: impl Into<String>, texture_2d: Handle<Image>) -> Self {
        Self {
            name: name.into(),
            texture_2d,
            texture_3d: None,
        }
    }

    pub fn with_3d_texture(mut self, texture_3d: Handle<Image>) -> Self {
        self.texture_3d = Some(texture_3d);
        self
    }
}

/// Player inventory component - stores items in a fixed-size grid
#[derive(Component, Clone, Debug)]
pub struct Inventory {
    /// Items stored in the inventory (7 wide x 3 high = 21 slots)
    /// None represents an empty slot
    pub slots: Vec<Option<Item>>,
    /// Width of the inventory grid
    pub width: usize,
    /// Height of the inventory grid
    pub height: usize,
}

impl Inventory {
    /// Create a new inventory with the specified dimensions
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            slots: vec![None; width * height],
            width,
            height,
        }
    }

    /// Get the item at a specific slot index
    pub fn get_slot(&self, index: usize) -> Option<&Item> {
        self.slots.get(index).and_then(|slot| slot.as_ref())
    }

    /// Set the item at a specific slot index
    pub fn set_slot(&mut self, index: usize, item: Option<Item>) {
        if index < self.slots.len() {
            self.slots[index] = item;
        }
    }

    /// Swap items between two slots
    pub fn swap_slots(&mut self, from_index: usize, to_index: usize) {
        if from_index < self.slots.len() && to_index < self.slots.len() {
            self.slots.swap(from_index, to_index);
        }
    }

    /// Add an item to the first available slot
    pub fn add_item(&mut self, item: Item) -> bool {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.is_none()) {
            *slot = Some(item);
            true
        } else {
            false
        }
    }

    /// Remove an item from a specific slot
    pub fn remove_item(&mut self, index: usize) -> Option<Item> {
        if index < self.slots.len() {
            self.slots[index].take()
        } else {
            None
        }
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new(7, 3) // 7 wide x 3 high
    }
}
