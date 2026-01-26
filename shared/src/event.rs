use bevy_internal::prelude::*;

use crate::{
    message_io::network::Endpoint,
    netlib::{EndpointGeneral, WebSocketEndpoint},
};
use serde::{Deserialize, Serialize};

pub mod client;
pub mod server;

/// Every spawned entity gets a unique NetEntId.
#[derive(Debug, Clone, Copy, Component, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NetEntId(pub u64);

/// Every unique player gets a unique PlayerId.
#[derive(
    Debug, Clone, Copy, Component, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct PlayerId(pub u64);

#[derive(Debug, Clone, Copy, Component, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MyNetEntParentId(pub u64);

impl NetEntId {
    pub fn random() -> Self {
        // ID < 10 are reserved for special purposes.
        Self(rand::random_range(10..=u64::MAX))
    }

    // Very rarely used: only for special meta entities.
    pub fn none() -> Self {
        Self(0)
    }

    pub fn is_none(&self) -> bool {
        self.0 == 0
    }
}

impl PlayerId {
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
    pub endpoint: EndpointGeneral,
}

/// Event Reader with endpoint data.
pub type UDPacketEvent<'w, 's, E> = MessageReader<'w, 's, EventFromEndpoint<E>>;

impl<E> EventFromEndpoint<E> {
    pub fn new(endpoint: EndpointGeneral, e: E) -> Self {
        EventFromEndpoint { event: e, endpoint }
    }
    pub fn new_udp(endpoint: Endpoint, e: E) -> Self {
        EventFromEndpoint {
            event: e,
            endpoint: EndpointGeneral::UDP(endpoint),
        }
    }
    pub fn new_ws(endpoint: WebSocketEndpoint, e: E) -> Self {
        EventFromEndpoint {
            event: e,
            endpoint: EndpointGeneral::WebSocket(endpoint),
        }
    }
}
