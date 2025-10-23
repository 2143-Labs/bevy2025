
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NormalMeshMaterial {
    pub mesh: super::MeshGenerator,
    pub material: super::MaterialGenerator,
}

//include!(concat!(env!("OUT_DIR"), "/net_components_groups.rs"));

pub enum NetComponentGroups {
    NormalMeshMaterial(pub NormalMeshMaterial),
}

impl NetComponentGroups {
    pub fn insert_components(
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
                    mesh_handle,
                    material_handle,
                ));
            }
            n => {
                error!(?n, "Invalid NetComponentGroups variant");
            }
        }
    }
}
