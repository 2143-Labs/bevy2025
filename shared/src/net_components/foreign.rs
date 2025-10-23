use bevy::prelude::*;

pub struct UTransform(pub Transform);
pub struct URigidBody(pub bevy_rapier3d::prelude::RigidBody);
pub struct UCollider(pub bevy_rapier3d::prelude::Collider);
pub struct UMass(pub bevy_rapier3d::prelude::Mass);

//include!(concat!(env!("OUT_DIR"), "/net_components_foreign.rs"));

pub enum NetComponentForeign {
    Transform(pub Transform),
    RigidBody(pub bevy_rapier3d::prelude::RigidBody),
    Collider(pub bevy_rapier3d::prelude::Collider),
    Mass(pub bevy_rapier3d::prelude::Mass),
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
