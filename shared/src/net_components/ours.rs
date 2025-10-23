use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Component, Debug, Eq, PartialEq, Clone, Copy)]
pub struct Health {
    pub hp: u32,
}
//include!(concat!(env!("OUT_DIR"), "/net_components_ours.rs"));

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NetComponentOurs {
    Health(Health),
}

impl NetComponentOurs {
    pub fn insert_components(
        self,
        entity: &mut EntityCommands<'_>,
    ) {
        match self {
            NetComponentOurs::Health(c) => {
                entity.insert(c);
            }
        }
    }
}


//commands.spawn((
    //Ball, // Marker component for counting
    //Mesh3d(meshes.add(Sphere::new(0.5))),
    //MeshMaterial3d(materials.add(StandardMaterial {
        //base_color: color,
        //metallic: 0.0,
        //perceptual_roughness: 0.5,
        //..default()
    //})),
    //Transform::from_translation(spawn_pos),
    //RigidBody::Dynamic,
    //Collider::sphere(0.5),
    //Mass(0.3), // Lighter balls that will float (density ~0.57 of water)
//));


//pub struct MeshMaterial // {
    //pub mesh: super::MeshRef,
    //pub material: super::MaterialRef,
//}
