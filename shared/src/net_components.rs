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
}

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
}

pub trait ToNetComponent {
    fn to_net_component(self) -> NetComponent;
}

pub fn make_ball(
    net_ent_id: NetEntId,
    transform: Transform,
    color: Color,
    owner: ControlledBy,
) -> SpawnUnit2 {
    let sphere_size = 0.5;
    SpawnUnit2 {
        net_ent_id,
        components: vec![
            owner.to_net_component(),
            ents::Ball(sphere_size).to_net_component(),
            ents::SendNetworkTranformUpdates.to_net_component(),
            transform.to_net_component(),
            color.to_net_component(),
            avian3d::prelude::RigidBody::Dynamic.to_net_component(),
            avian3d::prelude::Collider::sphere(sphere_size).to_net_component(),
            avian3d::prelude::Mass(0.3).to_net_component(), // Lighter balls that will float (density ~0.57 of water)
                                                            // Add other ball components here as needed
        ],
    }
}

pub fn make_man(net_ent_id: NetEntId, transform: Transform, owner: ControlledBy) -> SpawnUnit2 {
    SpawnUnit2 {
        net_ent_id,
        components: vec![
            owner.to_net_component(),
            ents::Man(3.0).to_net_component(),
            ents::SendNetworkTranformUpdates.to_net_component(),
            ents::CanAssumeControl.to_net_component(),
            transform.to_net_component(),
            //avian3d::prelude::RigidBody::Dynamic.to_net_component(),
            //avian3d::prelude::Collider::sphere(3.0).to_net_component(),
            //avian3d::prelude::Mass(70.0).to_net_component(),
        ],
    }
}
