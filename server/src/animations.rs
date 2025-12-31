use bevy::prelude::*;
use shared::{
    event::{server::CastSkillUpdate, NetEntId, PlayerId, UDPacketEvent},
    net_components::{ents::SendNetworkTranformUpdates, ours::ControlledBy},
    netlib::{send_outgoing_event_next_tick, ServerNetworkingResources},
    skills::animations::{SharedAnimationPlugin, UsingSkillSince},
    CurrentTick,
};

use crate::{ConnectedPlayer, EndpointToPlayerId, PlayerEndpoint};

pub struct AnimationPluginServer;

impl Plugin for AnimationPluginServer {
    fn build(&self, app: &mut App) {
        app.add_plugins(SharedAnimationPlugin)
            .add_systems(Update, on_unit_begin_skill_use);
    }
}

// Lets of TODO here
fn on_unit_begin_skill_use(
    mut skill_change: UDPacketEvent<CastSkillUpdate>,
    current_tick: Res<shared::CurrentTick>,
    mut our_unit: Query<
        (
            &NetEntId,
            Entity,
            Option<&mut UsingSkillSince>,
            &ControlledBy,
        ),
        With<SendNetworkTranformUpdates>,
    >,
    sr: Res<ServerNetworkingResources>,
    endpont_to_player: Res<EndpointToPlayerId>,
    time: Res<Time>,
    tick: Res<CurrentTick>,
    clients: Query<(&PlayerEndpoint, &PlayerId), With<ConnectedPlayer>>,
    mut commands: Commands,
) {
    for packet in skill_change.read() {
        let Some(player_id) = endpont_to_player.map.get(&packet.endpoint) else {
            warn!(
                "Received CastSkillUpdate from unknown endpoint: {:?}",
                packet.endpoint
            );
            continue;
        };

        let player_id = *player_id.value();

        let mut cancelled = false;
        //let mut event;
        for (ent_id, entity, maybe_existing_skill, controlled_by) in &mut our_unit {
            if packet.event.net_ent_id != *ent_id {
                continue;
            }

            if !controlled_by.players.contains(&player_id) {
                warn!(
                    ?player_id,
                    ?ent_id,
                    "Player tried to cast skill on unit they do not control"
                );
                cancelled = true;
                continue;
            }

            let new_using_skill = UsingSkillSince {
                real_time: time.elapsed_secs_f64(),
                tick: tick.0,
                skill: packet.event.skill.clone(),
            };

            if let Some(mut existing_cast) = maybe_existing_skill {
                #[allow(clippy::collapsible_if)]
                if existing_cast.skill == packet.event.skill {
                    if packet.event.begin_casting {
                        // Already using this skill, no need to do anything
                        info!(
                            ?player_id,
                            ?ent_id,
                            "Player tried to begin casting a skill they are already casting"
                        );
                        break;
                    }
                }
                // Stopping the current skill
                info!(?player_id, ?ent_id, "Stopping skill casting early");
                *existing_cast = new_using_skill;
                // send update to clients as update_entity
                // TODO
            } else {
                info!(?player_id, ?ent_id, "Beginning skill casting");
                commands.entity(entity).insert(new_using_skill);
            }
        }

        // TODO verify cast and unit and control and and and
        if cancelled {
            // to tell the unit to stop casting, we can send them a reply event like this
            //let our_event = shared::netlib::EventToClient::CastSkillUpdateToClient(
            //shared::event::client::CastSkillUpdateToClient {
            //net_ent_id: packet.event.net_ent_id,
            //begin_casting: false,
            //skill: packet.event.skill.clone(),
            //},
            //);
            //send_outgoing_event_next_tick(&sr, packet.endpoint, &our_event);
        }

        let event_to_send = shared::netlib::EventToClient::CastSkillUpdateToClient(
            shared::event::client::CastSkillUpdateToClient {
                net_ent_id: packet.event.net_ent_id,
                begin_casting: packet.event.begin_casting && !cancelled,
                skill: packet.event.skill.clone(),
                begin_casting_tick: current_tick.0,
            },
        );
        for (client_endpoint, _client_player_id) in clients {
            if client_endpoint.0 == packet.endpoint {
                // Don't send back to the original sender
                continue;
            }
            send_outgoing_event_next_tick(&sr, client_endpoint.0, &event_to_send);
        }
    }
}
