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
    }
}
