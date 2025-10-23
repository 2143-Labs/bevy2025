//!This is types that can be sent over the network to spawn a unit.
//!This is divided into 3 groups:
//! - foreign: components in bevy we want to directly add to the entity
//! - ours: components we defined ourselves that we want to add to the entity
//! - groups: groups of components that we want to add to the entity
//! - ents: Marker Compoenents to identify entities
pub mod foreign;
pub mod ours;
pub mod ents;
pub mod groups;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

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
            "sphere" => Mesh::from(shape::Cube { size: self.1 }),
            _ => Mesh::from(shape::Cube { size: 1.0 }),
        };

        meshes.add(my_mesh)
    }
}

impl MaterialGenerator {
    pub fn new(name: impl AsRef<str>, color: Color) -> Self {
        MaterialGenerator(name.as_ref().to_string(), color)
    }

    pub fn generate(&self, materials: &mut ResMut<Assets<StandardMaterial>>) -> Handle<StandardMaterial> {
        let my_material = match self.0.as_str() {
            "basic" => StandardMaterial {
                base_color: self.1,
                metallic: 0.0,
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

pub enum NetComponent {
    Foreign(foreign::NetComponentForeign),
    Ours(ours::NetComponentOurs),
    Groups(groups::NetComponentGroups),
    Ents(ents::NetComponentEnts),
}

impl NetComponent {
    pub fn to_components(
        &self,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
    ) -> Vec<Component> {
        match self {
            NetComponent::Foreign(foreign) => foreign.to_components(meshes, materials),
            NetComponent::Ours(ours) => ours.to_components(meshes, materials),
            NetComponent::Groups(groups) => groups.to_components(meshes, materials),
            NetComponent::Ents(ents) => ents.to_components(meshes, materials),
        }
    }
}
