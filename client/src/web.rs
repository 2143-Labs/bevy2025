use bevy::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::HtmlParagraphElement;
use web_sys::js_sys::Reflect;

use shared::netlib::ClientNetworkingResources;
use shared::netlib::MainServerEndpoint;

//use raw_window_handle::HasRawWindowHandle;

pub struct WebPlugin;

use crate::game_state::NetworkGameState;

impl Plugin for WebPlugin {
    fn build(&self, app: &mut App) {
        info!("Initializing web plugin");
        // get our default location
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let ip_str: String = document
            .get_element_by_id("server_ip")
            .expect("No server_ip element in html")
            .text_content()
            .expect("server_ip element has no text content");

        let port: u16 = document
            .get_element_by_id("server_port")
            .expect("No server_port element in html")
            .text_content()
            .expect("server_port element has no text content")
            .parse()
            .expect("server_port element text content is not a valid u16");

        let ip_addr: std::net::IpAddr = ip_str
            .parse()
            .expect("server_ip element text content is not a valid IpAddr");

        let loading_element = document
            .get_element_by_id("loading")
            .expect("No loading_status element in html")
            // p tag
            .dyn_into::<HtmlParagraphElement>()
            .expect("loading element is not a paragraph element");

        let localstorage_player_id = document
            .get_element_by_id("localstorage_player_id")
            .expect("No player_id element in html")
            .dyn_into::<HtmlParagraphElement>()
            .expect("player_id element is not a paragraph element")
            .text_content();
        let localstorage_token = document
            .get_element_by_id("localstorage_login_token")
            .expect("No auth_token element in html")
            .dyn_into::<HtmlParagraphElement>()
            .expect("auth_token element is not a paragraph element")
            .text_content();

        if let (Some(token), Some(player_id)) = (localstorage_token, localstorage_player_id) {
            info!("Found auth token and player id in localstorage, using them for authentication");
            app.insert_resource(crate::login::LoginServerResource {
                temp_auth_token: token,
                player_id: shared::event::PlayerId(
                    player_id
                        .parse()
                        .expect("player_id in localstorage is not a valid u64"),
                ),
            });
        } else {
            info!("No auth token or player id found in localstorage, starting unauthenticated");
        }

        loading_element.set_text_content(Some(""));

        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                canvas: Some("#bevy-canvas".into()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }));
        //app.add_plugins(DefaultPlugins);

        //app.insert_resource(MainServerEndpoint(
        //shared::netlib::EndpointGeneral::WebSocket(shared::netlib::WebSocketEndpoint {
        //socket_addr: (ip_addr, port).into(),
        //}),
        //));

        // TODO
        //app.insert_resource(crate::network::AuthServerEndpoint(auth_server));

        info!("Connecting to server at {}:{}", ip_str, port);

        // TODO use this to query for active servers
        //commands.insert_resource(crate::login::LoginServerResource {
        //player_id: PlayerId(random::random_range(0..=u64::MAX)),
        //temp_auth_token: "yippee".to_string(),
        //});

        app.add_systems(OnEnter(NetworkGameState::ClientSendRequestPacket), setup);
        //while in ClientConencting, run
        app.add_systems(
            Update,
            while_connecting.run_if(in_state(NetworkGameState::ClientConnecting)),
        );

        use crate::Config;
        app.add_systems(Startup, move |mut config: ResMut<Config>| {
            config.ip = ip_str.clone();
            config.port = port.clone();
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
    let taken_events;
    if let Some(mut m) = net_res
        .event_list_outgoing_websocket
        .get_mut(&our_endpoint.as_websocket().unwrap())
    {
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

fn setup(
    mut commands: Commands,
    net_res: Res<ClientNetworkingResources>,
    main_server_endpoint: Res<MainServerEndpoint>,
) {
    let ws_endpoint = main_server_endpoint.as_websocket().unwrap();
    let addr = ws_endpoint.socket_addr;
    let url = format!("ws://{}:{}", addr.ip(), addr.port());
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
