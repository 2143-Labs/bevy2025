use bevy::prelude::*;
use std::{collections::HashMap, net::SocketAddr};
//use dashmap::DashMap;
use futures_channel::mpsc::{unbounded, UnboundedSender};
use shared::netlib::ServerNetworkingResources;
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
    ws_resource: WebsocketResource,
    raw_stream: TcpStream,
    addr: SocketAddr,
) {
    info!("New WebSocket connection: {}", addr);
    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");

    let (tx, rx) = unbounded();

    let peer = Arc::new(SingleConnectionPeer { tx });

    {
        let mut map = ws_resource.socket_addr_to_player_id.lock().await;
        map.insert(addr, peer.clone());
    }

    let (outgoing, incoming) = ws_stream.split();

    let broadcast_incoming = incoming.try_for_each(|msg| {
        // Handle incoming messages here
        info!("Received a message from {}: {}", addr, msg);
        futures_util::future::ok(())
    });

    let receive_from_others = rx.map(Ok).forward(outgoing);

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

    tokio_runtime.spawn(async move {
        let try_socket = TcpListener::bind((ip, port)).await;
        let listener = try_socket.expect("Failed to bind");

        while let Ok((stream, addr)) = listener.accept().await {
            let ws_resource = ws_resource.clone();
            tokio::spawn(handle_websocket_connection(ws_resource, stream, addr));
        }

        error!("WebSocket server has stopped unexpectedly");
    });
}
