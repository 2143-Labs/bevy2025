use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize)]
pub struct Ball;

#[derive(Component, Serialize, Deserialize)]
pub struct Interactable;

//include!(concat!(env!("OUT_DIR"), "/net_components_ents.rs"));

pub enum NetComponentEnts {
    Ball(pub Ball),
    Interactable(pub Interactable),
}

impl NetComponentEnts {
    pub fn insert_components(
        self,
        entity: &mut EntityCommands<'_>,
    ) {
        match self {
            NetComponentEnts::Ball(c) => {
                entity.insert(c);
            }
            NetComponentEnts::Interactable(c) => {
                entity.insert(c);
            }
        }
    }
}
