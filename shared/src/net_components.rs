//!This is types that can be sent over the network to spawn a unit.
//!This is divided into 3 groups:
//! - foreign: components in bevy we want to directly add to the entity
//! - ours: components we defined ourselves that we want to add to the entity
//! - groups: groups of components that we want to add to the entity
//! - ents: Marker Compoenents to identify entities
pub mod ents;
pub mod foreign;
pub mod groups;
pub mod ours;

use bevy::prelude::*;
use bevy_pbr::StandardMaterial;
use serde::{Deserialize, Serialize};

use crate::{event::{MyNetEntParentId, NetEntId, PlayerId, client::SpawnUnit2}, net_components::ours::ControlledBy};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MeshGenerator(pub String, pub f32);
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MaterialGenerator(pub String, pub Color);

impl MeshGenerator {
    pub fn new(name: impl AsRef<str>, f: f32) -> Self {
        MeshGenerator(name.as_ref().to_string(), f)
    }

    pub fn generate(&self, meshes: &mut ResMut<Assets<Mesh>>) -> Handle<Mesh> {
        let my_mesh = match self.0.as_str() {
            "sphere" => Mesh::from(Sphere { radius: self.1 }),
            _ => Mesh::from(Sphere { radius: self.1 }),
        };

        meshes.add(my_mesh)
    }
}

impl MaterialGenerator {
    pub fn new(name: impl AsRef<str>, color: Color) -> Self {
        MaterialGenerator(name.as_ref().to_string(), color)
    }

    pub fn generate(
        &self,
        materials: &mut ResMut<Assets<StandardMaterial>>,
    ) -> Handle<StandardMaterial> {
        let my_material = match self.0.as_str() {
            "basic" => StandardMaterial {
                base_color: self.1,
                metallic: 0.0,
                perceptual_roughness: 0.5,
                ..default()
            },
            "basic2" => StandardMaterial {
                base_color: self.1,
                metallic: 1.0,
                perceptual_roughness: 0.5,
                ..default()
            },
            _ => StandardMaterial {
                base_color: self.1,
                metallic: 0.0,
                perceptual_roughness: 0.5,
                ..default()
            },
        };

        materials.add(my_material)
    }
}

//include!(concat!(env!("OUT_DIR"), "/net_components.rs"));

/// A bevy component that can be sent over the network and added to an entity.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NetComponent {
    Foreign(foreign::NetComponentForeign),
    Ours(ours::NetComponentOurs),
    Groups(groups::NetComponentGroups),
    Ents(ents::NetComponentEnts),
    MyNetEntParentId(MyNetEntParentId),
    NetEntId(NetEntId),
    PlayerId(PlayerId),
}

impl NetComponent {
    /// Insert components (client) into given entity. If you dont have meshes/materials, use
    /// [`Self::insert_components_srv`] instead.
    pub fn insert_components(
        self,
        ent_commands: &mut EntityCommands,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
    ) {
        match self {
            NetComponent::Foreign(foreign) => foreign.insert_components(ent_commands),
            NetComponent::Ours(ours) => ours.insert_components(ent_commands),
            NetComponent::Groups(groups) => {
                groups.insert_components(ent_commands, meshes, materials)
            }
            NetComponent::Ents(ents) => ents.insert_components(ent_commands),
            NetComponent::MyNetEntParentId(my_net_ent_parent_id) => {
                ent_commands.insert(my_net_ent_parent_id);
            }
            NetComponent::NetEntId(net_ent_id) => {
                ent_commands.insert(net_ent_id);
            }
            NetComponent::PlayerId(player_id) => {
                ent_commands.insert(player_id);
            }
        }
    }

    pub fn insert_components_srv(self, ent_commands: &mut EntityCommands) {
        match self {
            NetComponent::Foreign(foreign) => foreign.insert_components(ent_commands),
            NetComponent::Ours(ours) => ours.insert_components(ent_commands),
            NetComponent::Groups(groups) => {
                groups.insert_components_srv(ent_commands);
            }
            NetComponent::Ents(ents) => ents.insert_components(ent_commands),
            NetComponent::MyNetEntParentId(my_net_ent_parent_id) => {
                ent_commands.insert(my_net_ent_parent_id);
            }
            NetComponent::NetEntId(net_ent_id) => {
                ent_commands.insert(net_ent_id);
            }
            NetComponent::PlayerId(player_id) => {
                ent_commands.insert(player_id);
            }
        }
    }
}

impl ToNetComponent for MyNetEntParentId {
    fn to_net_component(self) -> NetComponent {
        NetComponent::MyNetEntParentId(self)
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
    pub fn spawn_entity(
        self,
        commands: &mut Commands,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
    ) -> Entity {
        let mut ent_commands = commands.spawn(());
        if !self.net_ent_id.is_none() {
            ent_commands.insert(self.net_ent_id);
        }

        ent_commands.insert(self.net_ent_id);

        for net_comp in self.components {
            net_comp.insert_components(&mut ent_commands, meshes, materials);
        }

        ent_commands.id()
    }

    pub fn spawn_entity_srv(self, commands: &mut Commands) -> Entity {
        let mut ent_commands = commands.spawn(());
        if !self.net_ent_id.is_none() {
            ent_commands.insert(self.net_ent_id);
        }

        for net_comp in self.components {
            net_comp.insert_components_srv(&mut ent_commands);
        }

        ent_commands.id()
    }
}

pub trait ToNetComponent {
    fn to_net_component(self) -> NetComponent;
}

pub fn make_ball(net_ent_id: NetEntId, transform: Transform, color: Color, owner: ControlledBy)-> SpawnUnit2 {
    let sphere_size = 0.5;
    SpawnUnit2 {
        net_ent_id,
        components: vec![
            owner.to_net_component(),
            ents::Ball.to_net_component(),
            ents::SendNetworkTranformUpdates.to_net_component(),
            transform.to_net_component(),
            groups::NormalMeshMaterial {
                mesh: MeshGenerator("sphere".to_string(), sphere_size),
                material: MaterialGenerator("basic".to_string(), color),
            }
            .to_net_component(),
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
            ents::Man.to_net_component(),
            ents::SendNetworkTranformUpdates.to_net_component(),
            transform.to_net_component(),
            groups::NormalMeshMaterial {
                mesh: MeshGenerator::new("sphere", 3.0),
                material: MaterialGenerator::new("basic", Color::linear_rgb(0.8, 0.7, 0.6)),
            }.to_net_component(),
            avian3d::prelude::RigidBody::Dynamic.to_net_component(),
            avian3d::prelude::Collider::sphere(3.0).to_net_component(),
            avian3d::prelude::Mass(70.0).to_net_component(),
        ]
    }
}
