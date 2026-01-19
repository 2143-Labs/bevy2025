//!This is types that can be sent over the network to spawn a unit.
//!This is divided into 3 groups:
//! - foreign: components in bevy we want to directly add to the entity
//! - ours: components we defined ourselves that we want to add to the entity
//! - groups: groups of components that we want to add to the entity
//! - ents: Marker Compoenents to identify entities
pub mod ents;
pub mod foreign;
pub mod ours;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    character_controller::{CharacterControllerBundle, NPCControllerBundle},
    decimal::Decimal,
    event::{client::SpawnUnit2, NetEntId, PlayerId},
    net_components::ours::ControlledBy,
};

//include!(concat!(env!("OUT_DIR"), "/net_components.rs"));

/// A bevy component that can be sent over the network and added to an entity.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NetComponent {
    Foreign(foreign::NetComponentForeign),
    Ours(ours::NetComponentOurs),
    Ents(ents::NetComponentEnts),
    NetEntId(NetEntId),
    PlayerId(PlayerId),
    CharacterControllerBundle(Box<CharacterControllerBundle>),
    NPCControllerBundle(Box<NPCControllerBundle>),
}

#[cfg(test)]
mod test_net_component_size {
    use super::*;

    #[test]
    fn test_net_component_size() {
        use std::mem::size_of;
        println!("Size of NetComponent: {}", size_of::<NetComponent>());
        println!(
            "Size of Box<CharacterControllerBundle>: {}",
            size_of::<Box<CharacterControllerBundle>>()
        );
        println!("Size of NetEntId: {}", size_of::<NetEntId>());
        println!("Size of PlayerId: {}", size_of::<PlayerId>());
        println!(
            "Size of foreign::NetComponentForeign: {}",
            size_of::<foreign::NetComponentForeign>()
        );
        println!(
            "Size of ours::NetComponentOurs: {}",
            size_of::<ours::NetComponentOurs>()
        );
        println!(
            "Size of ents::NetComponentEnts: {}",
            size_of::<ents::NetComponentEnts>()
        );
        assert!(size_of::<NetComponent>() <= 64, "NetComponent is too large");
    }
}

use std::any::TypeId;
impl NetComponent {
    /// Insert components into world
    pub fn insert_components(self, ent_commands: &mut EntityCommands) {
        match self {
            NetComponent::Foreign(foreign) => foreign.insert_components(ent_commands),
            NetComponent::Ours(ours) => ours.insert_components(ent_commands),
            NetComponent::Ents(ents) => ents.insert_components(ent_commands),
            NetComponent::NetEntId(net_ent_id) => {
                ent_commands.insert(net_ent_id);
            }
            NetComponent::PlayerId(player_id) => {
                ent_commands.insert(player_id);
            }
            NetComponent::CharacterControllerBundle(bundle) => {
                ent_commands.insert(*bundle);
            }
            NetComponent::NPCControllerBundle(bundle) => {
                ent_commands.insert(*bundle);
            }
        }
    }

    pub unsafe fn from_type_id_ptr(
        type_id: TypeId,
        ptr: bevy::ptr::Ptr<'_>,
    ) -> Option<NetComponent> {
        if type_id == TypeId::of::<NetEntId>() {
            Some(NetComponent::NetEntId(
                unsafe { ptr.deref::<NetEntId>() }.clone(),
            ))
        } else if type_id == TypeId::of::<PlayerId>() {
            Some(NetComponent::PlayerId(
                unsafe { ptr.deref::<PlayerId>() }.clone(),
            ))
        } else if let Some(foreign) = unsafe { foreign::NetComponentForeign::from_type_id_ptr(type_id, ptr) } {
            Some(NetComponent::Foreign(foreign))
        } else if let Some(ours) = unsafe { ours::NetComponentOurs::from_type_id_ptr(type_id, ptr) } {
            Some(NetComponent::Ours(ours))
        } else if let Some(ents) = unsafe { ents::NetComponentEnts::from_type_id_ptr(type_id, ptr) } {
            Some(NetComponent::Ents(ents))
        // This won't happen because Bundles get expanded into their components on insert usually
        } else if type_id == TypeId::of::<CharacterControllerBundle>() {
            Some(NetComponent::CharacterControllerBundle(Box::new(
                unsafe { ptr.deref::<CharacterControllerBundle>() }.clone(),
            )))
        } else if type_id == TypeId::of::<NPCControllerBundle>() {
            Some(NetComponent::NPCControllerBundle(Box::new(
                unsafe { ptr.deref::<NPCControllerBundle>() }.clone(),
            )))
        } else {
            None
        }
    }
}

impl ToNetComponent for NetEntId {
    fn to_net_component(self) -> NetComponent {
        NetComponent::NetEntId(self)
    }
}
impl ToNetComponent for PlayerId {
    fn to_net_component(self) -> NetComponent {
        NetComponent::PlayerId(self)
    }
}

impl ToNetComponent for NPCControllerBundle {
    fn to_net_component(self) -> NetComponent {
        NetComponent::NPCControllerBundle(Box::new(self))
    }
}

impl ToNetComponent for CharacterControllerBundle {
    fn to_net_component(self) -> NetComponent {
        NetComponent::CharacterControllerBundle(Box::new(self))
    }
}

impl SpawnUnit2 {
    pub fn spawn_entity(self, commands: &mut Commands) -> Entity {
        let mut ent_commands = commands.spawn_empty();
        trace!(?self.net_ent_id, "Spawning entity with net_ent_id");
        if !self.net_ent_id.is_none() {
            ent_commands.insert(self.net_ent_id);
        }

        ent_commands.insert(self.net_ent_id);

        for net_comp in self.components {
            net_comp.insert_components(&mut ent_commands);
        }

        ent_commands.id()
    }

    pub fn new_with_vec(components: Vec<NetComponent>) -> Self {
        Self {
            net_ent_id: NetEntId::random(),
            components,
        }
    }

    pub fn new_with(components: impl IntoIterator<Item = NetComponent>) -> Self {
        Self {
            net_ent_id: NetEntId::random(),
            components: components.into_iter().collect(),
        }
    }
}

pub trait ToNetComponent {
    fn to_net_component(self) -> NetComponent;
}

pub fn make_ball(transform: Transform, color: Color, owner: ControlledBy) -> SpawnUnit2 {
    let sphere_size = 0.5;
    SpawnUnit2::new_with_vec(vec![
        owner.to_net_component(),
        ents::Ball(sphere_size).to_net_component(),
        ents::SendNetworkTranformUpdates.to_net_component(),
        avian3d::prelude::TransformInterpolation.to_net_component(),
        transform.to_net_component(),
        color.to_net_component(),
        avian3d::prelude::RigidBody::Dynamic.to_net_component(),
        avian3d::prelude::Collider::sphere(sphere_size).to_net_component(),
        avian3d::prelude::Mass(0.3).to_net_component(), // Lighter balls that will float (density ~0.57 of water)
                                                        // Add other ball components here as needed
    ])
}

pub fn make_small_loot(transform: Transform) -> SpawnUnit2 {
    SpawnUnit2::new_with_vec(vec![
        ents::ItemDrop { source: None }.to_net_component(),
        transform.to_net_component(),
    ])
}

pub fn make_man(transform: Transform, owner: ControlledBy, controller: &str) -> SpawnUnit2 {
    use crate::character_controller::CharacterControllerBundle;
    use avian3d::prelude::Collider;

    let mut comps = vec![
        owner.to_net_component(),
        ents::Man(3.0).to_net_component(),
        ents::SendNetworkTranformUpdates.to_net_component(),
        avian3d::prelude::TransformInterpolation.to_net_component(),
        ents::CanAssumeControl.to_net_component(),
        transform.to_net_component(),
        ours::BasePermanantStats::default().to_net_component(),
        //avian3d::prelude::RigidBody::Dynamic.to_net_component(),
        //avian3d::prelude::Collider::sphere(3.0).to_net_component(),
        //avian3d::prelude::Mass(70.0).to_net_component(),
    ];

    match controller {
        "TypeQ" => {
            comps.push(
                CharacterControllerBundle::new(Collider::capsule(1.0, 2.0), Vec3::NEG_Y * 9.81)
                    .with_movement(45.0, 0.9, 17.0, std::f32::consts::PI * 0.20)
                    .to_net_component(),
            );
        }
        "TypeE" => {
            comps.push(
                CharacterControllerBundle::new(Collider::capsule(1.0, 2.0), Vec3::NEG_Y * 9.81)
                    .with_movement(999.0, 0.8, 10.0, std::f32::consts::PI * 0.25)
                    .to_net_component(),
            );
        }
        _ => {}
    }

    SpawnUnit2::new_with_vec(comps)
}

pub fn make_npc(transform: Transform) -> SpawnUnit2 {
    use crate::character_controller::NPCControllerBundle;
    use avian3d::prelude::Collider;
    SpawnUnit2::new_with_vec(vec![
        ents::NPC.to_net_component(),
        ents::SendNetworkTranformUpdates.to_net_component(),
        avian3d::prelude::TransformInterpolation.to_net_component(),
        transform.to_net_component(),
        NPCControllerBundle::new(Collider::capsule(1.0, 2.0), Vec3::NEG_Y * 9.81)
            .with_movement(45.0, 0.9, 4.0, std::f32::consts::PI * 0.20)
            .to_net_component(),
        //avian3d::prelude::RigidBody::Dynamic.to_net_component(),
        //avian3d::prelude::Collider::sphere(3.0).to_net_component(),
        //avian3d::prelude::Mass(70.0).to_net_component(),
    ])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerConnectionInfo {
    pub ping: Decimal,
    pub packet_loss: Decimal,
}
