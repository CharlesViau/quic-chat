use std::io;

use anyhow::Context;
use tokio::sync::mpsc::Receiver;

use super::{
    chat_client::ChatClient,
    console,
    settings::{self, ClientSettings},
};

pub(super) async fn run() -> anyhow::Result<()> {
    let settings = settings::from_env()?;
    let mut client = ChatClient::bind(&settings)?;
    let mut messages = console::read_messages()?;

    report_connection_target(&settings);
    let outcome = run_until_shutdown(&mut client, &mut messages).await;
    client.shutdown().await;

    outcome
}

fn report_connection_target(settings: &ClientSettings) {
    println!(
        "Connecting to {} as {}. Press Ctrl+C to stop.",
        settings.server_addr, settings.server_name
    );
}

async fn run_until_shutdown(
    client: &mut ChatClient,
    messages: &mut Receiver<io::Result<String>>,
) -> anyhow::Result<()> {
    tokio::select! {
        outcome = client.run(messages) => outcome,
        outcome = wait_for_ctrl_c() => outcome,
    }
}

async fn wait_for_ctrl_c() -> anyhow::Result<()> {
    tokio::signal::ctrl_c()
        .await
        .context("Cannot wait for Ctrl+C")
}
