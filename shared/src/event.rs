use bevy::prelude::*;

use message_io::network::Endpoint;
use serde::{Deserialize, Serialize};

pub mod client;
pub mod server;

#[derive(Debug, Clone, Copy, Component, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NetEntId(pub u64);

#[derive(Debug, Clone, Copy, Component, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MyNetEntParentId(pub u64);

impl NetEntId {
    pub fn random() -> Self {
        Self(rand::random())
    }
}

impl MyNetEntParentId {
    pub fn new(id: NetEntId) -> Self {
        MyNetEntParentId(id.0)
    }

}

#[derive(Debug, Clone, Message)]
pub struct EventFromEndpoint<E> {
    pub event: E,
    pub endpoint: Endpoint,
}

/// Event Reader with endpoint data.
pub type ERFE<'w, 's, E> = MessageReader<'w, 's, EventFromEndpoint<E>>;

impl<E> EventFromEndpoint<E> {
    pub fn new(endpoint: Endpoint, e: E) -> Self {
        EventFromEndpoint { event: e, endpoint }
    }
}
