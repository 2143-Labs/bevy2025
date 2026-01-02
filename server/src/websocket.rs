use bevy::prelude::*;
use std::{collections::HashMap, net::SocketAddr};
use tokio_tungstenite::tungstenite::Bytes;
//use dashmap::DashMap;
use futures_channel::mpsc::{unbounded, UnboundedSender};
use shared::netlib::{ServerNetworkingResources, WebSocketEndpoint};
use std::sync::Arc;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tokio_tungstenite::tungstenite::protocol::Message;

use futures_util::{SinkExt, StreamExt, TryStreamExt};

use crate::{ServerState, TokioRuntimeResource};

pub struct WebsocketPlugin;

impl Plugin for WebsocketPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(ServerState::Running), setup_shared_websocket_server);

        app.insert_resource(WebsocketResource::default());
    }
}

pub struct SingleConnectionPeer {
    pub tx: UnboundedSender<Message>,
}

#[derive(Resource, Clone)]
struct WebsocketResource {
    // must use hashmap because SocketAddr does not implement Hash + Eq
    socket_addr_to_player_id: Arc<Mutex<HashMap<SocketAddr, Arc<SingleConnectionPeer>>>>,
}

impl Default for WebsocketResource {
    fn default() -> Self {
        WebsocketResource {
            socket_addr_to_player_id: Arc::new(Mutex::new(HashMap::new())),
        }
    }
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

    let (mut tx, rx) = unbounded();

    let peer = Arc::new(SingleConnectionPeer { tx: tx.clone() });

    {
        let mut map = ws_resource.socket_addr_to_player_id.lock().await;
        map.insert(addr, peer.clone());
    }

    let res_clone = net_res.clone();
    let mut our_tx = tx.clone();
    tokio::spawn(async move {
        // We need to send all queued events for this user
        let ws_user = WebSocketEndpoint { socket_addr: addr };
        loop {
            net_res
                .event_list_outgoing_websocket
                .retain(|endpoint, msg| {
                    if *endpoint == ws_user {

                        use shared::netlib::EventGroupingOwned;
                        let taken_msgs = msg.drain(..).collect::<Vec<_>>();
                        let new_msg = EventGroupingOwned::Batch(taken_msgs);
                        let bytes = match postcard::to_stdvec(&new_msg) {
                            Ok(b) => b,
                            Err(e) => {
                                warn!(
                                    ?ws_user,
                                    ?e,
                                    "Failed to serialize outgoing websocket message"
                                );
                                return true; // keep in list to try again later
                            }
                        };
                        let send_result = our_tx.start_send(Message::Binary(Bytes::from(bytes)));
                        if let Err(e) = send_result {
                            warn!(?ws_user, ?e, "Failed to send outgoing websocket message");
                            return true; // keep in list to try again later
                        }
                        false // remove from list
                    } else {
                        true // keep in list
                    }
                });

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    });

    let (outgoing, incoming) = ws_stream.split();

    use shared::netlib::EventGroupingOwned;
    let endpoint = WebSocketEndpoint { socket_addr: addr };

    let broadcast_incoming = incoming.try_for_each(|msg| {
        // Handle incoming messages here
        //info!("Received a message from {}: {}", addr, msg);
        //net_res.event_list_incoming_websocket.write().unwrap().push((addr, msg.into_data()));
        let msg = match msg {
            Message::Binary(b) => b,
            Message::Text(t) => Bytes::from(t),
            _ => {
                warn!(?endpoint, "Got non-binary/text message from endpoint");
                return futures_util::future::ok(());
            }
        };

        let event: EventGroupingOwned<shared::netlib::EventToServer> =
            match postcard::from_bytes(&*msg) {
                Ok(e) => e,
                Err(p) => {
                    warn!(?endpoint, ?p, "Got invalid json from endpoint");
                    return futures_util::future::ok(());
                }
            };

        let data_len = msg.len();
        net_res
            .networking_stats
            .total_bytes_received_this_second
            .fetch_add(data_len, std::sync::atomic::Ordering::Relaxed);
        net_res
            .networking_stats
            .packets_received_this_second
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let mut list = net_res.event_list_incoming_websocket.write().unwrap();
        match event {
            EventGroupingOwned::Single(x) => {
                let pair = (endpoint, x);
                list.push(pair);
            }
            EventGroupingOwned::Batch(events) => {
                list.extend(events.into_iter().map(|x| (endpoint, x)));
            }
            EventGroupingOwned::Reliable(packet_id, _dedup_id, tick, events) => {
                let mut seen_map = net_res.reliable_packet_ids_seen.write().unwrap();

                if seen_map.get(&packet_id).is_some() {
                    net_res
                        .networking_stats
                        .total_bytes_received_ignored_this_second
                        .fetch_add(data_len, std::sync::atomic::Ordering::Relaxed);
                } else {
                    seen_map.insert(packet_id, tick); // TODO store tick properly
                    list.extend(events.into_iter().map(|x| (endpoint, x)));
                }
            }
        }
        futures_util::future::ok(())
    });

    let receive_from_others = rx.map(Ok).forward(outgoing);

    //let bytes = string::from("Welcome to the WebSocket server!");
    //tx.start_send(Message::Text(bytes)).unwrap();

    futures_util::pin_mut!(broadcast_incoming, receive_from_others);
    futures_util::future::select(broadcast_incoming, receive_from_others).await;

    info!("WebSocket connection {} closed.", addr);
    {
        let mut map = ws_resource.socket_addr_to_player_id.lock().await;
        map.remove(&addr);
    }
}

fn setup_shared_websocket_server(
    res: Res<ServerNetworkingResources>,
    tokio_runtime: Res<TokioRuntimeResource>,
    ws_resource: Res<WebsocketResource>,
) {
    let (ip, port) = res.con_str.as_ref().clone();
    let port = port + 1;
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
