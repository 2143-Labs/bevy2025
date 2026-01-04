use bevy::prelude::*;
use shared::{event::client::SpawnProjectile, netlib::ServerNetworkingResources};

use crate::{ConnectedPlayer, PlayerEndpoint, ServerState};

pub struct ProjectilePlugin;

impl Plugin for ProjectilePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(shared::projectile::ProjectilePlugin);
        app.add_systems(
            Update,
            (network_projectiles).run_if(in_state(ServerState::Running)),
        );
    }
}

fn network_projectiles(
    mut messager_reader: MessageReader<SpawnProjectile>,
    mut connected_clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
    sr: Res<ServerNetworkingResources>,
) {
    let events_collected = messager_reader
        .read()
        .cloned()
        .map(|e| crate::EventToClient::SpawnProjectile(e))
        .collect::<Vec<_>>();
    if events_collected.is_empty() {
        return;
    }

    for client in &mut connected_clients {
        sr.send_outgoing_event_next_tick_batch(client.0, &events_collected);
    }
}
