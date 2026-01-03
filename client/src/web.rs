use bevy::prelude::*;
use std::sync::Arc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::js_sys::Reflect;

use shared::netlib::ClientNetworkingResources;
use shared::netlib::EndpointGeneral;
use shared::netlib::MainServerEndpoint;
use shared::netlib::WebSocketEndpoint;

//use raw_window_handle::HasRawWindowHandle;

pub struct WebPlugin;

use crate::game_state::NetworkGameState;

impl Plugin for WebPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(NetworkGameState::ClientSendRequestPacket), setup);
        //while in ClientConencting, run
        app.add_systems(
            Update,
            while_connecting.run_if(in_state(NetworkGameState::ClientConnecting)),
        );

        app.add_systems(
            Update,
            send_messages_over_websocket.run_if(resource_exists::<WebSocketResource>),
        );
    }
}

fn while_connecting(
    mut _commands: Commands,
    mut _next_game_state: ResMut<NextState<NetworkGameState>>,
) {
    //next_game_state.set(NetworkGameState::ClientConnected);
}

#[derive(Clone)]
struct MagicWebSocketPointer {
    ptr: *mut web_sys::WebSocket,
}

impl MagicWebSocketPointer {
    fn new(ws: &'static mut web_sys::WebSocket) -> Self {
        Self {
            ptr: ws as *mut web_sys::WebSocket,
        }
    }
}

impl std::ops::Deref for MagicWebSocketPointer {
    type Target = web_sys::WebSocket;

    fn deref(&self) -> &Self::Target {
        // SAFETY: wasm is always single-threaded and we leaked this ptr
        unsafe { &*self.ptr }
    }
}

unsafe impl Send for MagicWebSocketPointer {}
unsafe impl Sync for MagicWebSocketPointer {}

#[derive(Resource)]
pub struct WebSocketResource {
    websocket: MagicWebSocketPointer,
}

fn send_messages_over_websocket(
    mut commands: Commands,
    net_res: Res<ClientNetworkingResources>,
    ws_res: Res<WebSocketResource>,
    our_endpoint: Res<MainServerEndpoint>,
) {
    let taken_events;
    if let Some(mut m) = net_res
        .event_list_outgoing_websocket
        .get_mut(&our_endpoint.as_websocket().unwrap())
    {
        taken_events = std::mem::take(&mut *m);
    } else {
        taken_events = Vec::new();
    }
    info!(
        "Sending {} queued events over websocket",
        taken_events.len()
    );
    if taken_events.len() == 0 {
        return;
    }
    info!(e = ?taken_events[0], "Events");

    use shared::netlib::EventGroupingRef;
    let data = postcard::to_stdvec(&EventGroupingRef::Batch(&taken_events)).unwrap();

    ws_res.websocket.send_with_u8_array(&data).unwrap();
}

fn setup(
    mut commands: Commands,
    net_res: Res<ClientNetworkingResources>,
    endpoint: Res<MainServerEndpoint>,
) {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    //let canvas = document
    //.get_element_by_id("textbox")
    //.unwrap()
    //.dyn_into::<HtmlParagraphElement>()
    //.unwrap();

    //let mut inhtml = canvas.inner_html();
    //inhtml.push_str("Hello from Bevy Web!");

    //info!("We found the canvas element: {:?}", canvas);
    //get the host and port from document location

    //let ip = &net_res.con_str.0;
    //let port = net_res.con_str.1 + 1;
    //let url = format!("ws://{}:{}", ip, port);

    let url = "ws://71.126.177.34:25556";
    let ws = Box::new(web_sys::WebSocket::new(&url).unwrap());
    let mut_ws = Box::leak(ws);
    let magic_ws = MagicWebSocketPointer::new(mut_ws);
    let endpoint = endpoint.as_websocket().unwrap();

    let ws_clone = magic_ws.clone();
    let our_resources = net_res.clone();
    let onmessage_callback = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
        debug!(?e, "WebSocket message received");
        if let Ok(data) = e.data().dyn_into::<web_sys::Blob>() {
            let our_resources = our_resources.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let stream = data
                    .stream()
                    .get_reader()
                    .dyn_into::<web_sys::ReadableStreamDefaultReader>()
                    .unwrap();
                let mut chunk_from_blob = vec![];
                loop {
                    let chunk = wasm_bindgen_futures::JsFuture::from(stream.read())
                        .await
                        .expect("Reading from blob failed")
                        .dyn_into::<web_sys::js_sys::Object>()
                        .unwrap();
                    let done = Reflect::get(&chunk, &JsValue::from_str("done")).unwrap();
                    if done.is_truthy() {
                        break;
                    }
                    let chunk_value = Reflect::get(&chunk, &JsValue::from_str("value"))
                        .unwrap()
                        .dyn_into::<web_sys::js_sys::Uint8Array>()
                        .unwrap();
                    //copied from stackoverflow
                    let chunk_len = chunk_from_blob.len();
                    chunk_from_blob.resize(chunk_len + chunk_value.length() as usize, 0);
                    chunk_value.copy_to(&mut chunk_from_blob[chunk_len..]);
                }
                let data_len = chunk_from_blob.len();
                trace!("Received blob of size {}", data_len);

                use shared::netlib::EventGroupingOwned;
                let event: EventGroupingOwned<shared::event::client::EventToClient> =
                    match postcard::from_bytes(&chunk_from_blob) {
                        Ok(e) => e,
                        Err(p) => {
                            warn!("Got invalid json from server");
                            return;
                        }
                    };

                // TODO copied from netlib
                //res.networking_stats
                //.total_bytes_received_this_second
                //.fetch_add(data_len, std::sync::atomic::Ordering::Relaxed);
                //res.networking_stats
                //.packets_received_this_second
                //.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                let mut list = our_resources.event_list_incoming_websocket.write().unwrap();
                match event {
                    EventGroupingOwned::Single(x) => {
                        let pair = (endpoint, x);
                        list.push(pair);
                    }
                    EventGroupingOwned::Batch(events) => {
                        list.extend(events.into_iter().map(|x| (endpoint, x)));
                    }
                    EventGroupingOwned::Reliable(packet_id, _dedup_id, tick, events) => {
                        let mut seen_map = our_resources.reliable_packet_ids_seen.write().unwrap();

                        if seen_map.get(&packet_id).is_some() {
                            our_resources
                                .networking_stats
                                .total_bytes_received_ignored_this_second
                                .fetch_add(data_len, std::sync::atomic::Ordering::Relaxed);
                        } else {
                            seen_map.insert(packet_id, tick); // TODO store tick properly
                            list.extend(events.into_iter().map(|x| (endpoint, x)));
                        }
                    }
                }
            });
        } else {
            info!("Received non-text message");
            info!(data=?e.data(), "Data");
        }
    }) as Box<dyn FnMut(_)>);

    magic_ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();

    let onopen_callback = Closure::wrap(Box::new(move |_e: web_sys::MessageEvent| {
        info!("WebSocket connection opened");
        // in here we are going to loop forever and send packets from the outgoing queue
        //let our_endpoint = WebSocketEndpoint {
        //id: 0,
        //};
        //loop {
        //let taken_events;
        //if let Some(mut m) = our_resources.event_list_outgoing_websocket.get_mut(&our_endpoint) {
        //taken_events = std::mem::take(&mut *m);
        //} else {
        //taken_events = Vec::new();
        //}
        //info!("Sending {} queued events over websocket", taken_events.len());

        //}
    }) as Box<dyn FnMut(_)>);
    magic_ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
    onopen_callback.forget();

    commands.insert_resource(WebSocketResource {
        websocket: magic_ws,
    });
}
