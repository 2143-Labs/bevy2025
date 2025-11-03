use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::net_components::ToNetComponent;

#[derive(Serialize, Deserialize, Component, Debug, Eq, PartialEq, Clone)]
pub struct Health {
    pub hp: u32,
}

#[derive(Serialize, Deserialize, Component, Debug, Eq, PartialEq, Clone)]
pub struct PlayerName {
    pub name: String,
}

#[derive(Serialize, Deserialize, Component, Debug, PartialEq, Clone)]
pub struct PlayerColor {
    /// HSL hue value (0.0-360.0)
    pub hue: f32,
}
//include!(concat!(env!("OUT_DIR"), "/net_components_ours.rs"));

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NetComponentOurs {
    Health(Health),
    PlayerName(PlayerName),
    PlayerColor(PlayerColor),
}

impl NetComponentOurs {
    pub fn insert_components(self, entity: &mut EntityCommands<'_>) {
        match self {
            NetComponentOurs::Health(c) => {
                entity.insert(c);
            }
            NetComponentOurs::PlayerName(c) => {
                entity.insert(c);
            }
            NetComponentOurs::PlayerColor(c) => {
                entity.insert(c);
            }
        }
    }
}

impl ToNetComponent for Health {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::Health(self))
    }
}

impl ToNetComponent for PlayerName {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::PlayerName(self))
    }
}

impl ToNetComponent for PlayerColor {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::PlayerColor(self))
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
