use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::net_components::ToNetComponent;

/// Simple Network physics entity
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Ball(pub f32);

/// This is the entity corresponding to the physical camera entity of the player
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct PlayerCamera;

/// Controllable player entity
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Man(pub f32);

/// Simple Interactable entity
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct CanAssumeControl;

/// Anything with this component will have its transform sent over the network regularly from the
/// server. (should maybe move to "ours" category)
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct SendNetworkTranformUpdates;

//include!(concat!(env!("OUT_DIR"), "/net_components_ents.rs"));

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NetComponentEnts {
    Ball(Ball),
    CanAssumeControl(CanAssumeControl),
    SendNetworkTranformUpdates(SendNetworkTranformUpdates),
    PlayerCamera(PlayerCamera),
    Man(Man),
}

impl NetComponentEnts {
    pub fn insert_components(self, entity: &mut EntityCommands<'_>) {
        match self {
            NetComponentEnts::Ball(c) => {
                entity.insert(c);
            }
            NetComponentEnts::CanAssumeControl(c) => {
                entity.insert(c);
            }
            NetComponentEnts::SendNetworkTranformUpdates(c) => {
                entity.insert(c);
            }
            NetComponentEnts::PlayerCamera(c) => {
                entity.insert(c);
            }
            NetComponentEnts::Man(c) => {
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

impl ToNetComponent for CanAssumeControl {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ents(NetComponentEnts::CanAssumeControl(self))
    }
}

impl ToNetComponent for PlayerCamera {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ents(NetComponentEnts::PlayerCamera(self))
    }
}

impl ToNetComponent for Man {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ents(NetComponentEnts::Man(self))
    }
}

impl ToNetComponent for SendNetworkTranformUpdates {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ents(NetComponentEnts::SendNetworkTranformUpdates(self))
    }
}
