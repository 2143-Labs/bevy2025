use std::time::Duration;

use crate::{event::EventFromEndpoint, net_components::NetComponent};
use crate::netlib::ServerResources;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    server::{Cast, ChangeMovement},
    spells::UpdateSharedComponent,
    NetEntId, UnitData,
};

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct WorldData {
    pub your_unit_id: NetEntId,
    pub unit_data: Vec<UnitData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct SpawnUnit {
    pub data: UnitData,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct SpawnUnit2 {
    pub net_ent_id: NetEntId,
    pub components: Vec<NetComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct WorldData2 {
    pub your_unit_id: NetEntId,
    pub units: Vec<SpawnUnit2>,
}

// TODO add codegen logic systems for updating each component
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct UpdateUnit2 {
    pub net_ent_id: NetEntId,
    pub components: Vec<NetComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct PlayerDisconnected {
    pub id: NetEntId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct SomeoneMoved {
    pub id: NetEntId,
    pub movement: ChangeMovement,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct SomeoneCast {
    pub caster_id: NetEntId,
    pub cast_id: NetEntId,
    pub cast: Cast,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub enum YourCastResult {
    /// Go ahead with cast
    Ok(NetEntId),
    /// Go ahread with cast, but you had some extra cd to account for
    OffsetBy(Duration, NetEntId),
    /// You can't cast.
    No(Duration),
}

#[derive(Debug, Clone, Serialize, Deserialize, Message, Hash, PartialEq, Eq)]
pub struct BulletHit {
    pub bullet: NetEntId,
    pub player: NetEntId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct SomeoneUpdateComponent {
    pub id: NetEntId,
    pub update: UpdateSharedComponent,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct Chat {
    pub source: Option<NetEntId>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct UnitDie {
    pub id: NetEntId,
    pub disappear: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct SpawnInteractable {
    pub id: NetEntId,
    pub location: Vec3,
    // TODO
    //pub interaction_type: T
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct DespawnInteractable {
    pub id: NetEntId,
}

include!(concat!(env!("OUT_DIR"), "/client_event.rs"));
