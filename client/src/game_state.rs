use bevy::prelude::*;

#[derive(States, Reflect, PartialEq, Eq, Debug, Clone, Hash, Default)]
pub enum NetworkGameState {
    #[default]
    MainMenu,

    /// See also [shared::netlib::NetworkConnectionTarget]
    ClientConnecting,
    ClientSendRequestPacket,
    ClientConnected,

    Quit,
}

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<NetworkGameState>()
            .add_systems(OnEnter(NetworkGameState::Quit), quit_event);
    }
}

fn quit_event(mut app_exit_events: MessageWriter<bevy::app::AppExit>) {
    app_exit_events.write(bevy::app::AppExit::Success);
}
