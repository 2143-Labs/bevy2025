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
        let total_bytes_sent_this_second = self
            .total_bytes_sent_this_second
            .swap(0, std::sync::atomic::Ordering::Relaxed);
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

        let packets_sent_this_second = self
            .packets_sent_this_second
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

use dashmap::DashMap;

#[derive(Resource, Clone)]
pub struct NetworkingResources<TI, TO> {
    pub event_list_incoming: Arc<RwLock<Vec<(Endpoint, TI)>>>,
    // TODO make this hashmap instead
    pub event_list_outgoing: Arc<DashMap<Endpoint, Vec<TO>>>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Tick(pub u64);

impl Tick {
    pub fn increment(&mut self) {
        self.0 += 1;
    }
}

impl std::ops::Sub for Tick {
    type Output = Tick;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::Add for Tick {
    type Output = Tick;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

pub use crate::event::client::EventToClient;
pub use crate::event::server::EventToServer;
use crate::{CurrentTick, BASE_TICKS_PER_SECOND};

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

fn send_outgoing_event_reliable_internal_chunk<TI, TO: NetworkingEvent>(
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

    if data_len > 3000 {
        warn!(data_len = data_len, "Sending large reliable batch event");
    }
}

pub fn send_outgoing_event_reliable_internal<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    mut event: &[TO],
    tick: &Tick,
) {
    const TARGET_PACKET_SIZE: usize = 1450;
    while !event.is_empty() {
        let mut chunk_size = 1;
        let packet_id: PacketIdentifier = rand::random();
        let dedup_id: DuplicationIdentifier = rand::random();
        // We construct chunks until we reach the larest that fits in one chunk
        // TODO improve this a lot
        'send: loop {
            if chunk_size >= event.len() {
                break 'send;
            }

            chunk_size += 1;
            let data = postcard::to_stdvec(&EventGroupingRef::Reliable(
                packet_id,
                dedup_id,
                *tick,
                &event[..chunk_size],
            ))
            .unwrap();
            let data_len = data.len();

            if data_len > TARGET_PACKET_SIZE {
                chunk_size -= 1;
                break 'send;
            }
        }
        send_outgoing_event_reliable_internal_chunk(
            resources,
            endpoint,
            &event[..chunk_size],
            tick,
        );
        if chunk_size >= event.len() {
            break;
        }
        event = &event[chunk_size..];
    }
}

pub fn send_outgoing_event_next_tick<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    event: &TO,
) {
    resources
        .event_list_outgoing
        .entry(endpoint)
        .or_insert_with(Vec::new)
        .push(event.clone());
}

pub fn send_outgoing_event_next_tick_batch<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    events: &[TO],
) {
    resources
        .event_list_outgoing
        .entry(endpoint)
        .or_insert_with(Vec::new)
        .extend_from_slice(events);
}

pub fn flush_outgoing_events<TI: NetworkingEvent, TO: NetworkingEvent>(
    tick: Res<CurrentTick>,
    resources: Res<NetworkingResources<TI, TO>>,
) {
    use rayon::prelude::*;
    resources
        .event_list_outgoing
        .par_iter_mut()
        .for_each(|mut entry| {
            send_outgoing_event_reliable_internal(&resources, *entry.key(), entry.value(), &tick.0);
            entry.value_mut().clear();
        })
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
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        res2.networking_stats.flush_and_reset();
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
