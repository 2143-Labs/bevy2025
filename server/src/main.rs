use server::{main_multiplayer_server, ticket::TicketVerifier};
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
        main_multiplayer_server(runtime2, verifier);
    });
}
