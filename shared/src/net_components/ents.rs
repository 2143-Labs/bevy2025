use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{event::NetEntId, net_components::ToNetComponent};

/// Simple Network physics entity
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Ball(pub f32);

/// This is the entity corresponding to the physical camera entity of the player
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct PlayerCamera;

/// Controllable player entity
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Man(pub f32);

/// Controllable player entity
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct NPC;

/// Simple Interactable entity
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct CanAssumeControl;

/// Anything with this component will have its transform sent over the network regularly from the
/// server. (should maybe move to "ours" category)
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct SendNetworkTranformUpdates;

/// Controllable player entity
#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct ItemDrop {
    pub source: Option<NetEntId>,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Tower;

//include!(concat!(env!("OUT_DIR"), "/net_components_ents.rs"));

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NetComponentEnts {
    Ball(Ball),
    CanAssumeControl(CanAssumeControl),
    SendNetworkTranformUpdates(SendNetworkTranformUpdates),
    PlayerCamera(PlayerCamera),
    Man(Man),
    NPC(NPC),
    ItemDrop(ItemDrop),
    Tower(Tower),
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
            NetComponentEnts::ItemDrop(c) => {
                entity.insert(c);
            }
            NetComponentEnts::NPC(c) => {
                entity.insert(c);
            }
            NetComponentEnts::Tower(c) => {
                entity.insert(c);
            }
        }
    }

    pub unsafe fn from_type_id_ptr(
        type_id: std::any::TypeId,
        ptr: bevy::ptr::Ptr<'_>,
    ) -> Option<NetComponentEnts> {
        if type_id == std::any::TypeId::of::<Ball>() {
            Some(NetComponentEnts::Ball(
                unsafe { ptr.deref::<Ball>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<CanAssumeControl>() {
            Some(NetComponentEnts::CanAssumeControl(
                unsafe { ptr.deref::<CanAssumeControl>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<PlayerCamera>() {
            Some(NetComponentEnts::PlayerCamera(
                unsafe { ptr.deref::<PlayerCamera>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<Man>() {
            Some(NetComponentEnts::Man(unsafe { ptr.deref::<Man>() }.clone()))
        } else if type_id == std::any::TypeId::of::<SendNetworkTranformUpdates>() {
            Some(NetComponentEnts::SendNetworkTranformUpdates(
                unsafe { ptr.deref::<SendNetworkTranformUpdates>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<ItemDrop>() {
            Some(NetComponentEnts::ItemDrop(
                unsafe { ptr.deref::<ItemDrop>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<NPC>() {
            Some(NetComponentEnts::NPC(unsafe { ptr.deref::<NPC>() }.clone()))
        } else if type_id == std::any::TypeId::of::<Tower>() {
            Some(NetComponentEnts::Tower(
                unsafe { ptr.deref::<Tower>() }.clone(),
            ))
        } else {
            None
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

impl ToNetComponent for ItemDrop {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ents(NetComponentEnts::ItemDrop(self))
    }
}

impl ToNetComponent for NPC {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ents(NetComponentEnts::NPC(self))
    }
}

impl ToNetComponent for Tower {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ents(NetComponentEnts::Tower(self))
    }
}
