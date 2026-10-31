use bevy::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::HtmlParagraphElement;
use web_sys::js_sys::Reflect;

use shared::netlib::{
    ClientNetworkingResources, EndpointGeneral, MainServerEndpoint, WebSocketEndpoint,
};

//use raw_window_handle::HasRawWindowHandle;

pub struct WebPlugin;

use crate::game_state::NetworkGameState;

/// Placeholder UDP target for the message_io web stub, which only needs a parseable
/// `SocketAddr`. Web traffic goes over the WebSocket from [`game_server_url`].
const WEB_STUB_ADDR: (&str, u16) = ("127.0.0.1", 1);

/// Game server pod 0 on the page's own host, through the gateway's `/ws/0` route.
pub fn game_server_url() -> String {
    let location = web_sys::window().expect("no window").location();
    let scheme = match location.protocol().as_deref() {
        Ok("http:") => "ws",
        _ => "wss",
    };
    let host = location.host().expect("no location host");
    format!("{scheme}://{host}/ws/0")
}

impl Plugin for WebPlugin {
    fn build(&self, app: &mut App) {
        info!("Initializing web plugin");
        let document = web_sys::window().unwrap().document().unwrap();
        let loading_element = document
            .get_element_by_id("loading")
            .expect("No loading_status element in html")
            // p tag
            .dyn_into::<HtmlParagraphElement>()
            .expect("loading element is not a paragraph element");

        loading_element.set_text_content(Some(""));

        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                canvas: Some("#bevy-canvas".into()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }));

        app.add_systems(OnEnter(NetworkGameState::ClientSendRequestPacket), setup);
        //while in ClientConencting, run
        app.add_systems(
            Update,
            while_connecting.run_if(in_state(NetworkGameState::ClientConnecting)),
        );

        use crate::Config;
        app.add_systems(Startup, |mut config: ResMut<Config>| {
            config.ip = WEB_STUB_ADDR.0.to_string();
            config.port = WEB_STUB_ADDR.1;
        });

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

/// This type exists to allow us to store a web_sys::WebSocket in a Resource while still being Send
/// + Sync. This is safe because we arent calling this from a WebWorker.
#[derive(Clone)]
struct MagicWebSocketPointer {
    ptr: *mut web_sys::WebSocket,
}

impl MagicWebSocketPointer {
    fn new(ws: web_sys::WebSocket) -> Self {
        let mut_ws = Box::leak(Box::new(ws));
        Self {
            ptr: mut_ws as *mut web_sys::WebSocket,
        }
    }
}

impl std::ops::Deref for MagicWebSocketPointer {
    type Target = web_sys::WebSocket;

    fn deref(&self) -> &Self::Target {
        // SAFETY: We aren't using any WebWorkers, and we created this pointer from a static
        // reference
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
    net_res: Res<ClientNetworkingResources>,
    ws_res: Res<WebSocketResource>,
    our_endpoint: Res<MainServerEndpoint>,
) {
    // Between a reconnect's ClientConnecting (UDP endpoint) and `setup` (WebSocket endpoint).
    let Some(endpoint) = our_endpoint.as_websocket() else {
        return;
    };
    let taken_events;
    if let Some(mut m) = net_res.event_list_outgoing_websocket.get_mut(&endpoint) {
        taken_events = std::mem::take(&mut *m);
    } else {
        taken_events = Vec::new();
    }

    trace!(
        "Sending {} queued events over websocket",
        taken_events.len()
    );
    if taken_events.is_empty() {
        return;
    }

    use shared::netlib::EventGroupingRef;
    let data = postcard::to_stdvec(&EventGroupingRef::Batch(&taken_events)).unwrap();

    // don't check result
    let _ = ws_res.websocket.send_with_u8_array(&data);
}

fn setup(mut commands: Commands, net_res: Res<ClientNetworkingResources>) {
    // `setup_incoming_client` inserted a UDP endpoint for the stub; on web everything goes over
    // this WebSocket instead. The address is only a key for the outgoing queue.
    let ws_endpoint = WebSocketEndpoint {
        socket_addr: format!("{}:{}", WEB_STUB_ADDR.0, WEB_STUB_ADDR.1)
            .parse()
            .unwrap(),
    };
    commands.insert_resource(MainServerEndpoint(EndpointGeneral::WebSocket(ws_endpoint)));

    let url = game_server_url();
    info!("Connecting to game server at {url}");
    let ws = web_sys::WebSocket::new(&url).unwrap();
    let magic_ws = MagicWebSocketPointer::new(ws);

    let our_resources = net_res.clone();
    let onmessage_callback = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
        debug!(?e, "WebSocket message received");

        let data = match e.data().dyn_into::<web_sys::Blob>() {
            Ok(data) => data,
            Err(err) => {
                warn!("Received non-blob message from socket");
                info!(data=?e.data(), ?err, "Data");
                return;
            }
        };

        let our_resources = our_resources.clone();
        wasm_bindgen_futures::spawn_local(async move {
            // Because this is a websocket, we need to read the blob in chunks. Use the javascript
            // api to do this
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

                if chunk_from_blob.len() > 100 * 1024 * 1024 {
                    error!("Blob from websocket is too large, aborting without processing");
                    return;
                }
            }

            let data_len = chunk_from_blob.len();
            trace!("Received blob of size {}", data_len);

            let data_ptr = our_resources.event_list_incoming_websocket.clone();
            shared::netlib::on_data_incoming(
                &our_resources,
                ws_endpoint,
                data_ptr,
                &chunk_from_blob,
            );
        });
    }) as Box<dyn FnMut(_)>);
    magic_ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();

    let onopen_callback = Closure::wrap(Box::new(move |_e: web_sys::MessageEvent| {
        info!("WebSocket connection opened with server");
    }) as Box<dyn FnMut(_)>);
    magic_ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
    onopen_callback.forget();

    commands.insert_resource(WebSocketResource {
        websocket: magic_ws,
    });
}
