//!This is for events that are sent FROM the client TO the server.
use crate::event::EventFromEndpoint;
use crate::netlib::ServerResources;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::NetEntId;

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

include!(concat!(env!("OUT_DIR"), "/server_event.rs"));
