//! The instance's join code, and the box that joins another tutorial by its code.
//!
//! The label sits at the top right of the world for as long as the client is on a tutorial
//! instance (`AssignedServer::join_code`). Escape opens the pause menu, which carries a
//! `join someone: ____` box under the code: submitting one asks the authserver to move this
//! character to that tutorial (`/api/v1/join-code`), and the client then reconnects — `/play`
//! answers with the new instance.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::Button;

use crate::{
    auth::{AssignedServer, AuthRequest, JoinCodeAnswer},
    camera::ChompInputs,
    game_state::{GameState, OverlayMenuState},
    network::MoveToTutorial,
    ui::FocusedInput,
    ui::text_input::{TextInput, TextInputDisplay},
};

/// Characters in a join code (`JOIN_CODE_ALPHABET` on the authserver, four of them).
const CODE_LEN: usize = 4;
const PLACEHOLDER: &str = "____";

/// The `join code: XXXX` label; holds the text it shows.
#[derive(Component)]
pub struct JoinCodeLabel(pub String);

/// Root of the "join someone" box.
#[derive(Component)]
pub struct JoinSomeonePanel;

/// The code input inside that box.
#[derive(Component)]
pub struct JoinCodeInput;

/// The line under the input: what happened to the last code entered.
#[derive(Component)]
pub struct JoinCodeStatusText;

/// The code being looked up, and what to tell the player about it.
#[derive(Resource, Default)]
pub struct JoinCodeStatus {
    pending: Option<String>,
    line: String,
}

/// Set while the join-code box holds the keyboard. Only this plugin removes the [`ChompInputs`]
/// it inserted — the skills menu's is its own.
#[derive(Resource)]
struct JoinCodeChomps;

pub struct JoinCodePlugin;

impl Plugin for JoinCodePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<JoinCodeStatus>()
            .add_systems(
                Update,
                (sync_join_code_label, on_join_code_answer).run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnExit(GameState::Playing), despawn_join_code_label)
            .add_systems(OnEnter(OverlayMenuState::Paused), spawn_join_panel)
            .add_systems(OnExit(OverlayMenuState::Paused), despawn_join_panel)
            .add_systems(
                Update,
                (
                    sync_join_code_chomps,
                    join_code_keys,
                    update_join_status_text,
                )
                    .run_if(in_state(OverlayMenuState::Paused)),
            );
    }
}

fn label_font() -> TextFont {
    TextFont {
        font_size: FontSize::Px(18.0),
        ..default()
    }
}

/// The faint white text with a black outline: four offset copies behind the fill. Bevy 0.20 has
/// no text outline, only `TextShadow`.
fn spawn_outlined_text(parent: &mut ChildSpawnerCommands, text: String) {
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        parent.spawn((
            Text::new(text.clone()),
            label_font(),
            TextColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(dx),
                top: Val::Px(dy),
                ..default()
            },
        ));
    }
    parent.spawn((
        Text::new(text),
        label_font(),
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
    ));
}

fn spawn_join_code_label(commands: &mut Commands, text: String) {
    commands
        .spawn((
            JoinCodeLabel(text.clone()),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                right: Val::Px(16.0),
                ..default()
            },
            GlobalZIndex(10),
        ))
        .with_children(|parent| spawn_outlined_text(parent, text));
}

/// Keeps one label on screen, showing this instance's code — and none on a server without one
/// (a static server or a gym).
fn sync_join_code_label(
    assigned: Option<Res<AssignedServer>>,
    labels: Query<(Entity, &JoinCodeLabel)>,
    mut commands: Commands,
) {
    let want = assigned
        .as_deref()
        .and_then(|assigned| assigned.join_code.as_deref())
        .map(|code| format!("join code: {code}"));
    match (want, labels.iter().next()) {
        (None, None) => {}
        (None, Some((entity, _))) => commands.entity(entity).despawn(),
        (Some(want), None) => spawn_join_code_label(&mut commands, want),
        (Some(want), Some((entity, label))) => {
            if label.0 != want {
                commands.entity(entity).despawn();
                spawn_join_code_label(&mut commands, want);
            }
        }
    }
}

fn despawn_join_code_label(labels: Query<Entity, With<JoinCodeLabel>>, mut commands: Commands) {
    for entity in labels.iter() {
        commands.entity(entity).despawn();
    }
}

/// Escape opens the pause menu; a tutorial instance puts its "join someone" box there.
fn spawn_join_panel(
    mut commands: Commands,
    assigned: Option<Res<AssignedServer>>,
    existing: Query<Entity, With<JoinSomeonePanel>>,
    mut status: ResMut<JoinCodeStatus>,
) {
    *status = JoinCodeStatus::default();
    if existing.iter().next().is_some() {
        return;
    }
    // Only a managed instance has a join code; there is nothing to join from elsewhere.
    if assigned
        .as_deref()
        .and_then(|assigned| assigned.join_code.as_deref())
        .is_none()
    {
        return;
    }
    commands
        .spawn((
            JoinSomeonePanel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(40.0),
                right: Val::Px(16.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(6.0),
                ..default()
            },
            GlobalZIndex(10),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("join someone:"),
                label_font(),
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                TextShadow {
                    offset: Vec2::splat(1.0),
                    color: Color::BLACK,
                },
            ));
            panel
                .spawn((
                    Node {
                        width: Val::Px(90.0),
                        height: Val::Px(30.0),
                        border: UiRect::all(Val::Px(2.0)),
                        padding: UiRect::all(Val::Px(6.0)),
                        justify_content: JustifyContent::Start,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.4, 0.4, 0.4)),
                    BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                    Button,
                    Hovered::default(),
                    TextInput::new(PLACEHOLDER, CODE_LEN),
                    JoinCodeInput,
                ))
                .with_children(|input| {
                    input.spawn((
                        Text::new(PLACEHOLDER),
                        TextFont {
                            font_size: FontSize::Px(18.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.5, 0.5)),
                        TextInputDisplay,
                    ));
                });
            panel.spawn((
                JoinCodeStatusText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
            ));
        });
}

fn despawn_join_panel(
    mut commands: Commands,
    panels: Query<Entity, With<JoinSomeonePanel>>,
    mut focused: ResMut<FocusedInput>,
    chomped: Option<Res<JoinCodeChomps>>,
) {
    for entity in panels.iter() {
        commands.entity(entity).despawn();
    }
    // The focused entity may be one of the despawned input's children.
    focused.0 = None;
    if chomped.is_some() {
        commands.remove_resource::<JoinCodeChomps>();
        commands.remove_resource::<ChompInputs>();
    }
}

/// While the box holds the keyboard, the game's own keys (WASD, Q/E, Escape to pause) must not act.
fn sync_join_code_chomps(
    focused: Res<FocusedInput>,
    inputs: Query<(), With<JoinCodeInput>>,
    chomped: Option<Res<JoinCodeChomps>>,
    mut commands: Commands,
) {
    let typing = focused.0.is_some_and(|entity| inputs.get(entity).is_ok());
    match (typing, chomped.is_some()) {
        (true, false) => {
            commands.insert_resource(JoinCodeChomps);
            commands.insert_resource(ChompInputs);
        }
        (false, true) => {
            commands.remove_resource::<JoinCodeChomps>();
            commands.remove_resource::<ChompInputs>();
        }
        _ => {}
    }
}

fn join_code_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    focused: Res<FocusedInput>,
    inputs: Query<&TextInput, With<JoinCodeInput>>,
    assigned: Option<Res<AssignedServer>>,
    mut status: ResMut<JoinCodeStatus>,
    mut auth_requests: MessageWriter<AuthRequest>,
    mut next_overlay: ResMut<NextState<OverlayMenuState>>,
) {
    let Some(entity) = focused.0 else {
        return;
    };
    let Ok(input) = inputs.get(entity) else {
        return;
    };
    // The camera's Escape handler is chomped while the box has the keyboard: this is the only way
    // out of it.
    if keyboard.just_pressed(KeyCode::Escape) {
        next_overlay.set(OverlayMenuState::Hidden);
        return;
    }
    if !(keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::NumpadEnter)) {
        return;
    }
    let code = input.value.trim().to_ascii_uppercase();
    if code.chars().count() != CODE_LEN {
        status.line = "Join codes are 4 characters".to_string();
        return;
    }
    if assigned
        .as_deref()
        .and_then(|assigned| assigned.join_code.as_deref())
        == Some(code.as_str())
    {
        status.line = "You are already there".to_string();
        return;
    }
    info!("join code {code}: asking the authserver");
    status.line = format!("Looking for {code}…");
    status.pending = Some(code.clone());
    auth_requests.write(AuthRequest::JoinCode(code));
}

fn update_join_status_text(
    status: Res<JoinCodeStatus>,
    mut texts: Query<&mut Text, With<JoinCodeStatusText>>,
) {
    if !status.is_changed() {
        return;
    }
    for mut text in texts.iter_mut() {
        text.0 = status.line.clone();
    }
}

fn on_join_code_answer(
    mut answers: MessageReader<JoinCodeAnswer>,
    mut status: ResMut<JoinCodeStatus>,
    mut moves: MessageWriter<MoveToTutorial>,
    mut next_overlay: ResMut<NextState<OverlayMenuState>>,
) {
    for answer in answers.read() {
        // An answer with no request in flight belongs to a previous box (Escape, then Enter).
        let Some(code) = status.pending.take() else {
            continue;
        };
        match answer {
            JoinCodeAnswer::Moved => {
                info!("join code {code}: moving to that tutorial");
                next_overlay.set(OverlayMenuState::Hidden);
                moves.write(MoveToTutorial { code });
            }
            JoinCodeAnswer::AlreadyThere => status.line = "You are already there".to_string(),
            JoinCodeAnswer::NotFound => status.line = "No tutorial with that code".to_string(),
            JoinCodeAnswer::Unavailable => {
                status.line = "That tutorial can't take players right now".to_string();
            }
            JoinCodeAnswer::Failed(e) => status.line = format!("Could not join: {e}"),
        }
    }
}
