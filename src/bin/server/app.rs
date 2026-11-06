use anyhow::Context;

use super::{chat_server::ChatServer, settings};

pub(super) async fn run() -> anyhow::Result<()> {
    let settings = settings::from_env()?;
    let mut server = ChatServer::bind(&settings)?;

    report_listening_address(&server)?;
    let outcome = run_until_shutdown(&mut server).await;
    server.shutdown().await;

    outcome
}

fn report_listening_address(server: &ChatServer) -> anyhow::Result<()> {
    let address = server.local_addr()?;

    println!("Listening on {address}. Press Ctrl+C to stop.");

    Ok(())
}

async fn run_until_shutdown(server: &mut ChatServer) -> anyhow::Result<()> {
    tokio::select! {
        outcome = server.run() => outcome,
        outcome = wait_for_ctrl_c() => outcome,
    }
}

async fn wait_for_ctrl_c() -> anyhow::Result<()> {
    tokio::signal::ctrl_c()
        .await
        .context("Cannot wait for Ctrl+C")
}
