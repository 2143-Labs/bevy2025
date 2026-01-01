use bevy::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

//use raw_window_handle::HasRawWindowHandle;

pub struct WebPlugin;

impl Plugin for WebPlugin {
    fn build(&self, _app: &mut App) {
        //let mut window_desc = WebHandle::empty();
        //handle.id = 1;
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let canvas = document
            .get_element_by_id("bevy-canvas")
            .unwrap()
            .dyn_into::<HtmlCanvasElement>()
            .unwrap();

        info!("We found the canvas element: {:?}", canvas);
    }
}
