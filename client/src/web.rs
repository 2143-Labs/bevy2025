use bevy::prelude::*;
use std::sync::Arc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::HtmlParagraphElement;

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
    }
}

#[derive(Resource)]
pub struct WebSocketResource {
    pub ws: Arc<web_sys::WebSocket>,
}

fn while_connecting(
    mut _commands: Commands,
    mut _next_game_state: ResMut<NextState<NetworkGameState>>,
) {
    //next_game_state.set(NetworkGameState::ClientConnected);
}

fn setup(
    mut commands: Commands,
    net_res: Res<ClientNetworkResources>,
) {
    //let mut window_desc = WebHandle::empty();
    //handle.id = 1;
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id("textbox")
        .unwrap()
        .dyn_into::<HtmlParagraphElement>()
        .unwrap();

    let mut inhtml = canvas.inner_html();
    inhtml.push_str("Hello from Bevy Web!");

    info!("We found the canvas element: {:?}", canvas);

    let ws = Arc::new(web_sys::WebSocket::new("ws://192.168.1.32:25556").unwrap());

    let onmessage_callback = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
        if let Ok(msg) = e.data().dyn_into() {
            web_sys::console::log_1(&msg)
        } else {
            info!("Received non-text message");
        }
    }) as Box<dyn FnMut(_)>);

    ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();

    let ws_clone = ws.clone();
    let onopen_callback = Closure::wrap(Box::new(move |_e: web_sys::MessageEvent| {
        ws_clone
            .send_with_str("Hello from Bevy WebSocket!")
            .unwrap();
        info!("WebSocket connection opened");
    }) as Box<dyn FnMut(_)>);
    ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
    onopen_callback.forget();

    commands.insert_resource(WebSocketResource { ws });
}
