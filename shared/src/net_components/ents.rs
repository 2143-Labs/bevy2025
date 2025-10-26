use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::net_components::ToNetComponent;

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Ball;

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Interactable;

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct PlayerCamera;

//include!(concat!(env!("OUT_DIR"), "/net_components_ents.rs"));

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NetComponentEnts {
    Ball(Ball),
    Interactable(Interactable),
    PlayerCamera(PlayerCamera),
}

impl NetComponentEnts {
    pub fn insert_components(self, entity: &mut EntityCommands<'_>) {
        match self {
            NetComponentEnts::Ball(c) => {
                entity.insert(c);
            }
            NetComponentEnts::Interactable(c) => {
                entity.insert(c);
            }
            NetComponentEnts::PlayerCamera(c) => {
                entity.insert(c);
            }
        }
    }
}

impl ToNetComponent for Ball {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ents(NetComponentEnts::Ball(self))
    }
}

impl ToNetComponent for Interactable {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ents(NetComponentEnts::Interactable(self))
    }
}

impl ToNetComponent for PlayerCamera {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ents(NetComponentEnts::PlayerCamera(self))
    }
}
