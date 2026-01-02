use bevy::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlParagraphElement;

//use raw_window_handle::HasRawWindowHandle;

pub struct WebPlugin;

impl Plugin for WebPlugin {
    fn build(&self, _app: &mut App) {
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

        let ws = web_sys::WebSocket::new("ws://71.126.177.34:25556/").unwrap();

        let onmessage_callback = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                let msg = txt.as_string().unwrap();
                info!("Received message: {}", msg);
            } else {
                info!("Received non-text message");
            }
        }) as Box);

        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        ws.set_onopen(Some(
            Closure::wrap(Box::new(move |_| {
                info!("WebSocket connection opened");
            }) as Box<dyn FnMut(_)>)
            .as_ref()
            .unchecked_ref(),
        ));
    }
}
