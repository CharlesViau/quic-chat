use std::io;

use anyhow::Context;
use quic_chat::protocol;
use quinn::{Connection, ConnectionError};
use tokio::sync::mpsc::Receiver;

pub(super) async fn run(
    connection: &Connection,
    messages: &mut Receiver<io::Result<String>>,
) -> anyhow::Result<()> {
    println!(
        "Connected to {}. Type a message and press Enter.",
        connection.remote_address()
    );

    tokio::select! {
        biased;
        reason = connection.closed() => report_disconnection(reason),
        outcome = send_messages(connection, messages) => outcome,
    }
}

async fn send_messages(
    connection: &Connection,
    messages: &mut Receiver<io::Result<String>>,
) -> anyhow::Result<()> {
    while let Some(message) = messages.recv().await {
        let message = message.context("Cannot read terminal input")?;
        if message.is_empty() {
            continue;
        }
        protocol::send_message(connection, &message).await?;
    }

    Ok(())
}

fn report_disconnection(reason: ConnectionError) -> anyhow::Result<()> {
    println!("Disconnected: {reason}");

    match reason {
        ConnectionError::ApplicationClosed(close) if close.error_code == 0u32.into() => Ok(()),
        error => Err(error).context("Server connection ended unexpectedly"),
    }
}
