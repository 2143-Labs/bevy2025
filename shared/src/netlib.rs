use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    collections::VecDeque,
    sync::{atomic::AtomicUsize, Arc, RwLock},
};

use crate::message_io::{
    network::{Endpoint, NetEvent, Transport},
    node::{NodeEvent, NodeHandler},
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
    pub event_list_incoming_udp: Arc<RwLock<Vec<(Endpoint, TI)>>>,
    pub event_list_incoming_websocket: Arc<RwLock<Vec<(WebSocketEndpoint, TI)>>>,
    // TODO make this hashmap instead
    pub event_list_outgoing_udp: Arc<DashMap<Endpoint, Vec<TO>>>,
    pub event_list_outgoing_websocket: Arc<DashMap<WebSocketEndpoint, Vec<TO>>>,
    pub reliable_packet_ids_seen: Arc<RwLock<HashMap<PacketIdentifier, Tick>>>,
    pub networking_stats: Arc<NetworkingStats>,
    pub handler: Option<NodeHandler<()>>,
    pub con_str: Arc<(String, u16)>,
}

/// Networking resources held by the client
pub type ClientNetworkingResources = NetworkingResources<EventToClient, EventToServer>;
/// Networking resources held by the server
pub type ServerNetworkingResources = NetworkingResources<EventToServer, EventToClient>;

/// Exists only on the client, holds the main server endpoint to which we are connected
#[derive(Resource, Clone)]
pub struct MainServerEndpoint(pub EndpointGeneral);

impl MainServerEndpoint {
    pub fn as_websocket(&self) -> Option<WebSocketEndpoint> {
        match &self.0 {
            EndpointGeneral::WebSocket(ws_endpoint) => Some(*ws_endpoint),
            EndpointGeneral::UDP(_) => None,
        }
    }
}

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

#[derive(Deserialize, Serialize)]
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

impl<TI, TO: NetworkingEvent> NetworkingResources<TI, TO> {
    pub fn send_outgoing_event_now(&self, endpoint: EndpointGeneral, event: &TO) {
        match endpoint {
            EndpointGeneral::WebSocket(ws_endpoint) => {
                // TODO make this faster- but queue for now
                self.event_list_outgoing_websocket
                    .entry(ws_endpoint)
                    .or_default()
                    .push(event.clone());
            }
            EndpointGeneral::UDP(udp_endpoint) => {
                send_outgoing_event_now_udp(self, udp_endpoint, event)
            }
        }
    }

    pub fn send_outgoing_event_now_batch(&self, endpoint: EndpointGeneral, events: &[TO]) {
        match endpoint {
            EndpointGeneral::WebSocket(ws_endpoint) => {
                // TODO make this faster- but queue for now
                self.event_list_outgoing_websocket
                    .entry(ws_endpoint)
                    .or_default()
                    .extend_from_slice(events);
            }
            EndpointGeneral::UDP(udp_endpoint) => {
                send_outgoing_event_now_batch_udp(self, udp_endpoint, events)
            }
        }
    }

    pub fn send_outgoing_event_next_tick(&self, endpoint: EndpointGeneral, event: &TO) {
        match endpoint {
            EndpointGeneral::WebSocket(ws_endpoint) => {
                self.event_list_outgoing_websocket
                    .entry(ws_endpoint)
                    .or_default()
                    .push(event.clone());
            }
            EndpointGeneral::UDP(udp_endpoint) => {
                send_outgoing_event_next_tick_udp(self, udp_endpoint, event)
            }
        }
    }
    pub fn send_outgoing_event_next_tick_batch(&self, endpoint: EndpointGeneral, events: &[TO]) {
        match endpoint {
            EndpointGeneral::WebSocket(ws_endpoint) => {
                self.event_list_outgoing_websocket
                    .entry(ws_endpoint)
                    .or_default()
                    .extend_from_slice(events);
            }
            EndpointGeneral::UDP(udp_endpoint) => {
                send_outgoing_event_next_tick_batch_udp(self, udp_endpoint, events)
            }
        }
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash)]
pub struct WebSocketEndpoint {
    pub socket_addr: std::net::SocketAddr,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash)]
pub enum EndpointGeneral {
    WebSocket(WebSocketEndpoint),
    UDP(Endpoint),
}

fn send_outgoing_event_now_udp<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    event: &TO,
) {
    trace!(?event, "Sending event");
    let event = postcard::to_stdvec(&EventGroupingRef::Single(event)).unwrap();
    resources
        .handler
        .as_ref()
        .expect("must have udp handler")
        .network()
        .send(endpoint, &event);

    resources
        .networking_stats
        .total_bytes_sent_this_second
        .fetch_add(event.len(), std::sync::atomic::Ordering::Relaxed);
    resources
        .networking_stats
        .packets_sent_this_second
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

fn send_outgoing_event_now_batch_udp<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    event: &[TO],
) {
    trace!(?event, "Sending batch event");
    let data = postcard::to_stdvec(&EventGroupingRef::Batch(event)).unwrap();
    if data.len() > 6000 {
        warn!(data_len = data.len(), "Sending large batch event");
    }
    resources
        .handler
        .as_ref()
        .expect("must have udp handler")
        .network()
        .send(endpoint, &data);

    resources
        .networking_stats
        .total_bytes_sent_this_second
        .fetch_add(data.len(), std::sync::atomic::Ordering::Relaxed);

    resources
        .networking_stats
        .packets_sent_this_second
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

fn send_outgoing_event_reliable_internal_chunk_udp<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    event: &[TO],
    tick: &Tick,
) {
    trace!(?event, "Sending doubled batch event");
    let handler = resources.handler.as_ref().expect("must have udp handler");

    let packet_id: PacketIdentifier = rand::random();
    let dedup_id: DuplicationIdentifier = rand::random();
    let data = postcard::to_stdvec(&EventGroupingRef::Reliable(
        packet_id, dedup_id, *tick, event,
    ))
    .unwrap();
    let data_len = data.len() * 2;
    handler.network().send(endpoint, &data);

    let dedup_id: DuplicationIdentifier = rand::random();
    let data = postcard::to_stdvec(&EventGroupingRef::Reliable(
        packet_id, dedup_id, *tick, event,
    ))
    .unwrap();
    handler.network().send(endpoint, &data);

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

fn send_outgoing_event_reliable_internal_udp<TI, TO: NetworkingEvent>(
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
        send_outgoing_event_reliable_internal_chunk_udp(
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

fn send_outgoing_event_next_tick_udp<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    event: &TO,
) {
    resources
        .event_list_outgoing_udp
        .entry(endpoint)
        .or_default()
        .push(event.clone());
}

fn send_outgoing_event_next_tick_batch_udp<TI, TO: NetworkingEvent>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: Endpoint,
    events: &[TO],
) {
    resources
        .event_list_outgoing_udp
        .entry(endpoint)
        .or_default()
        .extend_from_slice(events);
}

pub fn flush_outgoing_events_udp<TI: NetworkingEvent, TO: NetworkingEvent>(
    tick: Res<CurrentTick>,
    resources: Res<NetworkingResources<TI, TO>>,
) {
    use rayon::prelude::*;
    resources
        .event_list_outgoing_udp
        .par_iter_mut()
        .for_each(|mut entry| {
            send_outgoing_event_reliable_internal_udp(
                &resources,
                *entry.key(),
                entry.value(),
                &tick.0,
            );
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

fn setup_incoming_shared<TI: NetworkingEvent, TO: NetworkingEvent>(
    mut commands: Commands,
    mut ip: &str,
    port: u16,
    is_listener: bool,
) {
    info!(is_listener, "Seting up networking!");

    let (handler, listener) = crate::message_io::node::split::<()>();

    info!(
        "Setup networking resources for {}",
        std::any::type_name::<NetworkingResources::<TI, TO>>()
    );

    if ip == "localhost" {
        ip = "[::]";
    }
    let con_str = (ip.to_string(), port);
    if is_listener {
        let (_, udp_addr) = handler
            .network()
            .listen(Transport::Udp, con_str.clone())
            .unwrap();

        info!(?udp_addr, "Listening")
    } else {
        let (_endpoint, addr) = handler
            .network()
            .connect(Transport::Udp, con_str.clone())
            .unwrap();

        #[cfg(not(feature = "web"))]
        commands.insert_resource(MainServerEndpoint(EndpointGeneral::UDP(_endpoint)));

        info!(?addr, "Connected");
    }

    let res = NetworkingResources::<TI, TO> {
        handler: Some(handler.clone()),
        event_list_incoming_udp: Default::default(),
        event_list_outgoing_udp: Default::default(),
        reliable_packet_ids_seen: Default::default(),
        networking_stats: Arc::new(NetworkingStats::default()),
        event_list_incoming_websocket: Default::default(),
        event_list_outgoing_websocket: Default::default(),
        con_str: Arc::new(con_str),
    };

    // insert the new endpoints and remove the connection data
    commands.insert_resource(res.clone());
    commands.remove_resource::<NetworkConnectionTarget>();

    // web doesn't support threads
    // server must support both ws and udp listeners
    #[cfg(feature = "udp")]
    {
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
            on_data_incoming(res, endpoint, res.event_list_incoming_udp.clone(), data);
        }
        NetEvent::Disconnected(endpoint) => warn!(?endpoint, "Client disconnected"),
    }
}

pub trait EndpointTrait: std::fmt::Debug + Copy + Clone + Send + Sync + 'static {
    fn as_socket_addr(&self) -> std::net::SocketAddr;
}

impl EndpointTrait for WebSocketEndpoint {
    fn as_socket_addr(&self) -> std::net::SocketAddr {
        self.socket_addr
    }
}

impl EndpointTrait for Endpoint {
    fn as_socket_addr(&self) -> std::net::SocketAddr {
        self.addr()
    }
}

pub fn on_data_incoming<TI: NetworkingEvent, TO, K: EndpointTrait>(
    resources: &NetworkingResources<TI, TO>,
    endpoint: K,
    data_buffer: Arc<RwLock<Vec<(K, TI)>>>,
    data: &[u8],
) {
    let event: EventGroupingOwned<TI> = match postcard::from_bytes(data) {
        Ok(e) => e,
        Err(p) => {
            warn!(?endpoint, ?p, "Got invalid json from endpoint");
            return;
        }
    };
    let data_len = data.len();
    resources
        .networking_stats
        .total_bytes_received_this_second
        .fetch_add(data_len, std::sync::atomic::Ordering::Relaxed);
    resources
        .networking_stats
        .packets_received_this_second
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let mut list = data_buffer.write().unwrap();
    match event {
        EventGroupingOwned::Single(x) => {
            let pair = (endpoint, x);
            list.push(pair);
        }
        EventGroupingOwned::Batch(events) => {
            list.extend(events.into_iter().map(|x| (endpoint, x)));
        }
        EventGroupingOwned::Reliable(packet_id, _dedup_id, tick, events) => {
            let mut seen_map = resources.reliable_packet_ids_seen.write().unwrap();

            if seen_map.get(&packet_id).is_some() {
                resources
                    .networking_stats
                    .total_bytes_received_ignored_this_second
                    .fetch_add(data_len, std::sync::atomic::Ordering::Relaxed);
            } else {
                seen_map.insert(packet_id, tick); // TODO store tick properly
                list.extend(events.into_iter().map(|x| (endpoint, x)));
            }
        }
    }
}
