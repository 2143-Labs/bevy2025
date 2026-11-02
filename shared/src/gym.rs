//! LAN testing gym: ports, build hash and the discovery beacon wire format.
use serde::{Deserialize, Serialize};

/// UDP (game) and TCP (WebSocket) port of a client-hosted gym; axum health uses +1 (2144).
pub const GYM_GAME_PORT: u16 = 2143;
/// UDP port gyms broadcast their beacon to and testing clients listen on.
pub const GYM_DISCOVERY_PORT: u16 = 2145;
/// Git commit this binary was built from (12 hex chars), or "unknown".
pub const BUILD_HASH: &str = env!("ROTS_BUILD_HASH");

const BEACON_MAGIC: &[u8; 8] = b"ROTSGYM1";
const MAX_BEACON_LEN: usize = 1024;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GymBeacon {
    /// Random per hosted server; identifies a gym across beacons.
    pub instance: u64,
    pub build: String,
    pub name: String,
    pub game_port: u16,
    pub players: u32,
}

impl GymBeacon {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = BEACON_MAGIC.to_vec();
        out.extend(serde_json::to_vec(self).expect("GymBeacon serializes"));
        out
    }

    /// `None` for anything that is not a well-formed beacon (other programs share the LAN).
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_BEACON_LEN {
            return None;
        }
        serde_json::from_slice(bytes.strip_prefix(BEACON_MAGIC.as_slice())?).ok()
    }

    /// Same commit as this binary. Unknown builds never count as the same.
    pub fn same_build_as_local(&self) -> bool {
        BUILD_HASH != "unknown" && self.build == BUILD_HASH
    }
}
