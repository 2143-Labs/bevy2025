//! A provisioned instance's own lifecycle: the shutdown sentinel, idle shutdown and SIGTERM.
//!
//! A managed server (one an authserver provisioner created, `ROTS_SERVER_GENERATION` set) decides
//! when it is done and tears itself down; the workflow that owns it deletes the leftovers. A
//! static server (prod `rots-gameserver-0..3`, local dev) has none of the env below, gets no
//! plugin and behaves exactly as before — except for the startup sentinel check, which needs
//! `ROTS_STATE_DIR` and so does nothing there.
//!
//! Why a sentinel file: a StatefulSet pod has `restartPolicy: Always`, so a server that exits is
//! restarted by the controller whether it wanted to be or not. A server writes its generation
//! into `<ROTS_STATE_DIR>/shutdown` before exiting 0, and a process that starts with a sentinel
//! holding *its own* generation exits immediately. A successor cloned from that volume has a
//! different generation, so it deletes the sentinel and runs.
//!
//! Two shutdowns, two shapes:
//! - idle (and any SIGTERM on an instance without `ROTS_STATE_DIR`): ask the coordinator for a
//!   plan, tell the clients to go back to the main menu, write the sentinel, exit. The world dies
//!   with the instance.
//! - SIGTERM with a statefile (slice B): pause the world, write the final snapshot, exit 0 with
//!   *no* sentinel and no shutdown request. The StatefulSet restarts the pod on the same volume and
//!   the world comes back paused until its players return (see [`crate::statefile`]).
//!
//! Slice C replaces the `ToMenu` SIGTERM with a handoff to a successor instance.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use bevy::time::Real;
use shared::{
    event::MigrationKind, event::TUTORIAL_COMPLETE, event::client::HostMigration,
    netlib::EventToClient, netlib::ServerNetworkingResources,
};
use tokio::sync::oneshot;

use crate::{
    ConnectedPlayer, GameServerCoordinator, PauseReason, Paused, PlayerEndpoint, PlayerId,
    ServerState,
    coordinator::{CoordinatorLink, ShutdownPlan},
    statefile::Statefile,
    tutorial::{TutorialState, WIN_BANNER_SECS},
};

/// The sentinel's name inside `ROTS_STATE_DIR`.
pub const SENTINEL: &str = "shutdown";

/// The `reason` a `ToMenu` migration carries (slice C distinguishes handoffs).
const SERVER_SHUTDOWN: &str = "server_shutdown";

/// How long clients get to leave after being told the server is going away.
///
/// Together with the 30 s `terminationGracePeriodSeconds` this has to fit inside the pod's grace
/// period, or the kubelet kills the process before it can write the sentinel.
pub const MIGRATION_GRACE: f32 = 10.0;

/// How long to wait for the coordinator's shutdown plan before falling back to `ToMenu`. An
/// authserver outage must not outlive the pod's grace period.
pub const PLAN_TIMEOUT: f32 = 15.0;

/// What a fresh process should do with the volume it was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Startup {
    /// No sentinel, or one from another generation: run the world.
    Run,
    /// This generation already shut down: exit 0.
    Exit,
}

/// Does `contents` (the sentinel file, as written by [`write_sentinel`]) say `generation` already
/// shut down?
pub fn sentinel_matches(contents: &str, generation: u64) -> bool {
    contents.trim() == generation.to_string()
}

/// The decision [`prepare`] makes, without touching the filesystem. `None` = no sentinel file.
pub fn startup(sentinel: Option<&str>, generation: u64) -> Startup {
    match sentinel {
        Some(contents) if sentinel_matches(contents, generation) => Startup::Exit,
        _ => Startup::Run,
    }
}

/// `ROTS_STATE_DIR`, unset or empty = this instance keeps no state on disk.
fn state_dir() -> Option<PathBuf> {
    std::env::var("ROTS_STATE_DIR")
        .ok()
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

/// Write the sentinel that tells a restart of *this* generation to stay down.
pub fn write_sentinel(state_dir: &Path, generation: u64) -> std::io::Result<()> {
    std::fs::write(state_dir.join(SENTINEL), format!("{generation}\n"))
}

/// Read the volume's sentinel, and delete a stale one so the next generation is not greeted by
/// its predecessor's. Never fails: an unreadable directory is a fresh start.
pub fn prepare(state_dir: &Path, generation: u64) -> Startup {
    let sentinel = std::fs::read_to_string(state_dir.join(SENTINEL)).ok();
    match startup(sentinel.as_deref(), generation) {
        Startup::Run => {
            if sentinel.is_some() {
                // A successor cloned from a sentinel-bearing volume: its own generation may run.
                info!("shutdown sentinel names another generation: removing it");
                if let Err(e) = std::fs::remove_file(state_dir.join(SENTINEL)) {
                    warn!("could not remove the stale shutdown sentinel: {e}");
                }
            }
            Startup::Run
        }
        Startup::Exit => Startup::Exit,
    }
}

/// Startup: exit 0 straight away when this generation has already shut down.
pub fn exit_if_already_shut_down(coordinator: &GameServerCoordinator) {
    if !coordinator.managed {
        return;
    }
    let Some(dir) = state_dir() else {
        return;
    };
    if prepare(&dir, coordinator.generation) == Startup::Exit {
        info!(
            generation = coordinator.generation,
            "generation already shut down: exiting"
        );
        std::process::exit(0);
    }
}

/// The coordinator does not know this instance any more (a heartbeat got `409 unknown_instance`):
/// nothing that happens here is reachable, so write the sentinel and exit, which stops the
/// StatefulSet from restarting a world nobody can join.
pub fn exit_orphaned(generation: u64) -> ! {
    warn!(
        generation,
        "the coordinator does not list this instance: shutting down"
    );
    if let Some(dir) = state_dir() {
        if let Err(e) = write_sentinel(&dir, generation) {
            error!("could not write the shutdown sentinel: {e}");
        }
    }
    std::process::exit(0)
}

/// Exit after the shutdown is complete: sentinel first, so the restart stays down.
fn exit_for(generation: u64, reason: Option<&'static str>) -> ! {
    info!(generation, reason = ?reason, "shutting down");
    if let Some(dir) = state_dir() {
        if let Err(e) = write_sentinel(&dir, generation) {
            error!("could not write the shutdown sentinel: {e}");
        }
    }
    std::process::exit(0)
}

/// Counts how long this server has had nobody connected, and says when it should destroy itself.
///
/// `connected` is live connections only: slice B's ghosts (characters left in the world by a
/// disconnected player) do not keep a server alive.
#[derive(Resource, Debug, Clone, Copy)]
pub struct IdleTimer {
    /// `None`, or 0: idle shutdown is off.
    idle_secs: Option<f32>,
    idle_for: f32,
}

impl IdleTimer {
    pub fn new(idle_secs: Option<f32>) -> Self {
        Self {
            idle_secs: idle_secs.filter(|secs| *secs > 0.0),
            idle_for: 0.0,
        }
    }

    /// One frame: `true` once the server has been empty for the configured time.
    pub fn tick(&mut self, connected: usize, delta: f32) -> bool {
        let Some(idle_secs) = self.idle_secs else {
            return false;
        };
        if connected > 0 {
            self.idle_for = 0.0;
            return false;
        }
        self.idle_for += delta.max(0.0);
        self.idle_for >= idle_secs
    }
}

/// What the shutdown path does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownStep {
    /// Nothing to do yet.
    Wait,
    /// Tell every connected client to go back to the main menu (once).
    Broadcast,
    /// Write the sentinel and exit 0.
    Exit,
}

/// The shutdown, from the moment the coordinator's plan is known.
#[derive(Debug, Default, Clone, Copy)]
pub struct ShutdownFlow {
    plan: Option<ShutdownPlan>,
    broadcast: bool,
    waited: f32,
}

impl ShutdownFlow {
    pub fn on_plan(&mut self, plan: ShutdownPlan) {
        self.plan = Some(plan);
    }

    pub fn plan(&self) -> Option<ShutdownPlan> {
        self.plan
    }

    /// One frame. `clients` is the number of players still connected.
    pub fn tick(&mut self, clients: usize, delta: f32) -> ShutdownStep {
        match self.plan {
            None => ShutdownStep::Wait,
            Some(ShutdownPlan::Destroy) => ShutdownStep::Exit,
            // Slice A has no way to carry a world to another instance, so a handoff it is told
            // about (slice C) becomes the same thing a `ToMenu` does.
            Some(ShutdownPlan::Migrate { .. } | ShutdownPlan::ToMenu) => {
                if clients == 0 {
                    return ShutdownStep::Exit;
                }
                if !self.broadcast {
                    self.broadcast = true;
                    return ShutdownStep::Broadcast;
                }
                self.waited += delta;
                if self.waited >= MIGRATION_GRACE {
                    info!("clients did not leave within {MIGRATION_GRACE} s: exiting anyway");
                    return ShutdownStep::Exit;
                }
                ShutdownStep::Wait
            }
        }
    }
}

/// A shutdown in progress: idle or SIGTERM. Both ask the coordinator what to do with the players
/// and then follow [`ShutdownFlow`].
#[derive(Resource, Default)]
pub struct Shutdown {
    reason: Option<&'static str>,
    /// The coordinator's answer, until it arrives.
    reply: Option<oneshot::Receiver<Result<ShutdownPlan, String>>>,
    /// How long the answer has been outstanding.
    waited: f32,
    flow: ShutdownFlow,
}

impl Shutdown {
    fn is_running(&self) -> bool {
        self.reason.is_some()
    }

    /// Start a shutdown. `false` when one is already under way (a second SIGTERM, say).
    fn ask(
        &mut self,
        reason: &'static str,
        reply: oneshot::Receiver<Result<ShutdownPlan, String>>,
    ) -> bool {
        if self.is_running() {
            return false;
        }
        self.reason = Some(reason);
        self.reply = Some(reply);
        true
    }

    /// One frame: collect the plan if it has arrived (or give up on it after `PLAN_TIMEOUT`) and
    /// take the next step.
    fn step(&mut self, delta: f32, clients: usize) -> ShutdownStep {
        if self.flow.plan().is_none() {
            let answer = self.reply.as_mut().and_then(|reply| reply.try_recv().ok());
            match answer {
                Some(Ok(plan)) => {
                    info!(
                        ?plan,
                        reason = ?self.reason,
                        "coordinator answered the shutdown request"
                    );
                    self.flow.on_plan(plan);
                    self.reply = None;
                }
                Some(Err(e)) => {
                    warn!("shutdown request failed ({e}): sending everyone to the menu");
                    self.flow.on_plan(ShutdownPlan::ToMenu);
                    self.reply = None;
                }
                None => {
                    self.waited += delta;
                    if self.waited >= PLAN_TIMEOUT {
                        warn!(
                            "no shutdown plan after {PLAN_TIMEOUT} s: sending everyone to the menu"
                        );
                        self.flow.on_plan(ShutdownPlan::ToMenu);
                        self.reply = None;
                    }
                }
            }
        }
        self.flow.tick(clients, delta)
    }
}

/// Set when the process receives SIGTERM. The tokio signal handler cannot touch the world, so it
/// only raises this and `on_sigterm` does the work.
#[derive(Resource, Clone, Default)]
pub struct Sigterm(Arc<AtomicBool>);

/// Watch for SIGTERM, so a pod being stopped migrates its players off instead of dropping them.
#[cfg(unix)]
fn spawn_sigterm_watcher(runtime: &tokio::runtime::Runtime) -> Sigterm {
    let sigterm = Sigterm::default();
    let flag = sigterm.0.clone();
    runtime.spawn(async move {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = match signal(SignalKind::terminate()) {
            Ok(terminate) => terminate,
            Err(e) => {
                error!("cannot listen for SIGTERM: {e}");
                return;
            }
        };
        while terminate.recv().await.is_some() {
            info!("SIGTERM: migrating players off before shutting down");
            flag.store(true, Ordering::Release);
        }
    });
    sigterm
}

#[cfg(not(unix))]
fn spawn_sigterm_watcher(_runtime: &tokio::runtime::Runtime) -> Sigterm {
    Sigterm::default()
}

/// The identity this instance was provisioned with.
#[derive(Resource, Clone)]
pub struct InstanceSettings {
    pub generation: u64,
}

/// Managed mode only: the server destroys itself when idle and migrates its players off when the
/// pod is stopped.
pub struct LifecyclePlugin {
    settings: InstanceSettings,
    idle: IdleTimer,
    sigterm: Sigterm,
    /// Provisioned into the warm pool: hold the world paused until the coordinator claims it.
    pre_game: bool,
}

impl LifecyclePlugin {
    pub fn new(coordinator: &GameServerCoordinator, runtime: &tokio::runtime::Runtime) -> Self {
        let idle_secs = crate::parse_env::<f32>("ROTS_IDLE_SECS");
        info!(
            generation = coordinator.generation,
            idle_secs, "instance lifecycle: idle shutdown and SIGTERM handling enabled"
        );
        Self {
            settings: InstanceSettings {
                generation: coordinator.generation,
            },
            idle: IdleTimer::new(idle_secs),
            sigterm: spawn_sigterm_watcher(runtime),
            pre_game: coordinator.pre_game,
        }
    }
}

impl Plugin for LifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.settings.clone())
            .insert_resource(self.idle)
            .insert_resource(self.sigterm.clone())
            .init_resource::<Shutdown>()
            .add_systems(
                Update,
                (
                    unpause_when_activated,
                    ask_when_idle,
                    ask_when_tutorial_complete,
                    ask_on_sigterm,
                    drive_shutdown,
                )
                    .chain()
                    .run_if(in_state(ServerState::Running)),
            );
        if self.pre_game {
            // After `Startup` (so a restored world's `Restoring` pause is already in place) and on
            // the same frame the world goes online.
            app.add_systems(OnEnter(ServerState::Running), pause_before_activation);
        }
    }
}

/// A world that has never been online waits for the coordinator's activation. A world restored
/// from its volume already paused itself (`Restoring`) for its players: leave it alone.
fn pause_before_activation(mut paused: ResMut<Paused>) {
    if paused.reason().is_none() {
        paused.pause(PauseReason::PreGame);
    }
}

/// The coordinator's `POST /activate`, or a player who got in first: either way the world is
/// wanted now.
fn unpause_when_activated(
    axum: Res<crate::axum::AxumServerResource>,
    mut paused: ResMut<Paused>,
    players: Query<(), With<ConnectedPlayer>>,
) {
    if paused.reason() != Some(PauseReason::PreGame) {
        return;
    }
    if axum.activated.load(Ordering::Relaxed) || !players.is_empty() {
        paused.unpause();
    }
}

/// Whether an empty world may count towards its idle shutdown. A restored world is waiting for
/// its players; a pre-game one has never had any.
fn idle_applies(paused: &Paused) -> bool {
    !matches!(
        paused.reason(),
        Some(PauseReason::Restoring | PauseReason::PreGame)
    )
}

/// Nobody connected for `ROTS_IDLE_SECS`: ask the coordinator to destroy this instance.
///
/// A restored world is exempt until it has finished restoring, and a pre-game one until it is
/// activated: both are supposed to have nobody connected, and idle counting only starts once the
/// world unpauses.
fn ask_when_idle(
    time: Res<Time<Real>>,
    mut idle: ResMut<IdleTimer>,
    players: Query<(), With<ConnectedPlayer>>,
    paused: Res<Paused>,
    link: Res<CoordinatorLink>,
    mut shutdown: ResMut<Shutdown>,
) {
    if shutdown.is_running() || !idle_applies(&paused) {
        return;
    }
    let connected = players.iter().count();
    if !idle.tick(connected, time.delta_secs()) {
        return;
    }
    info!("idle with nobody connected: destroying this instance");
    let reply = link.shutdown_request("idle", Vec::new());
    shutdown.ask("idle", reply);
}

/// The flag was reached and "You win!" has been up long enough: ask the coordinator to send
/// everyone back to the main menu. The dump of every character was queued before this, so it
/// lands first.
fn ask_when_tutorial_complete(
    tutorial: Option<Res<TutorialState>>,
    link: Res<CoordinatorLink>,
    players: Query<&PlayerId, With<ConnectedPlayer>>,
    mut shutdown: ResMut<Shutdown>,
) {
    let Some(tutorial) = tutorial else {
        return;
    };
    let Some(elapsed) = tutorial.completed_for else {
        return;
    };
    if elapsed < WIN_BANNER_SECS || shutdown.is_running() {
        return;
    }
    let players: Vec<i64> = players.iter().map(|id| id.0 as i64).collect();
    info!(
        players = players.len(),
        "tutorial complete: asking to send everyone to the menu"
    );
    let reply = link.shutdown_request(TUTORIAL_COMPLETE, players);
    shutdown.ask(TUTORIAL_COMPLETE, reply);
}

/// SIGTERM: either save the world for the restart, or ask the coordinator what to do with the
/// players on this server.
fn ask_on_sigterm(
    sigterm: Res<Sigterm>,
    settings: Res<InstanceSettings>,
    link: Res<CoordinatorLink>,
    players: Query<&PlayerId, With<ConnectedPlayer>>,
    statefile: Option<ResMut<Statefile>>,
    mut paused: ResMut<Paused>,
    mut shutdown: ResMut<Shutdown>,
) {
    if !sigterm.0.swap(false, Ordering::AcqRel) {
        return;
    }
    let players: Vec<i64> = players.iter().map(|id| id.0 as i64).collect();
    if let Some(mut statefile) = statefile {
        // The world outlives this process: no sentinel, no shutdown request, no migration. Take
        // the final snapshot and exit; the pod restarts on this volume and restores it.
        info!(
            players = players.len(),
            "SIGTERM: saving the world for the restart instead of migrating players"
        );
        // Nothing may move between the snapshot and the exit; the restarted world starts paused
        // anyway (`PauseReason::Restoring`).
        paused.pause(PauseReason::Restoring);
        crate::statefile::request_final_snapshot(&mut statefile);
        return;
    }
    let reply = link.shutdown_request("sigterm", players);
    if !shutdown.ask("sigterm", reply) {
        // A second SIGTERM means somebody wants this pod gone now. Still write the sentinel, so
        // the restart the StatefulSet performs stays down.
        warn!("second SIGTERM during a shutdown: exiting immediately");
        exit_for(settings.generation, Some("sigterm"));
    }
}

/// Follow the coordinator's plan: tell the clients, wait for them, exit.
fn drive_shutdown(
    time: Res<Time<Real>>,
    settings: Res<InstanceSettings>,
    mut shutdown: ResMut<Shutdown>,
    clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
    sr: Res<ServerNetworkingResources>,
) {
    if !shutdown.is_running() {
        return;
    }
    let connected = clients.iter().count();
    match shutdown.step(time.delta_secs(), connected) {
        ShutdownStep::Wait => {}
        ShutdownStep::Broadcast => {
            // A finished tutorial says so: the client shows "Tutorial complete" instead of
            // "The server shut down".
            let reason = if shutdown.reason == Some(TUTORIAL_COMPLETE) {
                TUTORIAL_COMPLETE
            } else {
                SERVER_SHUTDOWN
            };
            let event = EventToClient::HostMigration(HostMigration {
                kind: MigrationKind::ToMenu {
                    reason: reason.to_string(),
                },
            });
            info!(connected, "telling clients to go back to the menu");
            for endpoint in clients.iter() {
                sr.send_outgoing_event_now(endpoint.0, &event, None);
            }
        }
        ShutdownStep::Exit => exit_for(settings.generation, shutdown.reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinel_matches_only_its_own_generation() {
        assert!(sentinel_matches("7\n", 7));
        assert!(sentinel_matches("7", 7));
        assert!(!sentinel_matches("7\n", 8));
        assert!(!sentinel_matches("8\n", 7));
        assert!(!sentinel_matches("not a generation", 7));
        assert!(!sentinel_matches("", 7));
    }

    #[test]
    fn a_fresh_process_runs_unless_the_sentinel_names_it() {
        assert_eq!(startup(None, 3), Startup::Run);
        assert_eq!(startup(Some("2\n"), 3), Startup::Run);
        assert_eq!(startup(Some("3\n"), 3), Startup::Exit);
    }

    #[test]
    fn the_sentinel_round_trips_through_the_volume() {
        let dir = std::env::temp_dir().join(format!("rots-sentinel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_sentinel(&dir, 5).unwrap();
        assert_eq!(prepare(&dir, 5), Startup::Exit);
        // A successor (another generation) removes it and runs.
        assert_eq!(prepare(&dir, 6), Startup::Run);
        assert_eq!(prepare(&dir, 6), Startup::Run);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn idle_does_not_apply_before_activation() {
        let mut paused = Paused::default();
        assert!(idle_applies(&paused));
        paused.pause(PauseReason::PreGame);
        assert!(!idle_applies(&paused));
        paused.unpause();
        paused.pause(PauseReason::Restoring);
        assert!(!idle_applies(&paused));
    }

    #[test]
    fn idle_timer_needs_the_whole_quiet_period() {
        let mut idle = IdleTimer::new(Some(60.0));
        assert!(!idle.tick(0, 59.0));
        assert!(idle.tick(0, 1.0));
    }

    #[test]
    fn idle_timer_resets_when_somebody_connects() {
        let mut idle = IdleTimer::new(Some(60.0));
        assert!(!idle.tick(0, 30.0));
        assert!(!idle.tick(1, 0.1));
        assert!(!idle.tick(0, 30.0));
        assert!(!idle.tick(0, 29.0));
        assert!(idle.tick(0, 1.0));
    }

    #[test]
    fn idle_timer_is_off_without_a_rule() {
        let mut idle = IdleTimer::new(None);
        assert!(!idle.tick(0, 100_000.0));
        let mut zero = IdleTimer::new(Some(0.0));
        assert!(!zero.tick(0, 100_000.0));
    }

    #[test]
    fn sigterm_broadcasts_once_then_waits_for_the_clients() {
        let mut flow = ShutdownFlow::default();
        assert_eq!(flow.tick(2, 0.1), ShutdownStep::Wait);
        flow.on_plan(ShutdownPlan::ToMenu);
        assert_eq!(flow.tick(2, 0.1), ShutdownStep::Broadcast);
        assert_eq!(flow.tick(2, 1.0), ShutdownStep::Wait);
        assert_eq!(flow.tick(1, 1.0), ShutdownStep::Wait);
        // Everyone left before the grace period ran out.
        assert_eq!(flow.tick(0, 0.1), ShutdownStep::Exit);
    }

    #[test]
    fn sigterm_exits_when_the_grace_period_runs_out() {
        let mut flow = ShutdownFlow::default();
        flow.on_plan(ShutdownPlan::ToMenu);
        assert_eq!(flow.tick(1, 0.1), ShutdownStep::Broadcast);
        assert_eq!(flow.tick(1, MIGRATION_GRACE - 0.1), ShutdownStep::Wait);
        assert_eq!(flow.tick(1, 0.2), ShutdownStep::Exit);
    }

    #[test]
    fn a_destroy_plan_exits_without_talking_to_anyone() {
        let mut flow = ShutdownFlow::default();
        flow.on_plan(ShutdownPlan::Destroy);
        assert_eq!(flow.tick(3, 0.1), ShutdownStep::Exit);
    }

    #[test]
    fn a_handoff_to_another_instance_falls_back_to_the_menu() {
        let mut flow = ShutdownFlow::default();
        flow.on_plan(ShutdownPlan::Migrate {
            slot: 4,
            generation: 9,
        });
        assert_eq!(flow.tick(1, 0.1), ShutdownStep::Broadcast);
    }
}
