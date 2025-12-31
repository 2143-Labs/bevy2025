use bevy::prelude::*;
use shared::event::{UDPacketEvent, server::CastSkillUpdate};

pub struct AnimationPluginServer;

impl Plugin for AnimationPluginServer {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            on_unit_begin_skill_use,
        );
    }
}

fn on_unit_begin_skill_use(
    mut skill_change: UDPacketEvent<CastSkillUpdate>,
) {
    for event in skill_change.read() {
        error!(
            ?event,
            "Server received CastSkillUpdate event"
        );
    }
}
