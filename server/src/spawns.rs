use bevy::prelude::*;
use shared::{
    event::{server::SpawnCircle, NetEntId},
    netlib::{send_event_to_server, EventToClient, ServerNetworkingResources},
};

use crate::{make_ball, ConnectedPlayer, HasColor, PlayerEndpoint, ServerState};

pub struct SpawnPlugin;
impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SpawnCircle>().add_systems(
            Update,
            (on_circle_spawn)
                //.run_if(on_timer(Duration::from_millis(10)))
                .run_if(in_state(ServerState::Running)),
        );
        //.add_systems(
        //Update,
        //(send_networked_Spawn_move)
        //.run_if(in_state(ServerState::Running))
        //.run_if(on_timer(Duration::from_millis(50))),
        //);
    }
}

fn on_circle_spawn(
    mut spawns: MessageReader<SpawnCircle>,
    mut commands: Commands,
    sr: Res<ServerNetworkingResources>,
    clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
) {
    for spawn in spawns.read() {
        let transform = Transform::from_translation(spawn.position);
        let ent_id = NetEntId::random();

        let unit = make_ball(ent_id, transform, spawn.color);
        let unit_ent = unit.clone().spawn_entity_srv(&mut commands);
        commands.entity(unit_ent).insert(HasColor(spawn.color));

        // Notify all clients about the new unit
        let event = EventToClient::SpawnUnit2(unit);
        for endpoint in &clients {
            send_event_to_server(&sr.handler, endpoint.0, &event);
        }
    }
}

//fn send_networked_Spawn_move(
//Spawns: Query<
//(&Transform, &MovementIntention, &AttackIntention, &NetEntId),
//(
//With<AIType>,
//Or<(
//Changed<Transform>,
//Changed<MovementIntention>,
//Changed<AttackIntention>,
//)>,
//),
//>,
//clients: Query<&PlayerEndpoint, With<ConnectedPlayerName>>,
//sr: Res<ServerResources<EventToServer>>,
//) {
//let mut all_events = vec![];
//for (&movement, mi, attack_intent, &id) in &Spawns {
//all_events.extend([
//EventToClient::SomeoneMoved(SomeoneMoved {
//id,
//movement: shared::event::server::ChangeMovement::SetTransform(movement),
//}),
//EventToClient::SomeoneMoved(SomeoneMoved {
//id,
//movement: shared::event::server::ChangeMovement::Move2d(mi.0),
//}),
//EventToClient::SomeoneMoved(SomeoneMoved {
//id,
//movement: shared::event::server::ChangeMovement::AttackIntent(
//attack_intent.clone(),
//),
//}),
//]);
//}

//if !all_events.is_empty() {
//for event_list in all_events.chunks(250) {
//for endpoint in &clients {
//send_event_to_server_batch(&sr.handler, endpoint.0, event_list);
//}
//}
//}
//}
