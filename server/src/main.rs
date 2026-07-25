mod service;
mod state;
mod world;

use std::net::SocketAddr;

use service::GameServiceImpl;
use state::GameState;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let addr: SocketAddr = "0.0.0.0:50051".parse()?;
    let state = GameState::new();
    let service = GameServiceImpl::new(state);

    info!("Game server listening on {addr}");

    tonic::transport::Server::builder()
        .add_service(service.into_server())
        .serve(addr)
        .await?;

    Ok(())
}
