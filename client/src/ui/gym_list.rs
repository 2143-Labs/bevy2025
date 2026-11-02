//! Floating "LAN testing gyms" list on the home menu (see crate::gym_discovery).
use std::net::IpAddr;

use bevy::prelude::*;
use bevy::{picking::hover::Hovered, ui::Pressed, ui_widgets::Button};
use shared::gym::BUILD_HASH;

use super::{home_menu, styles::*};
use crate::{
    game_state::MenuState,
    gym_discovery::{DiscoveredGym, DiscoveredGyms},
};

/// Root of the panel. A separate root from `HomeMenu`, so menu rebuilds leave it alone.
#[derive(Component)]
pub struct GymListPanel;

#[derive(Component)]
pub struct GymJoinButton {
    ip: IpAddr,
    port: u16,
}

pub fn spawn_gym_list_panel(mut commands: Commands, gyms: Res<DiscoveredGyms>) {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(24.0),
                top: Val::Px(24.0),
                width: Val::Px(340.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(10),
            GymListPanel,
        ))
        .id();
    fill_gym_panel(&mut commands, panel, &gyms);
}

fn small_text(text: impl Into<String>) -> (Text, TextFont, TextColor) {
    let (text, mut font, color) = label_text(text);
    font.font_size = FontSize::Px(16.0);
    (text, font, color)
}

fn short_hash(hash: &str) -> &str {
    hash.get(..7).unwrap_or(hash)
}

fn fill_gym_panel(commands: &mut Commands, panel: Entity, gyms: &DiscoveredGyms) {
    commands.entity(panel).despawn_children();
    commands.entity(panel).with_children(|parent| {
        parent.spawn(label_text("LAN testing gyms"));
        if gyms.0.is_empty() {
            parent.spawn(label_text("Searching the LAN..."));
        }
        for DiscoveredGym { ip, beacon, .. } in &gyms.0 {
            let (mut node, bg_color, border_color) = menu_button_bundle();
            // Gym names are up to 32 chars: let the button grow instead of overflowing.
            node.width = Val::Percent(100.0);
            node.height = Val::Auto;
            node.min_height = Val::Px(50.0);
            node.padding = UiRect::all(Val::Px(6.0));
            let (text, mut font, color) =
                menu_button_text(format!("Join {} ({} online)", beacon.name, beacon.players));
            font.font_size = FontSize::Px(22.0);
            parent
                .spawn((
                    node,
                    bg_color,
                    border_color,
                    Button,
                    Hovered::default(),
                    GymJoinButton {
                        ip: *ip,
                        port: beacon.game_port,
                    },
                ))
                .with_children(|button| {
                    button.spawn((text, font, color));
                });
            parent.spawn(small_text(format!(
                "{ip}:{} - build {}",
                beacon.game_port,
                short_hash(&beacon.build)
            )));
            if !beacon.same_build_as_local() {
                let (text, font, _) = small_text(format!(
                    "Different version (yours: {}) - may not connect",
                    short_hash(BUILD_HASH)
                ));
                parent.spawn((text, font, TextColor(Color::srgb(1.0, 0.75, 0.2))));
            }
        }
    });
}

pub fn refresh_gym_list(
    mut commands: Commands,
    gyms: Res<DiscoveredGyms>,
    panel: Query<Entity, With<GymListPanel>>,
) {
    if let Ok(panel) = panel.single() {
        fill_gym_panel(&mut commands, panel, &gyms);
    }
}

pub fn despawn_gym_list_panel(mut commands: Commands, panels: Query<Entity, With<GymListPanel>>) {
    for panel in &panels {
        commands.entity(panel).despawn();
    }
}

pub fn handle_gym_join_buttons(
    query: Query<&GymJoinButton, Added<Pressed>>,
    mut commands: Commands,
    mut config: ResMut<crate::Config>,
    mut next_auth_state: ResMut<NextState<crate::auth::AuthState>>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
) {
    for GymJoinButton { ip, port } in &query {
        info!("Joining LAN gym {ip}:{port}");
        home_menu::connect_to_gym(
            ip.to_string(),
            *port,
            &mut commands,
            &mut config,
            &mut next_auth_state,
            &mut next_menu_state,
        );
    }
}
