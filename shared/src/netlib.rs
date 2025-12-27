use bevy::prelude::*;
use message_io::{
    network::{Endpoint, NetEvent, Transport},
    node::{NodeEvent, NodeHandler},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::{Arc, Mutex}};

#[derive(Resource, Clone)]
pub struct NetworkingResources<TI, TO> {
    pub event_list_incoming: Arc<Mutex<Vec<(Endpoint, TI)>>>,
    pub event_list_outgoing: Arc<Mutex<Vec<(Endpoint, TO)>>>,
    pub reliable_packet_ids_seen: Arc<Mutex<HashMap<PacketIdentifier, Tick>>>,
    pub handler: NodeHandler<()>,
}

/// Networking resources held by the client
pub type ClientNetworkingResources = NetworkingResources<EventToClient, EventToServer>;
/// Networking resources held by the server
pub type ServerNetworkingResources = NetworkingResources<EventToServer, EventToClient>;

/// Exists only on the client, holds the main server endpoint to which we are connected
#[derive(Resource, Clone)]
pub struct MainServerEndpoint(pub Endpoint);

/// This type is only used for the inital connection, and then it is removed.
#[derive(Resource, Debug)]
pub struct NetworkConnectionTarget {
    pub ip: String,
    pub port: u16,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Tick(pub u64);

impl Tick {
    pub fn increment(&mut self) {
        self.0 += 1;
    }
}

use crate::CurrentTick;
pub use crate::event::client::EventToClient;
pub use crate::event::server::EventToServer;

pub trait NetworkingEvent:
    Clone + Serialize + for<'de> Deserialize<'de> + Send + 'static + core::fmt::Debug
{
}
impl NetworkingEvent for EventToServer {}
impl NetworkingEvent for EventToClient {}

type PacketIdentifier = u32;
type DuplicationIdentifier = u8;

#[derive(Deserialize)]
pub enum EventGroupingOwned<T> {
    Single(T),
    Batch(Vec<T>),
    Reliable(PacketIdentifier, DuplicationIdentifier, Tick, Vec<T>),
}

#[derive(Serialize)]
pub enum EventGroupingRef<'a, T> {
    Single(&'a T),
    Batch(&'a [T]),
    Reliable(PacketIdentifier, DuplicationIdentifier, Tick, &'a [T]),
}

pub fn send_outgoing_event_now<T: NetworkingEvent>(
    handler: &NodeHandler<()>,
    endpoint: Endpoint,
    event: &T,
) {
    trace!(?event, "Sending event");
    handler.network().send(
        endpoint,
        &postcard::to_stdvec(&EventGroupingRef::Single(event)).unwrap(),
    );
}

pub fn send_outgoing_event_now_batch<T: NetworkingEvent>(
    handler: &NodeHandler<()>,
    endpoint: Endpoint,
    event: &[T],
) {
    trace!(?event, "Sending batch event");
    let data = postcard::to_stdvec(&EventGroupingRef::Batch(event)).unwrap();
    if data.len() > 6000 {
        warn!(data_len = data.len(), "Sending large batch event");
    }
    handler.network().send(endpoint, &data);
}

pub fn send_outgoing_event_reliable_internal<T: NetworkingEvent>(
    handler: &NodeHandler<()>,
    endpoint: Endpoint,
    event: &[T],
    tick: &Tick,
) {
    trace!(?event, "Sending doubled batch event");

    let packet_id: PacketIdentifier = rand::random();
    let dedup_id: DuplicationIdentifier = rand::random();
    let data = postcard::to_stdvec(&EventGroupingRef::Reliable(packet_id, dedup_id, *tick, event)).unwrap();
    handler.network().send(endpoint, &data);

    let dedup_id: DuplicationIdentifier = rand::random();
    let data = postcard::to_stdvec(&EventGroupingRef::Reliable(packet_id, dedup_id, *tick, event)).unwrap();
    handler.network().send(endpoint, &data);
}


pub fn send_outgoing_event_next_tick<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    event: &TO,
) {
    let mut list = resources.event_list_outgoing.lock().unwrap();
    list.push((endpoint, event.clone()));
}

pub fn send_outgoing_event_next_tick_batch<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    events: &[TO],
) {
    let mut list = resources.event_list_outgoing.lock().unwrap();
    for event in events {
        list.push((endpoint, event.clone()));
    }
}

pub fn flush_outgoing_events<TI: NetworkingEvent, TO: NetworkingEvent>(
    tick: Res<CurrentTick>,
    resources: Res<NetworkingResources<TI, TO>>,
) {
    let mut list = resources.event_list_outgoing.lock().unwrap();
    // swap it out for a new empty list
    let events_to_send = std::mem::take(&mut *list);
    drop(list); // unlock mutex
    let mut events_per_endpoint: std::collections::HashMap<Endpoint, Vec<TO>> =
        std::collections::HashMap::new();
    //info!(num_events = events_to_send.len(), "Flushing outgoing events");
    for (endpoint, event) in events_to_send {
        events_per_endpoint.entry(endpoint).or_default().push(event);
    }

    for (endpoint, events) in events_per_endpoint {
        for chunk in events.chunks(50) {
            send_outgoing_event_reliable_internal(&resources.handler, endpoint, chunk, &tick.0);
        }
    }
}

pub fn setup_incoming_server<TI: NetworkingEvent, TO: NetworkingEvent>(
    commands: Commands,
    config: Res<NetworkConnectionTarget>,
) {
    setup_incoming_shared::<TI, TO>(commands, &config.ip, config.port, true);
}

pub fn setup_incoming_client<TI: NetworkingEvent, TO: NetworkingEvent>(
    commands: Commands,
    config: Res<NetworkConnectionTarget>,
) {
    setup_incoming_shared::<TI, TO>(commands, &config.ip, config.port, false);
}

pub fn setup_incoming_shared<TI: NetworkingEvent, TO: NetworkingEvent>(
    mut commands: Commands,
    ip: &str,
    port: u16,
    is_listener: bool,
) {
    info!(is_listener, "Seting up networking!");

    let (handler, listener) = message_io::node::split::<()>();

    let res = NetworkingResources::<TI, TO> {
        handler: handler.clone(),
        event_list_incoming: Default::default(),
        event_list_outgoing: Default::default(),
        reliable_packet_ids_seen: Default::default(),
    };

    // insert the new endpoints and remove the connection data
    commands.insert_resource(res.clone());
    commands.remove_resource::<NetworkConnectionTarget>();

    info!(
        "Setup networking resources for {}",
        std::any::type_name::<NetworkingResources::<TI, TO>>()
    );

    let con_str = (ip, port);
    if is_listener {
        let (_, addr) = handler.network().listen(Transport::Udp, con_str).unwrap();
        info!(?addr, "Listening")
    } else {
        let (endpoint, addr) = handler.network().connect(Transport::Udp, con_str).unwrap();
        commands.insert_resource(MainServerEndpoint(endpoint));
        info!(?addr, "Connected");
    }

    std::thread::spawn(move || {
        listener.for_each(|event| on_node_event_incoming(&res, event));
    });
}

pub fn on_node_event_incoming<TI: NetworkingEvent, TO>(
    res: &NetworkingResources<TI, TO>,
    event: NodeEvent<'_, ()>,
) {
    let net_event = match event {
        NodeEvent::Network(n) => n,
        NodeEvent::Signal(_) => {
            error!("MESSAGE SERVER SHUTDOWN SIGNAL RECEIVED!!!");
            panic!("Not implemented");
            // TODO graceful shutdown
        }
    };

    match net_event {
        NetEvent::Connected(endpoint, v) => info!(?endpoint, ?v, "Network Connected"),
        NetEvent::Accepted(endpoint, listener) => {
            info!(?endpoint, ?listener, "Connection Accepted")
        }
        NetEvent::Message(endpoint, data) => {
            let event: EventGroupingOwned<TI> = match postcard::from_bytes(data) {
                Ok(e) => e,
                Err(p) => {
                    warn!(?endpoint, ?p, "Got invalid json from endpoint");
                    return;
                }
            };

            let mut list = res.event_list_incoming.lock().unwrap();
            match event {
                EventGroupingOwned::Single(x) => {
                    let pair = (endpoint, x);
                    list.push(pair);
                }
                EventGroupingOwned::Batch(events) => {
                    list.extend(events.into_iter().map(|x| (endpoint, x)));
                }
                EventGroupingOwned::Reliable(packet_id, _dedup_id, tick, events) => {
                    let mut seen_map = res.reliable_packet_ids_seen.lock().unwrap();

                    // if we have seen this packet id ever, ignore it
                    if let Some(_seen_time) = seen_map.get(&packet_id) {
                        info!(?endpoint, packet_id, "Duplicate reliable packet ignored");
                    } else {
                        seen_map.insert(packet_id, tick); // TODO store tick properly
                        list.extend(events.into_iter().map(|x| (endpoint, x)));
                    }
                }
            }
        }
        NetEvent::Disconnected(endpoint) => warn!(?endpoint, "Client disconnected"),
    }
}
