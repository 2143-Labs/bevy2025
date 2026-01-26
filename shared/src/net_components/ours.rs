use bevy_internal::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    character_controller::{
        CharacterController, ControllerGravity, GroundNormal, Groundedness, JumpBuffer,
        JumpImpulse, MaxSlopeAngle, MovementAcceleration, MovementAction, MovementDampingFactor,
        NPCController,
    },
    event::PlayerId,
    items::InventoryId,
    net_components::ToNetComponent,
    netlib::Tick,
};

//include!(concat!(env!("OUT_DIR"), "/net_components_ours.rs"));
#[derive(Serialize, Deserialize, Component, Debug, Eq, PartialEq, Clone)]
pub struct Health {
    pub hp: u32,
}

#[derive(Serialize, Deserialize, Component, Debug, Eq, PartialEq, Clone)]
pub struct PlayerName {
    pub name: String,
}

#[derive(Serialize, Deserialize, Component, Debug, Eq, PartialEq, Clone)]
pub struct ControlledBy {
    pub players: Vec<PlayerId>,
}

#[derive(Serialize, Deserialize, Component, Debug, PartialEq, Clone)]
pub struct PlayerColor {
    /// HSL hue value (0.0-360.0)
    pub hue: f32,
}

#[derive(Serialize, Deserialize, Component, Debug, PartialEq, Clone)]
pub struct DespawnOnPlayerDisconnect {
    pub player_id: PlayerId,
}

#[derive(Serialize, Deserialize, Component, Debug, PartialEq, Clone)]
pub struct HasInventory {
    pub inventory_id: InventoryId,
}

#[derive(Serialize, Deserialize, Component, Debug, PartialEq, Clone)]
pub struct Dead {
    pub reason: String,
    pub died_on_tick: Tick,
}

///// This struct represents all the possible things a unit might be trying to do this tick.
//#[derive(Serialize, Deserialize, Component, Debug, PartialEq, Clone)]
//pub enum ControlIntent {
//MoveTo3D(Vec3),
//Stop,
//}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NetComponentOurs {
    Health(Health),
    PlayerName(PlayerName),
    ControlledBy(ControlledBy),
    DespawnOnPlayerDisconnect(DespawnOnPlayerDisconnect),
    PlayerColor(PlayerColor),
    HasInventory(HasInventory),
    MovementAction(MovementAction),
    ControllerGravity(ControllerGravity),
    MovementAcceleration(MovementAcceleration),
    MovementDampingFactor(MovementDampingFactor),
    JumpImpulse(JumpImpulse),
    MaxSlopeAngle(MaxSlopeAngle),
    Groundedness(Groundedness),
    GroundNormal(GroundNormal),
    JumpBuffer(JumpBuffer),
    Dead(Dead),
    CharacterController(CharacterController),
    NPCController(NPCController),
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
            NetComponentOurs::ControlledBy(c) => {
                entity.insert(c);
            }
            NetComponentOurs::DespawnOnPlayerDisconnect(c) => {
                entity.insert(c);
            }
            NetComponentOurs::HasInventory(c) => {
                entity.insert(c);
            }
            NetComponentOurs::MovementAction(c) => {
                entity.insert(c);
            }
            NetComponentOurs::ControllerGravity(c) => {
                entity.insert(c);
            }
            NetComponentOurs::MovementAcceleration(c) => {
                entity.insert(c);
            }
            NetComponentOurs::MovementDampingFactor(c) => {
                entity.insert(c);
            }
            NetComponentOurs::JumpImpulse(c) => {
                entity.insert(c);
            }
            NetComponentOurs::MaxSlopeAngle(c) => {
                entity.insert(c);
            }
            NetComponentOurs::Groundedness(c) => {
                entity.insert(c);
            }
            NetComponentOurs::GroundNormal(c) => {
                entity.insert(c);
            }
            NetComponentOurs::JumpBuffer(c) => {
                entity.insert(c);
            }
            NetComponentOurs::Dead(c) => {
                entity.insert(c);
            }
            NetComponentOurs::CharacterController(c) => {
                entity.insert(c);
            }
            NetComponentOurs::NPCController(c) => {
                entity.insert(c);
            }
        }
    }

    pub unsafe fn from_type_id_ptr(
        type_id: std::any::TypeId,
        ptr: bevy_internal::ptr::Ptr<'_>,
    ) -> Option<NetComponentOurs> {
        if type_id == std::any::TypeId::of::<Health>() {
            Some(NetComponentOurs::Health(
                unsafe { ptr.deref::<Health>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<PlayerName>() {
            Some(NetComponentOurs::PlayerName(
                unsafe { ptr.deref::<PlayerName>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<PlayerColor>() {
            Some(NetComponentOurs::PlayerColor(
                unsafe { ptr.deref::<PlayerColor>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<ControlledBy>() {
            Some(NetComponentOurs::ControlledBy(
                unsafe { ptr.deref::<ControlledBy>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<DespawnOnPlayerDisconnect>() {
            Some(NetComponentOurs::DespawnOnPlayerDisconnect(
                unsafe { ptr.deref::<DespawnOnPlayerDisconnect>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<HasInventory>() {
            Some(NetComponentOurs::HasInventory(
                unsafe { ptr.deref::<HasInventory>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<MovementAction>() {
            Some(NetComponentOurs::MovementAction(
                unsafe { ptr.deref::<MovementAction>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<ControllerGravity>() {
            Some(NetComponentOurs::ControllerGravity(
                unsafe { ptr.deref::<ControllerGravity>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<MovementAcceleration>() {
            Some(NetComponentOurs::MovementAcceleration(
                unsafe { ptr.deref::<MovementAcceleration>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<MovementDampingFactor>() {
            Some(NetComponentOurs::MovementDampingFactor(
                unsafe { ptr.deref::<MovementDampingFactor>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<JumpImpulse>() {
            Some(NetComponentOurs::JumpImpulse(
                unsafe { ptr.deref::<JumpImpulse>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<MaxSlopeAngle>() {
            Some(NetComponentOurs::MaxSlopeAngle(
                unsafe { ptr.deref::<MaxSlopeAngle>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<Groundedness>() {
            Some(NetComponentOurs::Groundedness(
                unsafe { ptr.deref::<Groundedness>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<GroundNormal>() {
            Some(NetComponentOurs::GroundNormal(
                unsafe { ptr.deref::<GroundNormal>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<JumpBuffer>() {
            Some(NetComponentOurs::JumpBuffer(
                unsafe { ptr.deref::<JumpBuffer>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<Dead>() {
            Some(NetComponentOurs::Dead(
                unsafe { ptr.deref::<Dead>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<CharacterController>() {
            Some(NetComponentOurs::CharacterController(
                unsafe { ptr.deref::<CharacterController>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<NPCController>() {
            Some(NetComponentOurs::NPCController(
                unsafe { ptr.deref::<NPCController>() }.clone(),
            ))
        } else {
            None
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

impl ToNetComponent for ControlledBy {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::ControlledBy(self))
    }
}

impl ToNetComponent for DespawnOnPlayerDisconnect {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::DespawnOnPlayerDisconnect(self))
    }
}

impl ToNetComponent for MovementAction {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::MovementAction(self))
    }
}

impl ToNetComponent for ControllerGravity {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::ControllerGravity(self))
    }
}

impl ToNetComponent for MovementAcceleration {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::MovementAcceleration(self))
    }
}

impl ToNetComponent for MovementDampingFactor {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::MovementDampingFactor(self))
    }
}

impl ToNetComponent for JumpImpulse {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::JumpImpulse(self))
    }
}

impl ToNetComponent for MaxSlopeAngle {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::MaxSlopeAngle(self))
    }
}

impl ToNetComponent for Groundedness {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::Groundedness(self))
    }
}

impl ToNetComponent for GroundNormal {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::GroundNormal(self))
    }
}

impl ToNetComponent for JumpBuffer {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::JumpBuffer(self))
    }
}

impl ToNetComponent for HasInventory {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::HasInventory(self))
    }
}

impl ToNetComponent for Dead {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::Dead(self))
    }
}

impl ToNetComponent for CharacterController {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::CharacterController(self))
    }
}

impl ToNetComponent for NPCController {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Ours(NetComponentOurs::NPCController(self))
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

impl ControlledBy {
    pub fn single(player_id: PlayerId) -> Self {
        ControlledBy {
            players: vec![player_id],
        }
    }
}

impl DespawnOnPlayerDisconnect {
    pub fn new(player_id: PlayerId) -> Self {
        DespawnOnPlayerDisconnect { player_id }
    }
}
