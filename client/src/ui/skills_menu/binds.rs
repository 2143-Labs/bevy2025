use bevy::prelude::*;
use dashmap::DashMap;
use shared::{Config, GameAction, event::NetEntId, items::SkillFromSkillSource, skills::Skill};

use crate::{
    camera::ChompInputs, game_state::GameState, network::CurrentThirdPersonControlledUnit,
};

#[derive(Hash, PartialEq, Eq, Clone, Debug, Message)]
pub struct SkillComboKey {
    pub base_key: GameAction,
    pub modifier_keys: Vec<GameAction>,
}

impl SkillComboKey {
    pub fn normalize(&mut self) {
        self.modifier_keys.sort();
    }

    pub fn normalized(&self) -> SkillComboKey {
        let mut new_key = self.clone();
        new_key.normalize();
        new_key
    }
}

#[derive(Resource)]
pub struct LocallyBoundSkills {
    pub bound_skills: DashMap<SkillComboKey, Skill>,
}

#[derive(Resource)]
pub struct CurrentlyPressedSkillKeys {
    pub pressed_modifier_keys: Vec<GameAction>,
    pub pressed_keys: Vec<GameAction>,
    pub last_pressed_combo: Option<SkillComboKey>,
}

// TODO change these to not construct string
impl std::fmt::Display for SkillComboKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut combo_string = String::new();
        for modifier in &self.modifier_keys {
            combo_string.push_str(&format!("{:?} + ", modifier));
        }
        combo_string.push_str(&format!("{:?}", self.base_key));
        write!(f, "{}", combo_string)
    }
}

impl std::fmt::Display for CurrentlyPressedSkillKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut combo_string = String::new();
        for modifier in &self.pressed_modifier_keys {
            combo_string.push_str(&format!("{:?} + ", modifier));
        }
        if let Some(key) = self.pressed_keys.first() {
            combo_string.push_str(&format!("{:?}", key));
        } else {
            combo_string.push_str("...");
        }
        write!(f, "{}", combo_string)
    }
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
/// This state will block all input and let players bind a skill
pub enum SkillBindOverlayState {
    /// Initial state to put menu in when ready to bind a skill
    Active,

    ReadyForBinding,

    JustFinishedBinding,
    #[default]
    Inactive,
}

#[derive(Resource)]
pub struct CurrentSkillWeAreBinding {
    pub skill: SkillFromSkillSource,
}

pub struct SkillBindsPlugin;

impl Plugin for SkillBindsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SkillComboKey>()
            .add_message::<BindSkillToKey>()
            .add_message::<BeginSkillUse>();
        app.insert_state(SkillBindOverlayState::Inactive);
        app.insert_resource(LocallyBoundSkills {
            bound_skills: DashMap::new(),
        })
        .insert_resource(CurrentlyPressedSkillKeys {
            pressed_modifier_keys: Vec::new(),
            pressed_keys: Vec::new(),
            last_pressed_combo: None,
        });

        app.add_systems(Update, tick_disappear_res.run_if(
            resource_exists::<DisappearIn>
        ));
        app.add_systems(
            Update,
            (on_bind_skill_to_key, check_for_key_combo_press).run_if(in_state(GameState::Playing)),
        );

        app.add_systems(
            OnExit(SkillBindOverlayState::Inactive),
            spawn_skill_bind_overlay,
        );
        app.add_systems(
            OnEnter(SkillBindOverlayState::Inactive),
            despawn_skill_bind_overlay,
        );
        app.add_systems(
            Update,
            (
                redraw_skill_bind_overlay,
                on_skill_combo_key_during_bind_overlay,
            ).run_if(not(in_state(SkillBindOverlayState::JustFinishedBinding)))
                .run_if(not(in_state(SkillBindOverlayState::Inactive))),
        );
    }
}

#[derive(Debug, Clone, Message)]
pub struct BindSkillToKey {
    pub skill: Skill,
    pub key: SkillComboKey,
}

#[derive(Debug, Clone, Message)]
pub struct BeginSkillUse {
    pub skill: SkillFromSkillSource,
    pub unit: NetEntId,
}

fn on_bind_skill_to_key(
    mut bind_events: MessageReader<BindSkillToKey>,
    mut query: Query<&mut Text, With<SkillBindOverlayText>>,
    locally_bound_skills: ResMut<LocallyBoundSkills>,
) {
    for event in bind_events.read() {
        locally_bound_skills
            .bound_skills
            .insert(event.key.normalized(), event.skill.clone());

        info!("Bound skill {:?} to key {:?}", event.skill, event.key);
        info!(
            "Current skill bindings: {:?}",
            locally_bound_skills.bound_skills
        );

        if let Ok(mut text) = query.single_mut() {
            let combo_string = event.key.to_string();
            text.0 = format!("Bound Skill: {}", combo_string);
        }
    }
}

const SKILL_KEYS: [GameAction; 7] = [
    GameAction::Jump,
    GameAction::Special1,
    GameAction::Special2,
    GameAction::Special3,
    GameAction::Fire1,
    GameAction::Fire2,
    GameAction::Escape,
];

const MODIFIER_KEYS: [GameAction; 3] = [GameAction::Mod1, GameAction::Mod2, GameAction::Mod3];

fn check_for_key_combo_press(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    config: Res<Config>,
    mut begin_key_combo: MessageWriter<SkillComboKey>,
    mut currently_pressed_skill_keys: ResMut<CurrentlyPressedSkillKeys>,
) {
    let mut pressed_modifiers = Vec::new();
    for modifier_key in MODIFIER_KEYS.iter() {
        if config.pressed(&keyboard, &mouse, *modifier_key) {
            pressed_modifiers.push(*modifier_key);
        }
    }

    let mut pressed_skill_keys = Vec::new();
    for skill_key in SKILL_KEYS.iter() {
        if config.just_pressed(&keyboard, &mouse, *skill_key) {
            let combo_key = SkillComboKey {
                base_key: *skill_key,
                modifier_keys: pressed_modifiers.clone(),
            }
            .normalized();

            begin_key_combo.write(combo_key.clone());
            currently_pressed_skill_keys.last_pressed_combo = Some(combo_key);
        }

        if config.pressed(&keyboard, &mouse, *skill_key) {
            pressed_skill_keys.push(*skill_key);
        }
    }
    currently_pressed_skill_keys.pressed_modifier_keys = pressed_modifiers;
    currently_pressed_skill_keys.pressed_keys = pressed_skill_keys;
}

#[derive(Component)]
pub struct SkillBindOverlay;

#[derive(Component)]
pub struct SkillBindOverlayText;

#[derive(Resource)]
pub struct DisappearIn(Timer);

fn tick_disappear_res(
    time: Res<Time>,
    mut commands: Commands,
    mut di: ResMut<DisappearIn>,
    mut next_state: ResMut<NextState<SkillBindOverlayState>>,
) {
    di.0.tick(time.delta());
    if di.0.is_finished() {
        commands.remove_resource::<DisappearIn>();
        next_state.set(SkillBindOverlayState::Inactive);
    }
}

fn spawn_skill_bind_overlay(mut commands: Commands) {
    // Disable camera/unit inputs
    commands.insert_resource(ChompInputs);

    // We do 2 things- darken the screen background and then spawn a box to print binds in
    commands
        .spawn((SkillBindOverlay, Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        }))
        .with_children(|parent| {
            // Darken background
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            ));

            // Box for binds
            parent
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(25.0),
                        top: Val::Percent(25.0),
                        width: Val::Percent(50.0),
                        height: Val::Percent(50.0),
                        border: UiRect::all(Val::Px(3.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                    BorderColor::all(Color::BLACK),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        SkillBindOverlayText,
                        Text::new("Press a key combination to bind a skill..."),
                        TextFont {
                            font_size: 32.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                    ));
                });
        });
}

fn despawn_skill_bind_overlay(
    mut commands: Commands,
    query: Query<Entity, With<SkillBindOverlay>>,
) {
    let Ok(overlay_entity) = query.single() else {
        error!("No SkillBindOverlay found when despawning skill bind overlay");
        return;
    };
    commands.entity(overlay_entity).despawn();
    commands.remove_resource::<ChompInputs>();
}

fn redraw_skill_bind_overlay(
    mut query: Query<&mut Text, With<SkillBindOverlayText>>,
    currently_pressed_skill_keys: Res<CurrentlyPressedSkillKeys>,
) {
    let Ok(mut text) = query.single_mut() else {
        error!("No SkillBindOverlayText found when redrawing skill bind overlay");
        return;
    };

    let combo_string = currently_pressed_skill_keys.to_string();
    text.0 = format!("Key: {}", combo_string);
}

fn on_skill_combo_key_during_bind_overlay(
    mut reader: MessageReader<SkillComboKey>,
    cur_state: Res<State<SkillBindOverlayState>>,
    mut next_state: ResMut<NextState<SkillBindOverlayState>>,
    mut bind_events: MessageWriter<BindSkillToKey>,
    current_binding_skill: Res<CurrentSkillWeAreBinding>,
    mut commands: Commands,
) {
    let mut our_cur_state = cur_state.clone();
    for ev in reader.read() {
        debug!("Checking for skill use during bind overlay {:?}", ev);
        // We need to eat the first bind, since usually fire will be pressed to open the bind
        // overlay itsslef
        if our_cur_state == SkillBindOverlayState::Active {
            debug!(
                "Key combo {:?} pressed during bind overlay, moving to ReadyForBinding",
                ev
            );
            next_state.set(SkillBindOverlayState::ReadyForBinding);
            our_cur_state = SkillBindOverlayState::ReadyForBinding;
            continue;
        }

        if ev.base_key == GameAction::Escape {
            debug!("Escape pressed during bind overlay, cancelling bind");
            next_state.set(SkillBindOverlayState::Inactive);
            return;
        }

        debug!(
            "Key combo {:?} pressed during bind overlay, moving to Inactive",
            ev
        );
        bind_events.write(BindSkillToKey {
            skill: current_binding_skill.skill.skill.clone(),
            key: ev.clone(),
        });
        next_state.set(SkillBindOverlayState::JustFinishedBinding);
        commands.insert_resource(DisappearIn(Timer::from_seconds(0.5, TimerMode::Once)));
    }
}
