use bevy::prelude::*;
use std::{collections::HashMap, net::SocketAddr};
use tokio_tungstenite::tungstenite::Bytes;
//use dashmap::DashMap;
use futures_channel::mpsc::{unbounded, UnboundedSender};
use shared::{netlib::{
    EventGroupingRef, ServerNetworkingResources, WebSocketEndpoint, on_data_incoming
}, tokio_udp::TokioRuntimeResource};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::protocol::Message;

use futures_util::{StreamExt, TryStreamExt};

use crate::{ServerState};

pub struct WebsocketPlugin;

impl Plugin for WebsocketPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(ServerState::Running), setup_shared_websocket_server);

        app.add_systems(
            FixedPostUpdate,
            flush_outgoing_events_websocket.run_if(in_state(ServerState::Running)),
        );

        app.insert_resource(WebsocketResource::default());
    }
}

pub struct SingleConnectionPeer {
    pub tx: UnboundedSender<Message>,
}

use std::sync::RwLock;

#[derive(Resource, Clone, Default)]
struct WebsocketResource {
    // must use hashmap because SocketAddr does not implement Hash + Eq
    //socket_addr_to_tx_queue: Arc<DashMap<SocketAddr, Arc<SingleConnectionPeer>>>,
    socket_addr_to_tx_queue: Arc<RwLock<HashMap<SocketAddr, Arc<SingleConnectionPeer>>>>,
}

fn flush_outgoing_events_websocket(
    resources: Res<ServerNetworkingResources>,
    ws_resource: Res<WebsocketResource>,
) {
    resources
        .event_list_outgoing_websocket
        .retain(|key, value| {
            let maybe_tx_queue = ws_resource
                .socket_addr_to_tx_queue
                .read()
                .unwrap()
                .get(&key.socket_addr)
                .map(|spc| spc.tx.clone());

            let Some(mut peer_queue) = maybe_tx_queue else {
                // this player has dropped the websocket connection but we still have them as a
                // "Connected Player". They can re-establish a new websocket connection.
                debug!(
                    "No websocket tx queue found for socket addr: {}",
                    key.socket_addr
                );
                return false;
            };

            let new_msg = EventGroupingRef::Batch(value);
            let bytes = match postcard::to_stdvec(&new_msg) {
                Ok(b) => b,
                Err(e) => {
                    warn!(?e, "Failed to serialize outgoing websocket message");
                    return false;
                }
            };
            let send_result = peer_queue.start_send(Message::Binary(Bytes::from(bytes)));
            if let Err(e) = send_result {
                warn!(?e, "Failed to send outgoing websocket message");
            }
            false
        })
}

async fn handle_websocket_connection(
    net_res: ServerNetworkingResources,
    ws_resource: WebsocketResource,
    raw_stream: TcpStream,
    addr: SocketAddr,
) {
    info!("New WebSocket connection: {}", addr);
    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");

    let (tx, rx) = unbounded();

    let peer = Arc::new(SingleConnectionPeer { tx: tx.clone() });
    drop(tx);

    {
        let mut map = ws_resource.socket_addr_to_tx_queue.write().unwrap();
        map.insert(addr, peer.clone());
    }

    let (outgoing, incoming) = ws_stream.split();

    let endpoint = WebSocketEndpoint { socket_addr: addr };

    let broadcast_incoming = incoming.try_for_each(|msg| {
        let msg = match msg {
            Message::Binary(b) => b,
            Message::Text(t) => Bytes::from(t),
            _ => {
                warn!(?endpoint, "Got non-binary/text message from endpoint");
                return futures_util::future::ok(());
            }
        };

        let data_buffer = net_res.event_list_incoming_websocket.clone();
        on_data_incoming(&net_res, endpoint, data_buffer, &msg);

        futures_util::future::ok(())
    });

    // Putting events into the WebsocketResource's units tx queue will send them out on the network
    // instantly- see `flush_outgoing_events_websocket` for events being places in this queue
    let receive_from_others = rx.map(Ok).forward(outgoing);

    futures_util::pin_mut!(broadcast_incoming, receive_from_others);
    futures_util::future::select(broadcast_incoming, receive_from_others).await;

    info!("WebSocket connection {} closed.", addr);
    {
        let mut map = ws_resource.socket_addr_to_tx_queue.write().unwrap();
        map.remove(&addr);
    }
}

fn setup_shared_websocket_server(
    res: Res<ServerNetworkingResources>,
    tokio_runtime: Res<TokioRuntimeResource>,
    ws_resource: Res<WebsocketResource>,
) {
    let (ip, port) = res.con_str.as_ref().clone();
    info!("Starting shared websocket server on {}:{}", ip, port);

    let ws_resource = (*ws_resource).clone();
    let net_res = res.clone();

    tokio_runtime.spawn(async move {
        let try_socket = TcpListener::bind((ip, port)).await;
        let listener = try_socket.expect("Failed to bind");

        while let Ok((stream, addr)) = listener.accept().await {
            let ws_resource = ws_resource.clone();
            tokio::spawn(handle_websocket_connection(
                net_res.clone(),
                ws_resource,
                stream,
                addr,
            ));
        }

        error!("WebSocket server has stopped unexpectedly");
    });
}
