use crate::{event::EventFromEndpoint};
use crate::netlib::ServerResources;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{spells::ShootingData, NetEntId};

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct ConnectRequest {
    pub name: Option<String>,
    pub my_location: Transform,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct SendChat {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct Heartbeat {}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct SpawnCircle {
    pub position: Vec3,
    pub color: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message, Component)]
pub enum Cast {
    Teleport(Vec3),
    Shoot(ShootingData),
    ShootTargeted(Vec3, NetEntId),
    Melee,
    Aoe(Vec3),
    Buff,
}

/// walking and stuff
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub enum ChangeMovement {
    StandStill,
    Move2d(Vec2),
    SetTransform(Transform),
}

include!(concat!(env!("OUT_DIR"), "/server_event.rs"));
