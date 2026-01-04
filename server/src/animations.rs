use bevy::prelude::*;
use shared::{
    CurrentTick, event::{NetEntId, PlayerId, UDPacketEvent, client::SpawnProjectile, server::CastSkillUpdate}, net_components::{ents::SendNetworkTranformUpdates, make_npc, ours::ControlledBy}, netlib::ServerNetworkingResources, projectile::{ProjectileAI, ProjectileSource}, skills::animations::{
        CastComplete, SharedAnimationPlugin, UnitFinishedSkillCast, UsingSkillSince,
    }
};

use crate::{ConnectedPlayer, EndpointToPlayerId, PlayerEndpoint};

pub struct AnimationPluginServer;

impl Plugin for AnimationPluginServer {
    fn build(&self, app: &mut App) {
        app.add_plugins(SharedAnimationPlugin)
            .add_systems(Update, (on_unit_begin_skill_use, on_unit_finish_cast));
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
                commands.entity(entity).remove::<CastComplete>();
                // send update to clients as update_entity
                // TODO
            } else {
                info!(?player_id, ?ent_id, "Beginning skill casting");
                commands.entity(entity).insert(new_using_skill);
                commands.entity(entity).remove::<CastComplete>();
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
            sr.send_outgoing_event_next_tick(client_endpoint.0, &event_to_send);
        }
    }
}

fn on_unit_finish_cast(
    mut cast_event_reader: MessageReader<UnitFinishedSkillCast>,
    query: Query<(&Transform, &NetEntId), With<UsingSkillSince>>,
    _time: Res<Time>,
    server_tick: Res<CurrentTick>,
    mut commands: Commands,
    connected_clients: Query<&PlayerEndpoint, With<ConnectedPlayer>>,
    sr: Res<ServerNetworkingResources>,
    spawn_projectile_writer: MessageWriter<SpawnProjectile>,
) {
    for UnitFinishedSkillCast {
        tick,
        net_ent_id,
        skill,
    } in cast_event_reader.read()
    {
        if tick.0 > 2 + server_tick.0 .0 {
            error!(
                ?net_ent_id,
                ?tick,
                ?server_tick,
                "Received UnitFinishedSkillCast event that is >2 ticks old from ourselves???"
            );
        }

        for (transform, ent_id) in query {
            if ent_id != net_ent_id {
                continue;
            }

            let projectile_source = ProjectileSource {
                source_entity: *ent_id,
                skill: skill.skill.clone(),
                skill_source: skill.source.clone(),
            };

            match &skill.skill {
                shared::skills::Skill::Spark => {
                    for _spark in 0..6 {
                        let mut path_targets: Vec<Vec3> = vec![];
                        let mut cur_pos = transform.translation;
                        for _target in 0..20 {
                            let mut next_target = Vec3::ZERO;
                            while next_target.length_squared() < 25.0
                                || next_target.length_squared() > 40.0
                            {
                                next_target = Vec3::new(
                                    rand::random_range(-10.0..10.0),
                                    0.0,
                                    rand::random_range(-10.0..10.0),
                                );
                            }
                            cur_pos += next_target;
                            path_targets.push(cur_pos);
                        }

                        let event = SpawnProjectile {
                            spawn_tick: server_tick.0,
                            projectile_origin: transform.translation,
                            projectile_source,
                            projectile_type: ProjectileAI::Spark {
                                projectile_path_targets: path_targets,
                            },
                        };

                        spawn_projectile_writer.write(event.clone());
                    }
                }
                shared::skills::Skill::Hammerdin => {
                    for hammer in 0..4 {
                        let proj = SpawnProjectile {
                            spawn_tick: server_tick.0,
                            projectile_origin: transform.translation,
                            projectile_source,
                            projectile_type: ProjectileAI::HammerDin {
                                init_angle_radians: (hammer as f32) * std::f32::consts::PI / 2.0,
                                center_point: transform.translation,
                                speed: 1.0,
                                spiral_width_modifier: 1.0,
                            },
                        };

                        spawn_projectile_writer.write(proj.clone());
                    }
                }
                shared::skills::Skill::SummonTestNPC => {
                    let random_xy = Vec3::new(
                        rand::random_range(-5.0..5.0),
                        0.0,
                        rand::random_range(-5.0..5.0),
                    );
                    let transform = Transform::from_translation(
                        transform.translation + Vec3::Y * 2.5 + random_xy,
                    );
                    info!(
                        ?net_ent_id,
                        "Spawning test NPC at {:?}", transform.translation
                    );
                    let npc = make_npc(transform);
                    npc.clone().spawn_entity(&mut commands);
                    let event = shared::netlib::EventToClient::SpawnUnit2(npc);
                    for client_endpoint in &connected_clients {
                        sr.send_outgoing_event_next_tick(client_endpoint.0, &event);
                    }
                }
                _ => {
                    warn!(?net_ent_id, ?skill.skill, "Received UnitFinishedSkillCast for unsupported skill");
                    break;
                }
            }
            break;
        }
    }
}
