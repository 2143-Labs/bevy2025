//! The white fade between the title screen and the world.
//!
//! `Play now` starts a fade to white, which is the "load screen" while the client gets a ticket
//! and connects in the background. Once the world is up (`GameState::Playing`) the fade goes back
//! down, so the player is in the world, driving a man, instead of watching a title screen.
//!
//! The connecting menu still owns the status line and the Cancel button, so this overlay carries
//! them while it is up: [`spawn_connecting_menu_and_connect`](super::connecting_menu) stays out of
//! the way when a fade is running.

use bevy::prelude::*;
use bevy::ui_widgets::Button;

use super::connecting_menu::{CancelButton, ConnectionStatusText};
use super::home_menu::HomeMenu;
use super::styles::{label_text, menu_button_bundle, menu_button_text};
use crate::game_state::{GameState, MenuState};

/// How long each side of the fade takes.
const FADE_SECS: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FadePhase {
    /// The title fades out to white.
    ToWhite,
    /// White, waiting for the world to be ready.
    White,
    /// The world fades in from white.
    FromWhite,
}

/// A fade in progress. `None` (no resource) means no overlay on screen.
#[derive(Resource)]
pub struct ScreenFade {
    phase: FadePhase,
    elapsed: f32,
}

impl ScreenFade {
    fn is_fading_out(&self) -> bool {
        self.phase == FadePhase::ToWhite
    }
}

/// The full-screen white overlay.
#[derive(Component)]
pub struct ScreenFadeOverlay;

/// The status line and Cancel button, shown once the screen is white.
#[derive(Component)]
pub struct FadeContents;

pub struct FadePlugin;

impl Plugin for FadePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drive_screen_fade)
            // Cancelled, failed, or thrown back to the menu: the overlay must not survive it.
            .add_systems(OnEnter(MenuState::Home), clear_screen_fade);
    }
}

/// Fade the title out to white. Called by Play now, before the connection starts.
pub fn start_fade_to_white(commands: &mut Commands) {
    commands.insert_resource(ScreenFade {
        phase: FadePhase::ToWhite,
        elapsed: 0.0,
    });
    commands
        .spawn((
            ScreenFadeOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
            GlobalZIndex(1000),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    FadeContents,
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(20.0),
                        ..default()
                    },
                    Visibility::Hidden,
                ))
                .with_children(|contents| {
                    contents.spawn((
                        {
                            let (text, font, color) = label_text("Getting ticket...");
                            (text, font, color)
                        },
                        TextFont {
                            font_size: FontSize::Px(20.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.35, 0.35, 0.35)),
                        ConnectionStatusText,
                    ));
                    let (node, bg_color, border_color) = menu_button_bundle();
                    let (text, font, color) = menu_button_text("Cancel");
                    contents
                        .spawn((node, bg_color, border_color, Button, CancelButton))
                        .with_children(|button| {
                            button.spawn((text, font, color));
                        });
                });
        });
}

/// One frame of the fade.
fn drive_screen_fade(
    time: Res<Time>,
    game_state: Res<State<GameState>>,
    fade: Option<ResMut<ScreenFade>>,
    mut overlays: Query<(Entity, &mut BackgroundColor), With<ScreenFadeOverlay>>,
    mut contents: Query<&mut Visibility, With<FadeContents>>,
    home_menu: Query<Entity, With<HomeMenu>>,
    mut commands: Commands,
) {
    let Some(mut fade) = fade else {
        return;
    };
    let delta = time.delta_secs();
    fade.elapsed += delta;
    let alpha = match fade.phase {
        FadePhase::ToWhite => (fade.elapsed / FADE_SECS).min(1.0),
        FadePhase::White => 1.0,
        FadePhase::FromWhite => 1.0 - (fade.elapsed / FADE_SECS).min(1.0),
    };
    for (_, mut background) in overlays.iter_mut() {
        background.0 = Color::srgba(1.0, 1.0, 1.0, alpha);
    }

    match fade.phase {
        FadePhase::ToWhite if fade.elapsed >= FADE_SECS => {
            // Fully white: the title can go, and the connecting status can show.
            fade.phase = FadePhase::White;
            fade.elapsed = 0.0;
            for entity in home_menu.iter() {
                commands.entity(entity).despawn();
            }
            for mut visibility in contents.iter_mut() {
                *visibility = Visibility::Inherited;
            }
        }
        FadePhase::White if *game_state.get() == GameState::Playing => {
            fade.phase = FadePhase::FromWhite;
            fade.elapsed = 0.0;
            for mut visibility in contents.iter_mut() {
                *visibility = Visibility::Hidden;
            }
        }
        FadePhase::FromWhite if fade.elapsed >= FADE_SECS => {
            for (entity, _) in overlays.iter() {
                commands.entity(entity).despawn();
            }
            commands.remove_resource::<ScreenFade>();
        }
        _ => {}
    }
}

fn clear_screen_fade(
    fade: Option<Res<ScreenFade>>,
    overlays: Query<Entity, With<ScreenFadeOverlay>>,
    mut commands: Commands,
) {
    if fade.is_none() {
        return;
    }
    for entity in overlays.iter() {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<ScreenFade>();
}

/// Whether the title screen is still fading out (`Play now` was just pressed).
pub fn is_fading_from_title(fade: Option<&ScreenFade>) -> bool {
    fade.is_some_and(ScreenFade::is_fading_out)
}
