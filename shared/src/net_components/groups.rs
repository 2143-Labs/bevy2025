use bevy::prelude::*;
use bevy_pbr::StandardMaterial;
use serde::{Deserialize, Serialize};

use crate::net_components::ToNetComponent;

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct NormalMeshMaterial {
    pub mesh: super::MeshGenerator,
    pub material: super::MaterialGenerator,
}

//include!(concat!(env!("OUT_DIR"), "/net_components_groups.rs"));

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NetComponentGroups {
    NormalMeshMaterial(NormalMeshMaterial),
}

impl NetComponentGroups {
    pub fn insert_components_client(
        self,
        entity: &mut EntityCommands<'_>,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
    ) {
        match self {
            NetComponentGroups::NormalMeshMaterial(c) => {
                let mesh_handle = c.mesh.generate(meshes);
                let material_handle = c.material.generate(materials);

                entity.insert((
                    Mesh3d(mesh_handle),
                    bevy_pbr::MeshMaterial3d(material_handle),
                ));
            }
        }
    }

    pub fn insert_components_srv(self, entity: &mut EntityCommands<'_>) {
        match self {
            NetComponentGroups::NormalMeshMaterial(c) => {
                entity.insert(c);
            }
        }
    }
}

impl ToNetComponent for NormalMeshMaterial {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Groups(NetComponentGroups::NormalMeshMaterial(self))
    }
}
