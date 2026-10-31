use bevy::prelude::PluginGroup;
use server::{GameServerCoordinator, ServerSettings, run_server, ticket::TicketVerifier};
use tokio::runtime;

//single thread
fn main() {
    let runtime = runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let runtime = std::sync::Arc::new(runtime);

    let runtime2 = runtime.clone();
    runtime.block_on(async {
        let verifier = TicketVerifier::from_env().await;
        run_server(
            bevy::prelude::DefaultPlugins
                .build()
                .add(bevy::app::ScheduleRunnerPlugin::run_loop(
                    server::SERVER_LOOP_WAIT,
                )),
            runtime2,
            ServerSettings {
                verifier,
                bind: None,
                terrain: None,
                coordinator: GameServerCoordinator::from_env(),
                lan_beacon: None,
            },
        );
    });
}
