use bevy_internal::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInputInterp {
    pub camera_facing: Quat,
    /// Vec3 is (x: right, y: up, z: forward)
    pub movement_input: Vec3,
    pub jump_pressed: bool,
}
