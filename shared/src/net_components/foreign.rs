#![allow(unused)]
use avian3d::prelude::TransformInterpolation;
use bevy_internal::prelude::*;
use serde::{Deserialize, Serialize};

use crate::net_components::ToNetComponent;

//include!(concat!(env!("OUT_DIR"), "/net_components_foreign.rs"));

/// This is a simple wrapper to allow Color to be a bevy Component
#[derive(Component)]
pub struct ComponentColor(pub Color);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NetComponentForeign {
    Transform(Transform),
    RigidBody(avian3d::prelude::RigidBody),
    Collider(avian3d::prelude::Collider),
    Mass(avian3d::prelude::Mass),
    LinearVelocity(avian3d::prelude::LinearVelocity),
    AngularVelocity(avian3d::prelude::AngularVelocity),
    Rotation(avian3d::prelude::Rotation),
    TransformInterpolation,
    Color(Color),
}

impl NetComponentForeign {
    pub fn insert_components(self, entity: &mut EntityCommands<'_>) {
        match self {
            NetComponentForeign::Transform(c) => {
                entity.insert(c);
            }
            NetComponentForeign::RigidBody(c) => {
                entity.insert(c);
            }
            NetComponentForeign::Collider(c) => {
                entity.insert(c);
            }
            NetComponentForeign::Mass(c) => {
                entity.insert(c);
            }
            NetComponentForeign::LinearVelocity(c) => {
                entity.insert(c);
            }
            NetComponentForeign::AngularVelocity(c) => {
                entity.insert(c);
            }
            NetComponentForeign::Rotation(c) => {
                entity.insert(c);
            }
            NetComponentForeign::Color(c) => {
                entity.insert(ComponentColor(c));
            }
            NetComponentForeign::TransformInterpolation => {
                entity.insert(TransformInterpolation);
            }
        }
    }

    pub unsafe fn from_type_id_ptr(
        type_id: std::any::TypeId,
        ptr: bevy_internal::ptr::Ptr<'_>,
    ) -> Option<NetComponentForeign> {
        if type_id == std::any::TypeId::of::<Transform>() {
            Some(NetComponentForeign::Transform(
                unsafe { ptr.deref::<Transform>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<avian3d::prelude::RigidBody>() {
            Some(NetComponentForeign::RigidBody(
                unsafe { ptr.deref::<avian3d::prelude::RigidBody>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<avian3d::prelude::Collider>() {
            Some(NetComponentForeign::Collider(
                unsafe { ptr.deref::<avian3d::prelude::Collider>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<avian3d::prelude::Mass>() {
            Some(NetComponentForeign::Mass(
                unsafe { ptr.deref::<avian3d::prelude::Mass>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<avian3d::prelude::LinearVelocity>() {
            Some(NetComponentForeign::LinearVelocity(
                unsafe { ptr.deref::<avian3d::prelude::LinearVelocity>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<avian3d::prelude::AngularVelocity>() {
            Some(NetComponentForeign::AngularVelocity(
                unsafe { ptr.deref::<avian3d::prelude::AngularVelocity>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<avian3d::prelude::Rotation>() {
            Some(NetComponentForeign::Rotation(
                unsafe { ptr.deref::<avian3d::prelude::Rotation>() }.clone(),
            ))
        } else if type_id == std::any::TypeId::of::<ComponentColor>() {
            let color = unsafe { ptr.deref::<ComponentColor>() };
            Some(NetComponentForeign::Color(color.0))
        } else if type_id == std::any::TypeId::of::<TransformInterpolation>() {
            Some(NetComponentForeign::TransformInterpolation)
        } else {
            None
        }
    }
}

impl ToNetComponent for Transform {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Foreign(NetComponentForeign::Transform(self))
    }
}

impl ToNetComponent for avian3d::prelude::RigidBody {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Foreign(NetComponentForeign::RigidBody(self))
    }
}

impl ToNetComponent for avian3d::prelude::Collider {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Foreign(NetComponentForeign::Collider(self))
    }
}

impl ToNetComponent for avian3d::prelude::Mass {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Foreign(NetComponentForeign::Mass(self))
    }
}

impl ToNetComponent for avian3d::prelude::LinearVelocity {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Foreign(NetComponentForeign::LinearVelocity(self))
    }
}

impl ToNetComponent for avian3d::prelude::AngularVelocity {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Foreign(NetComponentForeign::AngularVelocity(self))
    }
}

impl ToNetComponent for avian3d::prelude::Rotation {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Foreign(NetComponentForeign::Rotation(self))
    }
}

impl ToNetComponent for Color {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Foreign(NetComponentForeign::Color(self))
    }
}

impl ToNetComponent for TransformInterpolation {
    fn to_net_component(self) -> super::NetComponent {
        super::NetComponent::Foreign(NetComponentForeign::TransformInterpolation)
    }
}
