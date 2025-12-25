use bevy::prelude::*;
use shared::{
    event::{
        UDPacketEvent,
        client::{NewInventory, UpdateInventory, UpdateItems},
    },
    items::InventoryItemCache,
};

use crate::game_state::NetworkGameState;

pub struct InventoryNetworkPlugin;

impl Plugin for InventoryNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(InventoryItemCache::default())
            .add_systems(
                OnEnter(NetworkGameState::ClientConnected),
                clear_inventory_cache,
            );

        app.add_message::<NewInventory>();
        app.add_message::<UpdateInventory>();
        app.add_message::<UpdateItems>();

        app.add_systems(
            Update,
            (
                new_inventory_cache,
                update_inventory_cache,
                update_item_cache,
            )
                .run_if(in_state(NetworkGameState::ClientConnected)),
        );

        app.add_systems(
            Update,
            (
                update_new_inventory_cache_local,
                update_inventory_cache_local,
                update_item_cache_local,
            ),
        );
    }
}

fn clear_inventory_cache(inventory_item_cache: Res<InventoryItemCache>) {
    warn!("Clearing inventory item cache");
    inventory_item_cache.clear();
}

// Act on incoming event types,

fn new_inventory_cache(
    inventory_item_cache: Res<InventoryItemCache>,
    mut inventory_reader: MessageReader<NewInventory>,
) {
    for event in inventory_reader.read() {
        warn!("Adding new inventory to cache: {:?}", event.inventory);
        inventory_item_cache.insert_inventory(event.inventory.clone());
    }
}

fn update_inventory_cache(
    _inventory_item_cache: Res<InventoryItemCache>,
    mut inventory_reader: MessageReader<UpdateInventory>,
) {
    for _event in inventory_reader.read() {
        error!("TODO! inventory updates not implemented...");
    }
}

fn update_item_cache(
    inventory_item_cache: Res<InventoryItemCache>,
    mut item_reader: MessageReader<UpdateItems>,
) {
    for event in item_reader.read() {
        for item in &event.items {
            warn!("Updating item in cache: {:?}", item);
            inventory_item_cache.insert_item(item.clone());
        }
    }
}

// local event foward

fn update_inventory_cache_local(
    mut inventory_events: UDPacketEvent<UpdateInventory>,
    mut writer: MessageWriter<UpdateInventory>,
) {
    for event in inventory_events.read() {
        writer.write(event.event.clone());
    }
}

fn update_item_cache_local(
    mut inventory_events: UDPacketEvent<UpdateItems>,
    mut writer: MessageWriter<UpdateItems>,
) {
    for event in inventory_events.read() {
        writer.write(event.event.clone());
    }
}

fn update_new_inventory_cache_local(
    mut inventory_events: UDPacketEvent<NewInventory>,
    mut writer: MessageWriter<NewInventory>,
) {
    for event in inventory_events.read() {
        writer.write(event.event.clone());
    }
}
