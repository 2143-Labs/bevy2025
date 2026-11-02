//! Listens for LAN testing gym beacons (shared::gym) and keeps `DiscoveredGyms` current.
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::mpsc,
    time::{Duration, Instant},
};

use bevy::prelude::*;
use shared::gym::{GYM_DISCOVERY_PORT, GymBeacon};
use socket2::{Domain, Protocol, Socket, Type};

/// A gym not heard from for this long (3 missed beacons) is dropped from the list.
const GYM_EXPIRY: Duration = Duration::from_secs(6);

pub struct DiscoveredGym {
    pub ip: IpAddr,
    pub beacon: GymBeacon,
    last_seen: Instant,
}

/// Known gyms, sorted by name then IP. Only display changes trigger change detection.
#[derive(Resource, Default)]
pub struct DiscoveredGyms(pub Vec<DiscoveredGym>);

#[derive(Resource)]
struct GymBeaconRx(parking_lot::Mutex<mpsc::Receiver<(GymBeacon, IpAddr)>>);

pub struct GymDiscoveryPlugin;

impl Plugin for GymDiscoveryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DiscoveredGyms>()
            .add_systems(Startup, start_gym_listener)
            .add_systems(
                Update,
                update_discovered_gyms.run_if(resource_exists::<GymBeaconRx>),
            );
    }
}

/// Several clients on one machine all listen, hence SO_REUSEADDR.
fn bind_discovery_socket() -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.bind(&SocketAddr::from((Ipv4Addr::UNSPECIFIED, GYM_DISCOVERY_PORT)).into())?;
    Ok(UdpSocket::from(socket))
}

fn start_gym_listener(mut commands: Commands) {
    let socket = match bind_discovery_socket() {
        Ok(s) => s,
        Err(e) => {
            warn!("LAN gym discovery off: cannot listen on UDP {GYM_DISCOVERY_PORT}: {e}");
            return;
        }
    };
    info!("Listening for LAN testing gyms on UDP {GYM_DISCOVERY_PORT}");
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("gym-discovery".into())
        .spawn(move || {
            let mut buf = [0u8; 1500];
            loop {
                let (len, src) = match socket.recv_from(&mut buf) {
                    Ok(r) => r,
                    Err(e) => {
                        debug!("gym discovery recv failed: {e}");
                        continue;
                    }
                };
                let Some(beacon) = GymBeacon::decode(&buf[..len]) else {
                    continue;
                };
                if tx.send((beacon, src.ip())).is_err() {
                    break;
                }
            }
        });
    if let Err(e) = spawned {
        warn!("LAN gym discovery off: cannot start listener thread: {e}");
        return;
    }
    commands.insert_resource(GymBeaconRx(parking_lot::Mutex::new(rx)));
}

fn update_discovered_gyms(rx: Res<GymBeaconRx>, mut gyms: ResMut<DiscoveredGyms>) {
    let now = Instant::now();
    let mut reorder = false;
    let rx = rx.0.lock();
    while let Ok((beacon, ip)) = rx.try_recv() {
        let existing = gyms
            .0
            .iter()
            .position(|g| g.beacon.instance == beacon.instance);
        // A gym on this machine is heard over loopback and often also on its LAN address; keep
        // the LAN address once known so the row does not flip between the two.
        let ip = match existing {
            Some(i) if ip.is_loopback() => gyms.0[i].ip,
            _ => ip,
        };
        match existing {
            Some(i) if gyms.0[i].ip == ip && gyms.0[i].beacon == beacon => {
                gyms.bypass_change_detection().0[i].last_seen = now;
            }
            Some(i) => {
                let gym = &mut gyms.0[i];
                gym.ip = ip;
                gym.beacon = beacon;
                gym.last_seen = now;
                reorder = true;
            }
            None => {
                gyms.0.push(DiscoveredGym {
                    ip,
                    beacon,
                    last_seen: now,
                });
                reorder = true;
            }
        }
    }
    if gyms.0.iter().any(|g| g.last_seen.elapsed() > GYM_EXPIRY) {
        gyms.0.retain(|g| g.last_seen.elapsed() <= GYM_EXPIRY);
    }
    if reorder {
        gyms.0
            .sort_by(|a, b| (&a.beacon.name, a.ip).cmp(&(&b.beacon.name, b.ip)));
    }
}
