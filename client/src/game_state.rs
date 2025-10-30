use bevy::prelude::*;

/// Marker component for all world/gameplay entities that should be despawned when returning to MainMenu
#[derive(Component)]
pub struct WorldEntity;

/// High-level game state for menu vs gameplay
#[derive(States, Reflect, PartialEq, Eq, Debug, Clone, Hash, Default)]
pub enum GameState {
    #[default]
    MainMenu,
    Playing,
    Paused,
}

/// Network connection state - independent of GameState
#[derive(States, Reflect, PartialEq, Eq, Debug, Clone, Hash, Default)]
pub enum NetworkGameState {
    #[default]
    Disconnected,
    /// See also [shared::netlib::NetworkConnectionTarget]
    ClientConnecting,
    ClientSendRequestPacket,
    ClientConnected,
    Paused,
    Quit,
}

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_state::<NetworkGameState>()
            .add_systems(OnEnter(GameState::MainMenu), despawn_world)
            .add_systems(OnEnter(NetworkGameState::Quit), quit_event);
    }
}

/// Despawn all world entities when entering MainMenu
fn despawn_world(mut commands: Commands, world_entities: Query<Entity, With<WorldEntity>>) {
    for entity in world_entities.iter() {
        commands.entity(entity).despawn();
    }
    info!("Despawned {} world entities", world_entities.iter().count());
}

fn quit_event(mut app_exit_events: MessageWriter<bevy::app::AppExit>) {
    app_exit_events.write(bevy::app::AppExit::Success);
}
