//! LAN beacon for client-hosted testing gyms (see shared::gym).
use std::{
    net::{Ipv4Addr, UdpSocket},
    time::Duration,
};

use bevy::{prelude::*, time::common_conditions::on_timer};
use shared::gym::{BUILD_HASH, GYM_DISCOVERY_PORT, GYM_GAME_PORT, GymBeacon};

use crate::{ConnectedPlayer, ServerState};

pub struct GymBeaconPlugin {
    pub name: String,
}

#[derive(Resource)]
struct GymBeaconSender {
    socket: UdpSocket,
    beacon: GymBeacon,
}

impl Plugin for GymBeaconPlugin {
    fn build(&self, app: &mut App) {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).and_then(|s| {
            s.set_broadcast(true)?;
            s.set_nonblocking(true)?;
            Ok(s)
        });
        let socket = match socket {
            Ok(s) => s,
            Err(e) => {
                warn!("LAN gym beacon off: {e}");
                return;
            }
        };
        info!("Announcing gym on UDP broadcast port {GYM_DISCOVERY_PORT}");
        app.insert_resource(GymBeaconSender {
            socket,
            beacon: GymBeacon {
                instance: rand::random(),
                build: BUILD_HASH.to_string(),
                name: self.name.chars().take(32).collect(),
                game_port: GYM_GAME_PORT,
                players: 0,
            },
        });
        app.add_systems(
            Update,
            send_gym_beacon
                .run_if(in_state(ServerState::Running))
                .run_if(on_timer(Duration::from_secs(2))),
        );
    }
}

fn send_gym_beacon(
    mut sender: ResMut<GymBeaconSender>,
    players: Query<(), With<ConnectedPlayer>>,
    mut warned: Local<bool>,
) {
    sender.beacon.players = players.iter().count() as u32;
    let bytes = sender.beacon.encode();
    // Hosts do not always hear their own broadcast (a host firewall can drop it), so clients on
    // this machine also get a copy over loopback.
    for dest in [Ipv4Addr::BROADCAST, Ipv4Addr::LOCALHOST] {
        if let Err(e) = sender.socket.send_to(&bytes, (dest, GYM_DISCOVERY_PORT)) {
            if !*warned {
                warn!("gym beacon send to {dest} failed: {e}");
                *warned = true;
            } else {
                debug!("gym beacon send to {dest} failed: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use shared::{
        gym::GymBeacon,
        physics::terrain::{TerrainParams, generate_terrain_trimesh},
    };

    fn beacon() -> GymBeacon {
        GymBeacon {
            instance: 42,
            build: "0123456789ab".to_string(),
            name: "john@arch".to_string(),
            game_port: 2143,
            players: 3,
        }
    }

    #[test]
    fn beacon_roundtrip() {
        let b = beacon();
        assert_eq!(GymBeacon::decode(&b.encode()), Some(b));
    }

    #[test]
    fn beacon_rejects_foreign_packets() {
        assert_eq!(GymBeacon::decode(b"hello"), None);
        assert_eq!(GymBeacon::decode(b"ROTSGYM1not json"), None);
        // Valid JSON padded with whitespace: only the size limit rejects it.
        let mut oversized = beacon().encode();
        oversized.extend([b' '; 2000]);
        assert_eq!(GymBeacon::decode(&oversized), None);
    }

    #[test]
    fn gym_terrain_is_flat() {
        let params = TerrainParams::gym();
        assert!(!params.water);
        let (vertices, _) = generate_terrain_trimesh(&params);
        assert!(!vertices.is_empty());
        assert!(vertices.iter().all(|v| v.y == 0.0));
    }
}
