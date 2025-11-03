//!This is for events that are sent FROM the client TO the server.
use crate::event::{EventFromEndpoint, NetEntId};
//use crate::net_components::NetComponent;
use crate::netlib::NetworkingResources;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct ConnectRequest {
    pub name: Option<String>,
    pub my_location: Transform,
    pub color_hue: f32,
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

//#[derive(Debug, Clone, Serialize, Deserialize, Message)]
//pub struct RequestSpawnUnit2 //{
//pub components: Vec<NetComponent>,
//}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct ChangeMovement {
    pub net_ent_id: NetEntId,
    pub transform: Transform,
}

include!(concat!(env!("OUT_DIR"), "/server_event.rs"));
