//!This is for events that are sent FROM the client TO the server.
use crate::event::{EventFromEndpoint, NetEntId};
use crate::items::SkillFromSkillSource;
//use crate::net_components::NetComponent;
use crate::netlib::NetworkingResources;
use avian3d::prelude::{LinearVelocity, Rotation};
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
pub struct Heartbeat {
    pub client_started_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct SpawnCircle {
    pub position: Vec3,
    pub color: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct SpawnMan {
    pub position: Vec3,
}

//#[derive(Debug, Clone, Serialize, Deserialize, Message)]
//pub struct RequestSpawnUnit2 //{
//pub components: Vec<NetComponent>,
//}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct ChangeMovement {
    pub net_ent_id: NetEntId,
    pub transform: Transform,
    pub velocity: Option<LinearVelocity>,
    pub rotation: Option<Rotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct RequestScoreboard {}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct HeartbeatChallengeResponse {
    pub server_time: f64,
    pub local_latency_microsecs: f64,
    // TODO: make this only need the server challenge, not time. store time server side so client
    // can't cheat
    //pub server_challenge: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct IWantToDisconnect {}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
pub struct CastSkillUpdate {
    pub net_ent_id: NetEntId,
    /// True if starting to cast, false if stopping
    pub begin_casting: bool,
    pub skill: SkillFromSkillSource,
}

include!(concat!(env!("OUT_DIR"), "/server_event.rs"));
