//! The green flag at the end of a tutorial.
//!
//! Any living man that reaches it wins the tutorial for the whole instance: every client is told
//! (`TutorialComplete`), every character's units are saved to the authserver
//! (`/internal/v1/servers/tutorial-done`) and the server asks to shut down with
//! `tutorial_complete`, which the coordinator answers with `to_menu`. The world then sends everyone
//! back to the main menu and the instance is deleted, as for an idle one.

use bevy::prelude::*;

use shared::{
    event::client::TutorialComplete,
    net_components::{
        ToNetComponent,
        ents::{GoalFlag, Man},
        ours::{Dead, OwnedByPlayer},
    },
    netlib::{EventToClient, ServerNetworkingResources},
    physics::terrain::TerrainParams,
};

use crate::{
    ConnectedPlayer, PlayerEndpoint, PlayerId, PlayerSid, ServerState,
    coordinator::CoordinatorLink, unit_snapshot, world_not_paused,
};

/// How close a man's feet have to come to the flag, horizontally.
pub const FLAG_RADIUS: f32 = 3.0;
/// How long "You win!" stays up before the server sends everyone to the menu.
pub const WIN_BANNER_SECS: f32 = 5.0;
/// The pole's half height: the client draws it centred on the transform.
const FLAG_POLE_HALF_HEIGHT: f32 = 2.5;

/// Where the flag may stand, in order of preference. Players start near `(50, 50)` (the client's
/// default camera), so the first one is the far corner.
const FLAG_CANDIDATES: [(f32, f32); 5] = [
    (-60.0, -60.0),
    (-60.0, 60.0),
    (60.0, -60.0),
    (0.0, -70.0),
    (-70.0, 0.0),
];

pub struct TutorialPlugin;

impl Plugin for TutorialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TutorialState>()
            .add_systems(OnEnter(ServerState::Running), spawn_goal_flag)
            .add_systems(
                Update,
                (tick_tutorial_state, detect_flag_reached)
                    .chain()
                    .run_if(in_state(ServerState::Running))
                    .run_if(world_not_paused),
            );
    }
}

/// Set once a player reaches the flag: seconds since it happened.
#[derive(Resource, Default)]
pub struct TutorialState {
    pub completed_for: Option<f32>,
}

/// The first candidate that is not under water, standing on the ground.
pub fn flag_position(terrain: &TerrainParams) -> Vec3 {
    let dry = FLAG_CANDIDATES
        .iter()
        .copied()
        .find(|(x, z)| terrain.ground_height(*x, *z) > terrain.water_level() + 0.5);
    let (x, z) = dry.unwrap_or(FLAG_CANDIDATES[0]);
    Vec3::new(x, terrain.ground_height(x, z) + FLAG_POLE_HALF_HEIGHT, z)
}

/// Whether a unit is at the flag. Horizontal only: the man's feet and the pole's base are at
/// different heights.
pub fn reached(flag: Vec3, unit: Vec3) -> bool {
    let (dx, dz) = (flag.x - unit.x, flag.z - unit.z);
    dx * dx + dz * dz <= FLAG_RADIUS * FLAG_RADIUS
}

/// The flag is part of the world, so it is a unit like any other: a restored world already has one.
fn spawn_goal_flag(world: &World, terrain: Res<TerrainParams>, mut commands: Commands) {
    let existing = world
        .try_query_filtered::<Entity, With<GoalFlag>>()
        .map(|mut flags| flags.iter(world).count())
        .unwrap_or(0);
    if existing > 0 {
        return;
    }
    let at = flag_position(&terrain);
    info!(?at, "tutorial: placing the flag");
    shared::event::client::SpawnUnit2 {
        net_ent_id: shared::event::NetEntId::random(),
        components: vec![
            GoalFlag.to_net_component(),
            Transform::from_translation(at).to_net_component(),
        ],
    }
    .spawn_entity(&mut commands);
}

pub(crate) fn tick_tutorial_state(time: Res<Time>, mut tutorial: ResMut<TutorialState>) {
    if let Some(elapsed) = tutorial.completed_for.as_mut() {
        *elapsed += time.delta_secs();
    }
}

/// The first living man to reach the flag wins for everyone in the instance.
pub(crate) fn detect_flag_reached(
    tutorial: Res<TutorialState>,
    world: &World,
    mut commands: Commands,
    sr: Res<ServerNetworkingResources>,
    link: Option<Res<CoordinatorLink>>,
) {
    if tutorial.completed_for.is_some() {
        return;
    }
    let Some(flag) = world
        .try_query_filtered::<&Transform, With<GoalFlag>>()
        .and_then(|mut flags| flags.iter(world).map(|t| t.translation).next())
    else {
        return;
    };
    let Some((winner, _man)) = world
        .try_query_filtered::<(&OwnedByPlayer, &Transform), (With<Man>, Without<Dead>)>()
        .and_then(|mut men| {
            men.iter(world)
                .find(|(_, transform)| reached(flag, transform.translation))
                .map(|(owner, _)| (owner.player_id, ()))
        })
    else {
        return;
    };

    commands.insert_resource(TutorialState {
        completed_for: Some(0.0),
    });
    if let Some(mut clients) = world.try_query_filtered::<&PlayerEndpoint, With<ConnectedPlayer>>()
    {
        for endpoint in clients.iter(world) {
            sr.send_outgoing_event_now(
                endpoint.0,
                &EventToClient::TutorialComplete(TutorialComplete {}),
                None,
            );
        }
    }
    let dumps = character_dumps(world);
    if let Some(link) = link.as_deref() {
        link.tutorial_done(dumps.clone());
    }
    info!(
        winner = winner.0,
        characters = dumps.len(),
        "tutorial complete: the flag was reached"
    );
}

/// Every character in the world with a login session, connected or not: a ghost's units are as
/// much part of this tutorial as a connected player's.
fn character_dumps(world: &World) -> Vec<crate::coordinator::CharacterDump> {
    let mut dumps = Vec::new();
    let Some(mut players) = world.try_query::<(&PlayerId, &PlayerSid)>() else {
        return dumps;
    };
    let characters: Vec<i64> = players
        .iter(world)
        .map(|(player_id, _)| player_id.0 as i64)
        .collect();
    for character_id in characters {
        let mut units = Vec::new();
        if let Some(mut owned) = world.try_query::<(Entity, &OwnedByPlayer)>() {
            for (entity, owner) in owned.iter(world) {
                if owner.player_id.0 as i64 == character_id
                    && let Some(unit) = unit_snapshot(world, entity)
                {
                    units.push(unit);
                }
            }
        }
        dumps.push(crate::coordinator::CharacterDump {
            character_id,
            units,
        });
    }
    dumps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reached_is_horizontal() {
        let flag = Vec3::new(10.0, 5.0, 10.0);
        assert!(reached(flag, Vec3::new(10.0, 0.0, 10.0)));
        assert!(reached(flag, Vec3::new(12.9, 0.0, 10.0)));
        assert!(!reached(flag, Vec3::new(13.1, 0.0, 10.0)));
        assert!(!reached(flag, Vec3::new(10.0, 0.0, 13.1)));
    }

    /// The authserver stores the dump as JSONB: every unit in it must serialize on its own.
    #[test]
    fn character_dump_serializes_to_json() {
        use shared::net_components::{make_ball, make_man, ours::ControlledBy};

        let man = make_man(
            Transform::from_xyz(1.0, 2.0, 3.0),
            ControlledBy::single(PlayerId(7)),
            "TypeQ",
        );
        let ball = make_ball(
            Transform::from_xyz(1.0, 2.0, 3.0),
            Color::WHITE,
            ControlledBy::single(PlayerId(7)),
        );
        let dump = crate::coordinator::CharacterDump {
            character_id: 7,
            units: vec![man, ball],
        };
        let json = serde_json::to_value(&dump).expect("a dump is JSON");
        assert_eq!(json["character_id"], 7);
        assert_eq!(json["units"].as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn flag_is_never_placed_in_water() {
        let mut placed = 0;
        for seed in 0..50 {
            let terrain = TerrainParams {
                seed,
                ..TerrainParams::default()
            };
            let flag = flag_position(&terrain);
            let all_wet = FLAG_CANDIDATES
                .iter()
                .all(|(x, z)| terrain.ground_height(*x, *z) <= terrain.water_level() + 0.5);
            if all_wet {
                continue;
            }
            assert!(
                flag.y - FLAG_POLE_HALF_HEIGHT > terrain.water_level(),
                "seed {seed}: the flag is under water at {flag:?}"
            );
            assert!(
                flag.x.abs() < terrain.plane_size / 2.0 && flag.z.abs() < terrain.plane_size / 2.0,
                "seed {seed}: the flag is off the map at {flag:?}"
            );
            placed += 1;
        }
        assert!(placed > 0, "no seed had a dry candidate");
    }
}
