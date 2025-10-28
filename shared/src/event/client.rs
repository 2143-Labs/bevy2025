//!This is for events that are sent FROM the server TO the client.
use crate::netlib::NetworkingResources;
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
    pub your_unit_id: NetEntId,
    pub your_camera_unit_id: NetEntId,
    pub units: Vec<SpawnUnit2>,
}

// TODO add codegen logic systems for updating each component
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct UpdateUnit2 {
    pub net_ent_id: NetEntId,
    pub components: Vec<NetComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct DespawnUnit2 {
    pub net_ent_id: NetEntId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct PlayerDisconnected {
    pub id: NetEntId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct Chat {
    pub source: Option<NetEntId>,
    pub text: String,
}

include!(concat!(env!("OUT_DIR"), "/client_event.rs"));
