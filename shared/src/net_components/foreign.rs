#![allow(unused)]
use serde::{Deserialize, Serialize};
use bevy::prelude::*;

pub struct UTransform(Transform);
pub struct URigidBody(avian3d::prelude::RigidBody);
pub struct UCollider(avian3d::prelude::Collider);
pub struct UMass(avian3d::prelude::Mass);

//include!(concat!(env!("OUT_DIR"), "/net_components_foreign.rs"));

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NetComponentForeign {
    Transform(Transform),
    RigidBody(avian3d::prelude::RigidBody),
    Collider(avian3d::prelude::Collider),
    Mass(avian3d::prelude::Mass),
}

impl NetComponentForeign {
    pub fn insert_components(
        self,
        entity: &mut EntityCommands<'_>,
    ) {
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
        }
    }
}
