use anyhow::{Context, ensure};
use quinn::{Connection, SendStream};

pub const MAX_MESSAGE_BYTES: usize = 1024;
const ACKNOWLEDGEMENT: &[u8] = b"\0";

/// A validated message whose receipt the server has not yet acknowledged.
pub struct ReceivedMessage {
    text: String,
    acknowledgement: SendStream,
}

impl ReceivedMessage {
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Call after processing the message, so the sender may safely finish its session.
    pub async fn acknowledge(self) -> anyhow::Result<()> {
        write_and_finish(self.acknowledgement, ACKNOWLEDGEMENT)
            .await
            .context("Cannot acknowledge the received message")
    }
}

/// Sends one UTF-8 message and waits for the server's application acknowledgement.
pub async fn send_message(connection: &Connection, message: &str) -> anyhow::Result<()> {
    ensure!(
        message.len() <= MAX_MESSAGE_BYTES,
        "Message exceeds {MAX_MESSAGE_BYTES} bytes"
    );
    let (send, mut acknowledgement) = connection
        .open_bi()
        .await
        .context("Cannot open a message stream")?;
    write_and_finish(send, message.as_bytes()).await?;
    let reply = acknowledgement
        .read_to_end(ACKNOWLEDGEMENT.len())
        .await
        .context("Cannot read the message acknowledgement")?;
    ensure!(reply == ACKNOWLEDGEMENT, "Invalid message acknowledgement");

    Ok(())
}

/// Receives one complete, size-limited UTF-8 message without acknowledging it yet.
pub async fn receive_message(connection: &Connection) -> anyhow::Result<ReceivedMessage> {
    let (acknowledgement, mut receive) = connection
        .accept_bi()
        .await
        .context("Cannot accept a message stream")?;
    let bytes = receive
        .read_to_end(MAX_MESSAGE_BYTES)
        .await
        .context("Cannot read the message")?;
    let text = String::from_utf8(bytes).context("Message is not valid UTF-8")?;

    Ok(ReceivedMessage {
        text,
        acknowledgement,
    })
}

async fn write_and_finish(mut stream: SendStream, bytes: &[u8]) -> anyhow::Result<()> {
    stream
        .write_all(bytes)
        .await
        .context("Cannot write the message stream")?;
    stream.finish().context("Cannot finish the message stream")
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
