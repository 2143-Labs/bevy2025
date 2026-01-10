use server::main_multiplayer_server;
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
        main_multiplayer_server(runtime2);
    });
}
