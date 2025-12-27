//!This is for events that are sent FROM the server TO the client.
use crate::event::PlayerId;
use crate::items::{Inventory, Item, ItemId, ItemInInventory, ItemPlacement};
use crate::net_components::PlayerConnectionInfo;
use crate::netlib::{NetworkingResources, Tick};
use crate::physics::terrain::TerrainParams;
use crate::ServerTPS;
use crate::{event::EventFromEndpoint, net_components::NetComponent};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::NetEntId;

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct SpawnUnit2 {
    pub net_ent_id: NetEntId,
    pub components: Vec<NetComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct WorldData2 {
    pub your_player_id: PlayerId,
    pub your_camera_unit_id: NetEntId,
    pub terrain_params: TerrainParams,
    pub units: Vec<SpawnUnit2>,
}

// TODO add codegen logic systems for updating each component
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct UpdateUnit2 {
    pub net_ent_id: NetEntId,
    pub changed_components: Vec<NetComponent>,
    pub new_component: Vec<NetComponent>,
    pub removed_components: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct DespawnUnit2 {
    pub net_ent_id: NetEntId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct PlayerDisconnected {
    pub id: PlayerId,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct Chat {
    pub source: Option<NetEntId>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct BeginThirdpersonControllingUnit {
    pub player_id: PlayerId,
    pub unit: Option<NetEntId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct NewInventory {
    pub inventory: Inventory<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct UpdateInventory {
    pub inventory: Inventory<ItemId>,
    pub new_items: Vec<ItemInInventory<Item>>,
    pub removed_items: Vec<ItemId>,
    pub moved_items: Vec<(ItemId, ItemPlacement, ItemPlacement)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct UpdateItems {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct ServerSoreboardInfo {
    pub tps: ServerTPS,
    pub disconnected_players: Vec<PlayerId>,
    pub connected_players: Vec<PlayerId>,
    pub players_connection_info: Vec<(PlayerId, PlayerConnectionInfo)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct HeartbeatResponse {
    pub client_started_time: f64,
    pub server_time: f64,
    pub server_tick: Tick,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct HeartbeatChallenge {
    pub server_time: f64,
    //pub server_challenge: u64,
}

include!(concat!(env!("OUT_DIR"), "/client_event.rs"));
