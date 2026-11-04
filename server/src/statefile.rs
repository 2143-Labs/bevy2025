//! The world's statefile: everything needed to put a world back after the process running it died.
//!
//! A managed instance (`ROTS_STATE_DIR` set) writes `state.bin` every few seconds and once more on
//! SIGTERM. A fresh process on the same volume loads it and carries on: units come back with their
//! `NetEntId`s, every saved player comes back as a ghost (`DisconnectedPlayer`, keeping their
//! camera unit), and the world is paused (`PauseReason::Restoring`) until everybody is back or the
//! grace period runs out. That is what makes a forced failure (a hard-killed pod) survivable: the
//! StatefulSet restarts the pod on the same PVC and the world resumes where it was.
//!
//! The file is a postcard-encoded [`WorldSnapshot`], written as `state.bin.tmp`, fsynced and
//! renamed, so a process killed mid-write leaves the previous snapshot intact. An unreadable file,
//! or one written by another version, starts a fresh world instead of crash-looping the pod.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::time::Real;
use serde::{Deserialize, Serialize};
use shared::{
    CurrentTick,
    event::{PlayerId, client::SpawnUnit2},
    net_components::ours::{PlayerColor, PlayerName},
    physics::terrain::TerrainParams,
};

use crate::{
    ConnectedPlayer, DisconnectedPlayer, KeepGhosts, PauseReason, Paused, PlayerSid, ServerState,
};

/// The snapshot format this server writes. A file with any other version is unusable.
pub const VERSION: u16 = 1;

/// The snapshot inside `ROTS_STATE_DIR`.
pub const FILE: &str = "state.bin";
/// The snapshot under construction; renamed onto [`FILE`] once it is on disk.
pub const TMP: &str = "state.bin.tmp";

/// How often a live world is written.
pub const SNAPSHOT_EVERY_SECS: f32 = 5.0;

/// How long a restored world stays paused waiting for its players to come back. After that the
/// absent characters are live and killable like any other.
pub const RESTORE_GRACE_SECS: f32 = 30.0;

/// One character as the statefile remembers them: enough to spawn the player entity again as a
/// ghost, which is what `admit_players` rebinds when the player reconnects.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SavedPlayer {
    pub player_id: PlayerId,
    pub name: String,
    pub color: PlayerColor,
    /// The login session the character was playing with; `None` for an unverified connection
    /// (testing gym).
    pub sid: Option<String>,
}

/// A world, as one file. Units are captured with the same per-entity component walk a client's
/// [`shared::event::client::WorldData2`] uses (see [`crate::unit_snapshot`]).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorldSnapshot {
    pub version: u16,
    pub tick: u64,
    pub terrain: TerrainParams,
    /// Whether the world was paused when the snapshot was taken.
    pub paused: bool,
    pub units: Vec<SpawnUnit2>,
    pub players: Vec<SavedPlayer>,
}

impl WorldSnapshot {
    /// A world with nothing in it, used by the tests.
    #[cfg(test)]
    pub fn empty(terrain: TerrainParams) -> Self {
        Self {
            version: VERSION,
            tick: 0,
            terrain,
            paused: false,
            units: vec![],
            players: vec![],
        }
    }
}

/// Encode a snapshot for the volume.
pub fn encode(snapshot: &WorldSnapshot) -> Result<Vec<u8>, String> {
    postcard::to_stdvec(snapshot).map_err(|e| format!("cannot encode the statefile: {e}"))
}

/// Decode a snapshot, rejecting one this server cannot read.
pub fn decode(bytes: &[u8]) -> Result<WorldSnapshot, String> {
    let snapshot: WorldSnapshot =
        postcard::from_bytes(bytes).map_err(|e| format!("cannot decode the statefile: {e}"))?;
    if snapshot.version != VERSION {
        return Err(format!(
            "statefile version {} (this server writes {VERSION})",
            snapshot.version
        ));
    }
    Ok(snapshot)
}

/// Write `snapshot` to `dir` atomically: a half-written file is never the one a restart reads.
pub fn write(dir: &Path, snapshot: &WorldSnapshot) -> Result<(), String> {
    let bytes = encode(snapshot)?;
    let tmp = dir.join(TMP);
    let path = dir.join(FILE);
    let mut file =
        std::fs::File::create(&tmp).map_err(|e| format!("cannot create {}: {e}", tmp.display()))?;
    file.write_all(&bytes)
        .map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    file.sync_all()
        .map_err(|e| format!("cannot fsync {}: {e}", tmp.display()))?;
    drop(file);
    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("cannot rename {} to {}: {e}", tmp.display(), path.display()))
}

/// What a fresh process found on its volume.
#[derive(Debug)]
pub enum Loaded {
    /// No statefile: a brand-new instance.
    None,
    /// A usable snapshot.
    Snapshot(Box<WorldSnapshot>),
    /// Something is there but cannot be used; the reason goes to the log.
    Unusable(String),
}

/// Read the volume's snapshot. Never fails: a missing file is a fresh world and a broken one is
/// reported for the caller to log and replace.
pub fn load(dir: &Path) -> Loaded {
    let path = dir.join(FILE);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Loaded::None,
        Err(e) => return Loaded::Unusable(format!("cannot read {}: {e}", path.display())),
    };
    match decode(&bytes) {
        Ok(snapshot) => Loaded::Snapshot(Box::new(snapshot)),
        Err(e) => Loaded::Unusable(e),
    }
}

/// `ROTS_STATE_DIR`, unset or empty: this instance keeps no state on disk (static prod, local dev).
fn state_dir() -> Option<PathBuf> {
    std::env::var("ROTS_STATE_DIR")
        .ok()
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

/// Capture the whole world. Mirrors what a client is sent when it joins, plus the players who are
/// not connected (ghosts) and the tick.
pub fn capture(world: &World) -> WorldSnapshot {
    let tick = world.resource::<CurrentTick>().0.0;
    let terrain = world.resource::<TerrainParams>().clone();
    let paused = world
        .get_resource::<Paused>()
        .is_some_and(|paused| paused.is_paused());

    let mut units = vec![];
    if let Some(mut world_units) = world.try_query_filtered::<Entity, ()>() {
        for entity in world_units.iter(world) {
            // Only units are networked: a player entity is rebuilt from `players`, not from here.
            if world.get::<shared::event::NetEntId>(entity).is_none() {
                continue;
            }
            if let Some(unit) = crate::unit_snapshot(world, entity) {
                units.push(unit);
            }
        }
    }

    let mut players = vec![];
    if let Some(mut ghosts) = world.try_query_filtered::<
        (&PlayerId, &PlayerName, &PlayerColor, Option<&PlayerSid>),
        Or<(With<ConnectedPlayer>, With<DisconnectedPlayer>)>,
    >() {
        for (player_id, name, color, sid) in ghosts.iter(world) {
            players.push(SavedPlayer {
                player_id: *player_id,
                name: name.name.clone(),
                color: color.clone(),
                sid: sid.map(|sid| sid.0.clone()),
            });
        }
    }
    players.sort_by_key(|player| player.player_id.0);

    WorldSnapshot {
        version: VERSION,
        tick,
        terrain,
        paused,
        units,
        players,
    }
}

/// Does a restored world stop waiting? Everybody is back, or the grace period is over.
fn should_unpause(
    expected: &BTreeSet<PlayerId>,
    connected: &BTreeSet<PlayerId>,
    elapsed: f32,
) -> bool {
    elapsed >= RESTORE_GRACE_SECS || expected.iter().all(|id| connected.contains(id))
}

/// A restored world's wait for its players.
#[derive(Resource, Debug, Default)]
struct Restoring {
    running: bool,
    expected: BTreeSet<PlayerId>,
    elapsed: f32,
}

/// The snapshot a fresh process found, waiting for the world to exist.
#[derive(Resource, Default)]
struct RestoredWorld(Option<WorldSnapshot>);

/// The volume's statefile, and when it was last written.
#[derive(Resource, Debug)]
pub struct Statefile {
    dir: PathBuf,
    since_write: f32,
    /// SIGTERM: the world is going away, take the final snapshot and exit. Set by the shutdown
    /// path, acted on by [`write_pending`].
    final_requested: bool,
    /// Set once the final snapshot has been written: nothing may overwrite it.
    frozen: bool,
}

impl Statefile {
    /// Do not overwrite the snapshot again (a restart must read the shutdown state).
    fn freeze(&mut self) {
        self.frozen = true;
    }
}

/// The snapshot a `&World` system took, waiting for a writer.
///
/// Capturing and writing cannot be one system: `&World` may not be combined with `ResMut`, and the
/// writer needs to own [`Statefile`]. This slot is the seam between them.
#[derive(Resource, Default)]
struct PendingSnapshot(std::sync::Mutex<Option<WorldSnapshot>>);

/// The statefile, for instances that keep their world on a volume.
pub struct StatefilePlugin {
    dir: PathBuf,
}

impl StatefilePlugin {
    /// `None` when `ROTS_STATE_DIR` is not set: a static server keeps no state.
    pub fn from_env() -> Option<Self> {
        Some(Self::new(state_dir()?))
    }

    /// The plugin for a given volume, which is what the tests use.
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl Plugin for StatefilePlugin {
    fn build(&self, app: &mut App) {
        let restored = match load(&self.dir) {
            Loaded::None => {
                info!(dir = %self.dir.display(), "no statefile: starting a fresh world");
                None
            }
            Loaded::Snapshot(snapshot) => {
                info!(
                    dir = %self.dir.display(),
                    tick = snapshot.tick,
                    units = snapshot.units.len(),
                    players = snapshot.players.len(),
                    "statefile: restoring the world"
                );
                // The terrain is part of the world: a restored world must not grow a new one.
                app.insert_resource(snapshot.terrain.clone());
                Some(*snapshot)
            }
            Loaded::Unusable(reason) => {
                error!("statefile unusable, starting a fresh world: {reason}");
                None
            }
        };

        if let Err(e) = std::fs::create_dir_all(&self.dir) {
            error!("cannot create the state directory: {e}");
        }

        app.insert_resource(RestoredWorld(restored))
            // A world on a volume outlives a disconnect: its characters stay in it as ghosts.
            .insert_resource(KeepGhosts)
            .insert_resource(Statefile {
                dir: self.dir.clone(),
                since_write: 0.0,
                final_requested: false,
                frozen: false,
            })
            .init_resource::<Restoring>()
            .insert_resource(PendingSnapshot::default())
            .add_systems(Startup, restore_world)
            .add_systems(
                Update,
                (
                    tick_write_timer,
                    unpause_when_restored,
                    capture_pending,
                    write_pending,
                )
                    .chain()
                    .run_if(in_state(ServerState::Running)),
            );
    }
}

/// Startup: bring back the units, then the players as ghosts, then pause the world until they
/// return.
fn restore_world(
    restored: Res<RestoredWorld>,
    mut commands: Commands,
    mut tick: ResMut<CurrentTick>,
    mut paused: ResMut<Paused>,
    mut restoring: ResMut<Restoring>,
) {
    let Some(snapshot) = &restored.0 else {
        return;
    };

    tick.0 = shared::netlib::Tick(snapshot.tick);
    for unit in &snapshot.units {
        unit.clone().spawn_entity(&mut commands);
    }
    for player in &snapshot.players {
        let mut ghost = commands.spawn((
            PlayerName {
                name: player.name.clone(),
            },
            player.color.clone(),
            player.player_id,
            DisconnectedPlayer {
                disconnect_tick: tick.0,
            },
        ));
        if let Some(sid) = &player.sid {
            ghost.insert(PlayerSid(sid.clone()));
        }
    }

    restoring.running = true;
    restoring.expected = snapshot
        .players
        .iter()
        .map(|player| player.player_id)
        .collect();
    restoring.elapsed = 0.0;
    paused.pause(PauseReason::Restoring);
    info!(
        players = restoring.expected.len(),
        "restored world paused until its players are back"
    );
}

/// The world unpauses when every restored player is connected again, or after
/// [`RESTORE_GRACE_SECS`] — after which an absent character is live and killable like any other.
fn unpause_when_restored(
    time: Res<Time<Real>>,
    mut restoring: ResMut<Restoring>,
    mut paused: ResMut<Paused>,
    connected: Query<&PlayerId, With<ConnectedPlayer>>,
) {
    if !restoring.running {
        return;
    }
    restoring.elapsed += time.delta_secs();
    let back: BTreeSet<PlayerId> = connected.iter().copied().collect();
    if !should_unpause(&restoring.expected, &back, restoring.elapsed) {
        return;
    }
    info!(
        waited = restoring.elapsed,
        back = back.len(),
        "restored world resuming"
    );
    restoring.running = false;
    paused.unpause();
}

/// Advance the write clock. Its own system because [`capture_pending`] takes `&World`, which may not
/// be combined with `ResMut<Statefile>`.
fn tick_write_timer(time: Res<Time<Real>>, mut statefile: ResMut<Statefile>) {
    if statefile.frozen {
        return;
    }
    statefile.since_write += time.delta_secs();
}

/// Take the world once the interval has passed, or at once when SIGTERM asked for the final
/// snapshot.
fn capture_pending(
    paused: Res<Paused>,
    world: &World,
    statefile: Res<Statefile>,
    pending: Res<PendingSnapshot>,
) {
    // A pre-game world has never been online: a snapshot of it would be empty, and a restart
    // would restore it as a phantom world instead of re-entering pre-game.
    if paused.reason() == Some(PauseReason::PreGame) {
        return;
    }
    if statefile.frozen {
        return;
    }
    if !statefile.final_requested && statefile.since_write < SNAPSHOT_EVERY_SECS {
        return;
    }
    *pending.0.lock().expect("snapshot slot") = Some(capture(world));
}

/// Put the snapshot on the volume. `statefile.final_requested` means this is the shutdown write:
/// freeze it and exit, which the StatefulSet turns into a restart on the same volume.
fn write_pending(mut statefile: ResMut<Statefile>, pending: ResMut<PendingSnapshot>) {
    let Some(snapshot) = pending.0.lock().expect("snapshot slot").take() else {
        return;
    };
    let final_snapshot = statefile.final_requested;
    statefile.since_write = 0.0;
    match write(&statefile.dir, &snapshot) {
        Ok(()) => info!(
            units = snapshot.units.len(),
            players = snapshot.players.len(),
            tick = snapshot.tick,
            "wrote the statefile"
        ),
        Err(e) => error!("could not write the statefile: {e}"),
    }
    if final_snapshot {
        // No sentinel: the restart has to come back, on this volume, with this world.
        statefile.freeze();
        info!("SIGTERM: world saved, exiting for the restart");
        std::process::exit(0);
    }
}

/// SIGTERM with a statefile: the world outlives the process, so take the final snapshot instead of
/// migrating the players off. Called by the shutdown path.
pub fn request_final_snapshot(statefile: &mut Statefile) {
    statefile.final_requested = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::net_components::ToNetComponent;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rots-statefile-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn player(id: u64, sid: Option<&str>) -> SavedPlayer {
        SavedPlayer {
            player_id: PlayerId(id),
            name: format!("player {id}"),
            color: PlayerColor {
                hue: id as f32 * 10.0,
            },
            sid: sid.map(str::to_string),
        }
    }

    fn unit(id: u64) -> SpawnUnit2 {
        SpawnUnit2 {
            net_ent_id: shared::event::NetEntId(id),
            components: vec![
                PlayerName {
                    name: "unit".to_string(),
                }
                .to_net_component(),
                PlayerColor { hue: 12.5 }.to_net_component(),
                shared::net_components::ents::PlayerCamera.to_net_component(),
            ],
        }
    }

    #[test]
    fn statefile_round_trip() {
        let dir = temp_dir("round-trip");
        let mut snapshot = WorldSnapshot::empty(TerrainParams::gym());
        snapshot.tick = 4321;
        snapshot.paused = true;
        snapshot.units = vec![unit(1), unit(2)];
        snapshot.players = vec![player(7, Some("0123456789abcdef")), player(8, None)];

        write(&dir, &snapshot).expect("write");
        let loaded = match load(&dir) {
            Loaded::Snapshot(snapshot) => snapshot,
            other => panic!("expected a snapshot, got {other:?}"),
        };

        // The encoding is the comparison: a `SpawnUnit2` holds components that are not `PartialEq`
        // (colliders, transforms), and equal bytes are exactly what "equal" has to mean here.
        assert_eq!(
            encode(&snapshot).unwrap(),
            encode(&loaded).unwrap(),
            "loaded snapshot differs"
        );
        assert_eq!(loaded.version, VERSION);
        assert_eq!(loaded.tick, 4321);
        assert!(loaded.paused);
        assert_eq!(loaded.units.len(), 2);
        assert_eq!(loaded.units[1].net_ent_id, shared::event::NetEntId(2));
        assert_eq!(loaded.players.len(), 2);
        assert_eq!(loaded.players[0].player_id, PlayerId(7));
        assert_eq!(loaded.players[0].sid.as_deref(), Some("0123456789abcdef"));
        assert_eq!(loaded.players[1].sid, None);
        assert_eq!(loaded.players[1].color, PlayerColor { hue: 80.0 });

        // The temporary file is gone: only the rename's target remains.
        assert!(!dir.join(TMP).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_corrupt_statefile_is_unusable_rather_than_fatal() {
        let dir = temp_dir("corrupt");
        std::fs::write(dir.join(FILE), b"this is not a snapshot").unwrap();
        match load(&dir) {
            Loaded::Unusable(reason) => assert!(!reason.is_empty()),
            other => panic!("expected an unusable statefile, got {other:?}"),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_statefile_from_another_version_is_unusable() {
        let dir = temp_dir("version");
        let mut snapshot = WorldSnapshot::empty(TerrainParams::default());
        snapshot.version = VERSION + 1;
        std::fs::write(dir.join(FILE), encode(&snapshot).unwrap()).unwrap();
        match load(&dir) {
            Loaded::Unusable(reason) => assert!(reason.contains("version"), "{reason}"),
            other => panic!("expected an unusable statefile, got {other:?}"),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The write clock is what makes the file periodic; a world that never writes has nothing to
    /// restore from.
    #[test]
    fn the_world_is_written_every_five_seconds() {
        let dir = temp_dir("periodic");
        let mut app = App::new();
        // `App::new()` has no state machinery of its own: without this, `ServerState` never
        // reaches `Running` and nothing below sends a heartbeat or writes the world.
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<ServerState>()
            .insert_resource(Time::<Real>::default())
            .insert_resource(CurrentTick(shared::netlib::Tick(0)))
            .init_resource::<Paused>()
            .insert_resource(TerrainParams::default())
            .add_plugins(StatefilePlugin::new(dir.clone()));
        app.world_mut()
            .resource_mut::<NextState<ServerState>>()
            .set(ServerState::Running);
        app.update();
        assert!(
            !dir.join(FILE).exists(),
            "nothing is written before the first interval"
        );

        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_secs(4));
        app.update();
        assert!(
            !dir.join(FILE).exists(),
            "four seconds is not the interval yet"
        );

        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_secs(2));
        app.update();
        assert!(
            matches!(load(&dir), Loaded::Snapshot(_)),
            "the world goes to the volume once the interval passes"
        );
        assert!(
            !dir.join(TMP).exists(),
            "the temporary file is renamed away"
        );

        // And it keeps writing: the next interval replaces it.
        app.world_mut().resource_mut::<CurrentTick>().0.increment();
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_secs(6));
        app.update();
        match load(&dir) {
            Loaded::Snapshot(snapshot) => {
                assert_eq!(snapshot.tick, 1, "the second write is current")
            }
            other => panic!("expected a snapshot, got {other:?}"),
        }

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_statefile_is_a_fresh_world() {
        let dir = temp_dir("missing");
        assert!(matches!(load(&dir), Loaded::None));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn set(ids: impl IntoIterator<Item = u64>) -> BTreeSet<PlayerId> {
        ids.into_iter().map(PlayerId).collect()
    }

    #[test]
    fn a_restored_world_unpauses_when_everybody_is_back() {
        let expected = set([1, 2]);
        assert!(!should_unpause(&expected, &set([1]), 1.0));
        assert!(should_unpause(&expected, &set([1, 2]), 1.0));
        // An extra player does not keep it paused.
        assert!(should_unpause(&expected, &set([1, 2, 3]), 1.0));
    }

    #[test]
    fn a_restored_world_unpauses_after_the_grace_period() {
        let expected = set([1, 2]);
        assert!(!should_unpause(
            &expected,
            &set([]),
            RESTORE_GRACE_SECS - 0.1
        ));
        assert!(should_unpause(&expected, &set([]), RESTORE_GRACE_SECS));
    }

    #[test]
    fn a_restored_world_with_nobody_in_it_unpauses_at_once() {
        assert!(should_unpause(&set([]), &set([]), 0.0));
    }
}
