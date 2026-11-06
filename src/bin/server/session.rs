use quic_chat::protocol;
use quinn::{Connection, Incoming};

pub(super) async fn run(incoming: Incoming) {
    let address = incoming.remote_address();

    match incoming.await {
        Ok(connection) => observe_connection(connection).await,
        Err(error) => eprintln!("Connection from {address} failed: {error}"),
    }
}

async fn observe_connection(connection: Connection) {
    let address = connection.remote_address();
    println!("Client connected: {address}");

    let reason = tokio::select! {
        biased;
        reason = connection.closed() => reason,
        outcome = receive_messages(&connection) => {
            if let Err(error) = outcome {
                eprintln!("Message from {address} failed: {error:#}");
                connection.close(1u32.into(), b"Invalid message");
            }
            connection.closed().await
        }
    };
    println!("Client disconnected: {address} ({reason})");
}

async fn receive_messages(connection: &Connection) -> anyhow::Result<()> {
    loop {
        let message = protocol::receive_message(connection).await?;
        println!("{}: {:?}", connection.remote_address(), message.text());
        message.acknowledge().await?;
    }
}
