use std::{
    collections::{HashMap, VecDeque},
    env::current_dir,
    fs::OpenOptions,
    sync::atomic::AtomicI32,
};

use bevy::prelude::*;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::netlib::Tick;

pub mod character_controller;
pub mod decimal;
pub mod event;
pub mod items;
pub mod net_components;
pub mod netlib;
pub mod physics;
pub mod player_input;
pub mod projectile;
pub mod skills;
pub mod stats;
pub mod stats2;

#[cfg(not(feature = "udp"))]
pub mod message_io;
#[cfg(feature = "udp")]
pub use message_io;

pub const BASE_TICKS_PER_SECOND: u16 = 60;

#[derive(
    Reflect, Hash, Eq, PartialEq, Clone, Deserialize, Serialize, Debug, Ord, PartialOrd, Copy,
)]
pub enum GameAction {
    MoveForward,
    MoveBackward,
    StrafeRight,
    StrafeLeft,
    /// Move vertically up
    /// Space
    Ascend,
    /// Space
    Jump,
    Descend,

    /// Left Click
    Fire1,
    /// Right click
    Fire2,
    /// Shift
    Mod1,
    /// Ctrl
    Mod2,
    /// Alt
    Mod3,
    /// Q
    Special1,
    /// E
    Special2,
    /// F
    Special3,

    Escape,
    ZoomCameraIn,
    ZoomCameraOut,
    OpenInventory,
    Scoreboard,
    Skills,

    Chat,
}

static DEFAULT_BINDS: Lazy<Keybinds> = Lazy::new(|| {
    let kk = |k: KeyCode| KeyCodeOrMouseButton::KeyCode(k);
    let mb = |m: MouseButton| KeyCodeOrMouseButton::MouseButton(m);
    HashMap::from([
        (GameAction::MoveForward, vec![kk(KeyCode::KeyW)]),
        (GameAction::MoveBackward, vec![kk(KeyCode::KeyS)]),
        (GameAction::StrafeLeft, vec![kk(KeyCode::KeyA)]),
        (GameAction::StrafeRight, vec![kk(KeyCode::KeyD)]),
        (GameAction::Ascend, vec![kk(KeyCode::Space)]),
        (GameAction::Descend, vec![kk(KeyCode::ShiftLeft)]),
        (GameAction::Jump, vec![kk(KeyCode::Space)]),
        (GameAction::Fire1, vec![mb(MouseButton::Left)]),
        (GameAction::Fire2, vec![mb(MouseButton::Right)]),
        (
            GameAction::Mod1,
            vec![kk(KeyCode::ShiftLeft), kk(KeyCode::ShiftRight)],
        ),
        (
            GameAction::Mod2,
            vec![kk(KeyCode::ControlLeft), kk(KeyCode::ControlRight)],
        ),
        (
            GameAction::Mod3,
            vec![kk(KeyCode::AltLeft), kk(KeyCode::AltRight)],
        ),
        (GameAction::Special1, vec![kk(KeyCode::KeyQ)]),
        (GameAction::Special2, vec![kk(KeyCode::KeyE)]),
        (GameAction::Special3, vec![kk(KeyCode::KeyF)]),
        (GameAction::Escape, vec![kk(KeyCode::Escape)]),
        (GameAction::Chat, vec![kk(KeyCode::Enter)]),
        (GameAction::ZoomCameraIn, vec![kk(KeyCode::Equal)]),
        (GameAction::ZoomCameraOut, vec![kk(KeyCode::Minus)]),
        (
            GameAction::OpenInventory,
            vec![kk(KeyCode::KeyI), kk(KeyCode::Tab)],
        ),
        (GameAction::Scoreboard, vec![kk(KeyCode::KeyP)]),
        (GameAction::Skills, vec![kk(KeyCode::KeyK)]),
    ])
});

impl GameAction {
    /// Run condition that returns true if this keycode was just pressed
    pub const fn just_pressed(
        &'static self,
    ) -> impl Fn(Res<ButtonInput<KeyCode>>, Res<ButtonInput<MouseButton>>, Res<Config>) -> bool
    {
        move |keyboard_input, mouse_input, config| {
            config.just_pressed(&keyboard_input, &mouse_input, self.clone())
        }
    }
}

#[derive(Reflect, Clone, Resource, Deserialize, Serialize, Debug)]
pub struct Config {
    pub ip: String,
    pub host_ip: Option<String>,
    pub port: u16,
    pub name: Option<String>,
    /// Player color as HSL hue (0.0-360.0)
    #[serde(default)]
    pub player_color_hue: f32,
    //#[serde(default="default_sens")]
    pub sens: f32,
    //#[serde(default="default_qe_sens")]
    pub qe_sens: f32,
    /// Should sound play on hits?
    pub sound: Option<bool>,

    pub keybindings: Keybinds, // TODO rust_phf
}

#[derive(Reflect, Clone, Hash, Eq, PartialEq, Deserialize, Serialize, Debug)]
pub enum KeyCodeOrMouseButton {
    KeyCode(KeyCode),
    MouseButton(MouseButton),
}

impl From<KeyCode> for KeyCodeOrMouseButton {
    fn from(value: KeyCode) -> Self {
        KeyCodeOrMouseButton::KeyCode(value)
    }
}

impl From<MouseButton> for KeyCodeOrMouseButton {
    fn from(value: MouseButton) -> Self {
        KeyCodeOrMouseButton::MouseButton(value)
    }
}

type Keybinds = HashMap<GameAction, Vec<KeyCodeOrMouseButton>>;

impl Config {
    pub fn pressing_keybind(
        &self,
        mut keyboard_input: impl FnMut(KeyCode) -> bool,
        mut mouse_input: impl FnMut(MouseButton) -> bool,
        ga: GameAction,
    ) -> bool {
        let bound_key_codes = match self.keybindings.get(&ga) {
            Some(b) => b,
            None => DEFAULT_BINDS.get(&ga).unwrap(),
        };

        for key in bound_key_codes {
            match key {
                KeyCodeOrMouseButton::KeyCode(c) => {
                    if keyboard_input(*c) {
                        return true;
                    }
                }
                KeyCodeOrMouseButton::MouseButton(mb) => {
                    if mouse_input(*mb) {
                        return true;
                    }
                }
            }
        }

        false
    }

    pub fn just_pressed(
        &self,
        keyboard_input: &Res<ButtonInput<KeyCode>>,
        mouse_input: &Res<ButtonInput<MouseButton>>,
        ga: GameAction,
    ) -> bool {
        self.pressing_keybind(
            |x| keyboard_input.just_pressed(x),
            |x| mouse_input.just_pressed(x),
            ga,
        )
    }

    pub fn pressed(
        &self,
        keyboard_input: &Res<ButtonInput<KeyCode>>,
        mouse_input: &Res<ButtonInput<MouseButton>>,
        ga: GameAction,
    ) -> bool {
        self.pressing_keybind(
            |x| keyboard_input.pressed(x),
            |x| mouse_input.pressed(x),
            ga,
        )
    }

    pub fn just_released(
        &self,
        keyboard_input: &Res<ButtonInput<KeyCode>>,
        mouse_input: &Res<ButtonInput<MouseButton>>,
        ga: GameAction,
    ) -> bool {
        self.pressing_keybind(
            |x| keyboard_input.just_released(x),
            |x| mouse_input.just_released(x),
            ga,
        )
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ip: "127.0.0.1".into(),
            host_ip: None,
            port: 25565,
            player_color_hue: 0.0, // Default to red
            sens: 0.003,
            qe_sens: 3.0,
            name: None,
            sound: Some(false),
            keybindings: DEFAULT_BINDS.clone(),
        }
    }
}

impl Config {
    pub fn sound(&self) -> bool {
        self.sound.unwrap_or(false)
    }
}

pub struct ConfigPlugin;
impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        let config = Config::load_from_main_dir();
        app.insert_resource(config).register_type::<Config>();
    }
}

impl Config {
    pub fn default_config_str() -> String {
        serde_yaml::to_string(&Self::default()).unwrap()
    }

    pub fn debug_keybinds(&self) {
        info!(?self.keybindings);
    }

    pub fn load_from_main_dir() -> Self {
        let Ok(mut path) = current_dir() else {
            // we are in the web build TODO
            return Self {
                ip: "127.0.0.1".to_string(),
                port: 25555,
                ..Self::default()
            };
        };
        path.push("config.yaml");

        info!("Loading config from {path:?}");
        // Try to open config file
        match OpenOptions::new().read(true).open(&path) {
            Ok(file) => match serde_yaml::from_reader(file) {
                Ok(user_config) => {
                    let mut user_config: Config = user_config;

                    // For each keybind, assign the default if not bound.
                    let mut all_binds = DEFAULT_BINDS.clone();
                    all_binds.extend(user_config.keybindings);
                    user_config.keybindings = all_binds;

                    user_config
                }
                Err(e) => {
                    eprintln!("====================================");
                    eprintln!("===  Failed to load your config  ===");
                    eprintln!("====================================");
                    eprintln!("{:?}", e);
                    eprintln!("Here is the default config:");
                    println!("{}", Self::default_config_str());
                    panic!("Please fix the above error and restart your program");
                }
            },
            Err(kind) => match kind.kind() {
                //if it doesn't exist, try to create it.
                std::io::ErrorKind::NotFound => {
                    let config = Self::default();

                    let file_handler = OpenOptions::new()
                        .create(true)
                        .truncate(true)
                        .write(true)
                        .open(&path)
                        .unwrap();

                    serde_yaml::to_writer(file_handler, &config).unwrap();
                    // should mabye just crash here and ask them to review their config
                    config
                }
                e => panic!("Failed to open config file {e:?}"),
            },
        }
    }
}

pub struct TickPlugin;

impl Plugin for TickPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(BASE_TICKS_PER_SECOND as _))
            .insert_resource(CurrentTick(Tick(1)))
            .insert_resource(ServerTPS {
                last_tick_seconds_since_start: -1.0,
                latest_tick_times: VecDeque::new(),
            });
    }
}

#[derive(Resource, Debug)]
pub struct CurrentTick(pub Tick);

#[derive(Resource, Debug, Clone, Serialize, Deserialize, Message)]
pub struct ServerTPS {
    last_tick_seconds_since_start: f64,
    latest_tick_times: VecDeque<f64>,
}

pub fn increment_ticks(
    time: Res<Time<Fixed>>,
    mut current_tick: ResMut<CurrentTick>,
    mut last_completed_increment: ResMut<ServerTPS>,
) {
    current_tick.0.increment();

    let last_time = last_completed_increment.last_tick_seconds_since_start;
    let cur_time = time.elapsed_secs_f64();
    last_completed_increment.last_tick_seconds_since_start = cur_time;
    if last_time < 0.0 {
        // First tick, don't record delta
        return;
    }

    let delta = cur_time - last_time;
    last_completed_increment.latest_tick_times.push_back(delta);

    if last_completed_increment.latest_tick_times.len() > 1000 {
        last_completed_increment.latest_tick_times.pop_front();
    }

    if current_tick.0 .0.is_multiple_of(100) {
        let mut ticks_in_order = last_completed_increment
            .latest_tick_times
            .iter()
            .cloned()
            .collect::<Vec<f64>>();

        ticks_in_order.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let average = last_completed_increment
            .latest_tick_times
            .iter()
            .sum::<f64>()
            / last_completed_increment.latest_tick_times.len() as f64;

        debug!("Server Tick: {:?}", current_tick.0);
        debug!(
            "Average Tick Time: {:.4} s ({:.2} TPS)",
            average,
            1.0 / average
        );

        debug!(
            "tick 99%, 90%, 50%: {:.4}s, {:.4}s, {:.4}s",
            ticks_in_order[(ticks_in_order.len() as f64 * 0.99) as usize - 1],
            ticks_in_order[(ticks_in_order.len() as f64 * 0.90) as usize - 1],
            ticks_in_order[(ticks_in_order.len() as f64 * 0.50) as usize - 1],
        );
    }
}

pub type PlayerPingAtomic = AtomicI32;
pub type PlayerPingInteger = i32;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Ping information about a player, in milliseconds, but generic over the type used to store the
/// ping value. Usually used with either i16 or AtomicI16: use [`Self::to_integer`] to convert from
/// one to the other.
pub struct PlayerPing<V> {
    pub server_challenged_ping_microsec: V,
    pub client_reported_ping_microsec: V,
}

impl PlayerPing<PlayerPingAtomic> {
    pub fn to_integer(&self) -> PlayerPing<PlayerPingInteger> {
        PlayerPing {
            server_challenged_ping_microsec: self
                .server_challenged_ping_microsec
                .load(std::sync::atomic::Ordering::Relaxed),
            client_reported_ping_microsec: self
                .client_reported_ping_microsec
                .load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}

impl PlayerPing<PlayerPingInteger> {
    pub fn to_atomic(&self) -> PlayerPing<PlayerPingAtomic> {
        PlayerPing {
            server_challenged_ping_microsec: PlayerPingAtomic::new(
                self.server_challenged_ping_microsec,
            ),
            client_reported_ping_microsec: PlayerPingAtomic::new(
                self.client_reported_ping_microsec,
            ),
        }
    }
}
