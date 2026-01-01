use bevy::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

use raw_window_handle::{WebHandle, HasRawWindowHandle};

pub struct WebPlugin;

impl Plugin for WebPlugin {
    fn build(&self, app: &mut App) {
        let mut window_desc = WebHandle::empty();
        handle.id = 1;
        let canvas = HtmlCanvasElement::window()
            .unwrap();
    }
}
