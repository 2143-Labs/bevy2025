use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::event::NetEntId;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ProjectileAI {
    Straight {
        direction_vector: Vec3,
    },
    Homing {
        target_entity: NetEntId,
        turn_rate_deg_per_sec: f32,
    },
}
