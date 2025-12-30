use dashmap::DashMap;
use shared::{Config, GameAction, event::NetEntId, skills::Skill};
use bevy::prelude::*;

use crate::{game_state::GameState, network::CurrentThirdPersonControlledUnit};

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

pub struct SkillBindsPlugin;

impl Plugin for SkillBindsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SkillComboKey>()
            .add_message::<BindSkillToKey>()
            .add_message::<BeginSkillUse>();
        app.insert_resource(LocallyBoundSkills {
            bound_skills: DashMap::new(),
        })
        .insert_resource(CurrentlyPressedSkillKeys {
            pressed_modifier_keys: Vec::new(),
            pressed_keys: Vec::new(),
            last_pressed_combo: None,
        });
        app.add_systems(
            Update,
            (on_bind_skill_to_key,
             check_for_key_combo_press
             ).run_if(in_state(GameState::Playing)),
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
    pub skill: Skill,
    pub unit: NetEntId,
}

fn on_bind_skill_to_key(
    mut bind_events: MessageReader<BindSkillToKey>,
    locally_bound_skills: ResMut<LocallyBoundSkills>,
) {
    for event in bind_events.read() {
        locally_bound_skills
            .bound_skills
            .insert(event.key.normalized(), event.skill.clone());

        info!(
            "Bound skill {:?} to key {:?}",
            event.skill, event.key
        );
    }
}

const SKILL_KEYS: [GameAction; 6] = [
    GameAction::Jump,
    GameAction::Special1,
    GameAction::Special2,
    GameAction::Special3,
    GameAction::Fire1,
    GameAction::Fire2,
];

const MODIFIER_KEYS: [GameAction; 3] = [
    GameAction::Mod1,
    GameAction::Mod2,
    GameAction::Mod3,
];



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
            }.normalized();

            begin_key_combo.write(combo_key.clone());
            currently_pressed_skill_keys.last_pressed_combo = Some(combo_key);
            pressed_skill_keys.push(*skill_key);
        }
    }
    currently_pressed_skill_keys.pressed_modifier_keys = pressed_modifiers;
    currently_pressed_skill_keys.pressed_keys = pressed_skill_keys;
}
