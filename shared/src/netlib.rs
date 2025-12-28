use bevy::prelude::*;
use message_io::{
    network::{Endpoint, NetEvent, Transport},
    node::{NodeEvent, NodeHandler},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    collections::VecDeque,
    sync::{atomic::AtomicUsize, Arc, RwLock},
};

pub struct NetworkingStats {
    pub total_bytes_sent_this_second: AtomicUsize,
    pub total_bytes_received_this_second: AtomicUsize,
    pub total_bytes_received_ignored_this_second: AtomicUsize,
    pub recent_bytes_sent: RwLock<VecDeque<usize>>,
    pub recent_bytes_received: RwLock<VecDeque<usize>>,
    pub recent_bytes_received_ignored: RwLock<VecDeque<usize>>,

    pub packets_sent_this_second: AtomicUsize,
    pub packets_received_this_second: AtomicUsize,
    pub recent_packets_sent: RwLock<VecDeque<usize>>,
    pub recent_packets_received: RwLock<VecDeque<usize>>,
}

impl Default for NetworkingStats {
    fn default() -> Self {
        Self {
            total_bytes_sent_this_second: AtomicUsize::new(0),
            total_bytes_received_this_second: AtomicUsize::new(0),
            total_bytes_received_ignored_this_second: AtomicUsize::new(0),
            recent_bytes_sent: RwLock::new(VecDeque::new()),
            recent_bytes_received: RwLock::new(VecDeque::new()),
            recent_bytes_received_ignored: RwLock::new(VecDeque::new()),
            packets_sent_this_second: AtomicUsize::new(0),
            packets_received_this_second: AtomicUsize::new(0),
            recent_packets_sent: RwLock::new(VecDeque::new()),
            recent_packets_received: RwLock::new(VecDeque::new()),
        }
    }
}

impl NetworkingStats {
    pub fn flush_and_reset(&self) {
        let total_bytes_sent_this_second =
            self.total_bytes_sent_this_second.swap(0, std::sync::atomic::Ordering::Relaxed);
        let total_bytes_received_this_second = self
            .total_bytes_received_this_second
            .swap(0, std::sync::atomic::Ordering::Relaxed);
        let total_bytes_received_ignored_this_second = self
            .total_bytes_received_ignored_this_second
            .swap(0, std::sync::atomic::Ordering::Relaxed);

        self.recent_bytes_sent
            .write()
            .unwrap()
            .push_back(total_bytes_sent_this_second);

        self.recent_bytes_received
            .write()
            .unwrap()
            .push_back(total_bytes_received_this_second);

        self.recent_bytes_received_ignored
            .write()
            .unwrap()
            .push_back(total_bytes_received_ignored_this_second);

        let packets_sent_this_second =
            self.packets_sent_this_second
                .swap(0, std::sync::atomic::Ordering::Relaxed);

        let packets_received_this_second = self
            .packets_received_this_second
            .swap(0, std::sync::atomic::Ordering::Relaxed);
        self.recent_packets_sent
            .write()
            .unwrap()
            .push_back(packets_sent_this_second);
        self.recent_packets_received
            .write()
            .unwrap()
            .push_back(packets_received_this_second);

        self.cap_queues(BASE_TICKS_PER_SECOND as usize * 60);
    }

    fn cap_queues(&self, max_len: usize) {
        let mut recent_bytes_sent = self.recent_bytes_sent.write().unwrap();
        while recent_bytes_sent.len() > max_len {
            recent_bytes_sent.pop_front();
        }
        drop(recent_bytes_sent);

        let mut recent_bytes_received = self.recent_bytes_received.write().unwrap();
        while recent_bytes_received.len() > max_len {
            recent_bytes_received.pop_front();
        }
        drop(recent_bytes_received);

        let mut recent_bytes_received_ignored = self.recent_bytes_received_ignored.write().unwrap();
        while recent_bytes_received_ignored.len() > max_len {
            recent_bytes_received_ignored.pop_front();
        }
        drop(recent_bytes_received_ignored);

        let mut recent_packets_sent = self.recent_packets_sent.write().unwrap();
        while recent_packets_sent.len() > max_len {
            recent_packets_sent.pop_front();
        }
        drop(recent_packets_sent);

        let mut recent_packets_received = self.recent_packets_received.write().unwrap();
        while recent_packets_received.len() > max_len {
            recent_packets_received.pop_front();
        }
        drop(recent_packets_received);
    }
}

#[derive(Resource, Clone)]
pub struct NetworkingResources<TI, TO> {
    pub event_list_incoming: Arc<RwLock<Vec<(Endpoint, TI)>>>,
    // TODO make this hashmap instead
    pub event_list_outgoing: Arc<RwLock<Vec<(Endpoint, TO)>>>,
    pub reliable_packet_ids_seen: Arc<RwLock<HashMap<PacketIdentifier, Tick>>>,
    pub networking_stats: Arc<NetworkingStats>,
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

pub use crate::event::client::EventToClient;
pub use crate::event::server::EventToServer;
use crate::{BASE_TICKS_PER_SECOND, CurrentTick};

pub trait NetworkingEvent:
    Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static + core::fmt::Debug
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

pub fn send_outgoing_event_now<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    event: &TO,
) {
    trace!(?event, "Sending event");
    let event = postcard::to_stdvec(&EventGroupingRef::Single(event)).unwrap();
    resources.handler.network().send(endpoint, &event);

    resources
        .networking_stats
        .total_bytes_sent_this_second
        .fetch_add(event.len(), std::sync::atomic::Ordering::Relaxed);
    resources
        .networking_stats
        .packets_sent_this_second
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub fn send_outgoing_event_now_batch<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    event: &[TO],
) {
    trace!(?event, "Sending batch event");
    let data = postcard::to_stdvec(&EventGroupingRef::Batch(event)).unwrap();
    if data.len() > 6000 {
        warn!(data_len = data.len(), "Sending large batch event");
    }
    resources.handler.network().send(endpoint, &data);

    resources
        .networking_stats
        .total_bytes_sent_this_second
        .fetch_add(data.len(), std::sync::atomic::Ordering::Relaxed);

    resources
        .networking_stats
        .packets_sent_this_second
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub fn send_outgoing_event_reliable_internal<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    event: &[TO],
    tick: &Tick,
) {
    trace!(?event, "Sending doubled batch event");

    let packet_id: PacketIdentifier = rand::random();
    let dedup_id: DuplicationIdentifier = rand::random();
    let data = postcard::to_stdvec(&EventGroupingRef::Reliable(
        packet_id, dedup_id, *tick, event,
    ))
    .unwrap();
    let data_len = data.len() * 2;
    resources.handler.network().send(endpoint, &data);

    let dedup_id: DuplicationIdentifier = rand::random();
    let data = postcard::to_stdvec(&EventGroupingRef::Reliable(
        packet_id, dedup_id, *tick, event,
    ))
    .unwrap();
    resources.handler.network().send(endpoint, &data);

    resources
        .networking_stats
        .total_bytes_sent_this_second
        .fetch_add(data_len, std::sync::atomic::Ordering::Relaxed);

    resources
        .networking_stats
        .packets_sent_this_second
        .fetch_add(2, std::sync::atomic::Ordering::Relaxed);
}

pub fn send_outgoing_event_next_tick<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    event: &TO,
) {
    let mut list = resources.event_list_outgoing.write().unwrap();
    list.push((endpoint, event.clone()));
}

pub fn send_outgoing_event_next_tick_batch<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    events: &[TO],
) {
    let mut list = resources.event_list_outgoing.write().unwrap();
    for event in events {
        list.push((endpoint, event.clone()));
    }
}

pub fn flush_outgoing_events<TI: NetworkingEvent, TO: NetworkingEvent>(
    tick: Res<CurrentTick>,
    resources: Res<NetworkingResources<TI, TO>>,
) {
    let mut list = resources.event_list_outgoing.write().unwrap();
    // swap it out for a new empty list
    let events_to_send = std::mem::take(&mut *list);
    drop(list); // unlock RwLock
    let mut events_per_endpoint: std::collections::HashMap<Endpoint, Vec<TO>> =
        std::collections::HashMap::new();
    //info!(num_events = events_to_send.len(), "Flushing outgoing events");
    for (endpoint, event) in events_to_send {
        events_per_endpoint.entry(endpoint).or_default().push(event);
    }

    for (endpoint, events) in events_per_endpoint {
        for chunk in events.chunks(50) {
            send_outgoing_event_reliable_internal(&resources, endpoint, chunk, &tick.0);
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
        networking_stats: Arc::new(NetworkingStats::default()),
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

    let res2 = res.clone();
    std::thread::spawn(move || {
        listener.for_each(|event| on_node_event_incoming(&res2, event));
    });

    let res2 = res.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            res2.networking_stats.flush_and_reset();
            info!("Net: Sent {}kb {}pack\nRecv {}kb {}pack ({}kb ignored)",
                res2.networking_stats.recent_bytes_sent.read().unwrap().back().unwrap_or(&0) / 1024,
                res2.networking_stats.recent_packets_sent.read().unwrap().back().unwrap_or(&0),
                res2.networking_stats.recent_bytes_received.read().unwrap().back().unwrap_or(&0) / 1024,
                res2.networking_stats.recent_packets_received.read().unwrap().back().unwrap_or(&0),
                res2.networking_stats.recent_bytes_received_ignored.read().unwrap().back().unwrap_or(&0) / 1024,
            );
        }
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
            let data_len = data.len();
            res.networking_stats
                .total_bytes_received_this_second
                .fetch_add(data_len, std::sync::atomic::Ordering::Relaxed);
            res.networking_stats
                .packets_received_this_second
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            let mut list = res.event_list_incoming.write().unwrap();
            match event {
                EventGroupingOwned::Single(x) => {
                    let pair = (endpoint, x);
                    list.push(pair);
                }
                EventGroupingOwned::Batch(events) => {
                    list.extend(events.into_iter().map(|x| (endpoint, x)));
                }
                EventGroupingOwned::Reliable(packet_id, _dedup_id, tick, events) => {
                    let mut seen_map = res.reliable_packet_ids_seen.write().unwrap();

                    if seen_map.get(&packet_id).is_some() {
                        res.networking_stats
                            .total_bytes_received_ignored_this_second
                            .fetch_add(data_len, std::sync::atomic::Ordering::Relaxed);
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
