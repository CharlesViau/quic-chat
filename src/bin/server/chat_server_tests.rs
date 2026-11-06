use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use quinn::{ClientConfig, Connection, ConnectionError, Endpoint, rustls::RootCertStore};
use tokio::{sync::oneshot, task::JoinHandle, time::timeout};

use super::{ChatServer, MAX_CONNECTIONS};
use crate::settings::ServerSettings;

const TEST_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn a_client_can_send_multiple_messages_on_the_same_connection() -> anyhow::Result<()> {
    let mut server = RunningServer::start()?;
    let connection = server.connect("localhost").await??;
    for message in ["Hello", "Bonjour 🌍", "  spaces stay  "] {
        timeout(
            TEST_TIMEOUT,
            quic_chat::protocol::send_message(&connection, message),
        )
        .await
        .context("Server did not acknowledge the message")??;
    }
    assert!(connection.close_reason().is_none());
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn invalid_message_closes_only_the_offending_client() -> anyhow::Result<()> {
    let mut server = RunningServer::start()?;
    let invalid = server.connect("localhost").await??;
    let healthy = server.connect("localhost").await??;
    let (mut send, _reply) = timeout(TEST_TIMEOUT, invalid.open_bi()).await??;
    send.write_all(&[0xff]).await?;
    send.finish()?;

    let reason = timeout(TEST_TIMEOUT, invalid.closed()).await?;
    assert!(
        matches!(reason, ConnectionError::ApplicationClosed(close) if close.error_code == 1u32.into())
    );
    timeout(
        TEST_TIMEOUT,
        quic_chat::protocol::send_message(&healthy, "Still here"),
    )
    .await??;
    server.shutdown().await?;
    Ok(())
}

struct RunningServer {
    address: SocketAddr,
    client: Endpoint,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<anyhow::Result<()>>,
}

impl RunningServer {
    fn start() -> anyhow::Result<Self> {
        let directory = tempfile::tempdir()?;
        let identity = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
        let settings = ServerSettings {
            cert_path: directory.path().join("cert.pem"),
            key_path: directory.path().join("key.pem"),
            listen_addr: "127.0.0.1:0".parse()?,
        };
        std::fs::write(&settings.cert_path, identity.cert.pem())?;
        std::fs::write(&settings.key_path, identity.signing_key.serialize_pem())?;

        let mut server = ChatServer::bind(&settings)?;
        let address = server.local_addr()?;

        let mut roots = RootCertStore::empty();
        roots.add(identity.cert.der().clone())?;
        let mut client = Endpoint::client("127.0.0.1:0".parse()?)?;
        client.set_default_client_config(ClientConfig::with_root_certificates(Arc::new(roots))?);

        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            let outcome = tokio::select! {
                outcome = server.run() => outcome,
                outcome = stopped => outcome.context("Test shutdown sender disappeared"),
            };
            server.shutdown().await;
            outcome
        });

        Ok(Self {
            address,
            client,
            stop: Some(stop),
            task,
        })
    }

    async fn connect(&self, name: &str) -> anyhow::Result<Result<Connection, ConnectionError>> {
        let connecting = self.client.connect(self.address, name)?;
        timeout(TEST_TIMEOUT, connecting)
            .await
            .context("Client handshake did not finish")
    }

    async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.stop
            .take()
            .context("Test server already stopped")?
            .send(())
            .map_err(|_| anyhow::anyhow!("Server stopped before receiving shutdown"))?;
        let outcome = timeout(TEST_TIMEOUT, &mut self.task)
            .await
            .context("Server shutdown did not finish")?
            .context("Server task failed")?;
        outcome
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.client.close(0u32.into(), b"Test finished");
        self.task.abort();
    }
}

#[tokio::test]
async fn idle_clients_connect_concurrently_and_receive_server_shutdown() -> anyhow::Result<()> {
    let mut server = RunningServer::start()?;
    let first = server.connect("localhost").await??;
    let second = server.connect("localhost").await??;

    server.shutdown().await?;

    for connection in [first, second] {
        let reason = timeout(TEST_TIMEOUT, connection.closed()).await?;
        match reason {
            ConnectionError::ApplicationClosed(close) => {
                assert_eq!(close.reason.as_ref(), b"Server shutting down");
            }
            ConnectionError::ConnectionClosed(close) => {
                // QUIC uses a transport close if shutdown races the server's handshake.
                assert_eq!(
                    close.error_code,
                    quinn::TransportErrorCode::APPLICATION_ERROR
                );
            }
            other => anyhow::bail!("Expected server shutdown, got {other}"),
        }
    }
    Ok(())
}

#[tokio::test]
async fn a_failed_handshake_does_not_stop_other_clients() -> anyhow::Result<()> {
    let mut server = RunningServer::start()?;
    let rejected = server.connect("wrong.example").await?;
    assert!(
        rejected.is_err(),
        "An incorrect server name must be rejected"
    );

    let connection = server.connect("localhost").await??;
    assert!(connection.close_reason().is_none());
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn connection_limit_refuses_excess_clients_and_reuses_released_capacity() -> anyhow::Result<()>
{
    let mut server = RunningServer::start()?;
    let mut connections = Vec::new();
    for _ in 0..MAX_CONNECTIONS {
        connections.push(server.connect("localhost").await??);
    }

    let rejected = server.connect("localhost").await?;
    assert!(matches!(
        rejected,
        Err(ConnectionError::ConnectionClosed(close))
            if close.error_code == quinn::TransportErrorCode::CONNECTION_REFUSED
    ));

    let departed = connections.pop().context("No connected client to close")?;
    departed.close(0u32.into(), b"Free capacity");

    let replacement = timeout(TEST_TIMEOUT, async {
        loop {
            match server.connect("localhost").await? {
                Ok(connection) => break Ok::<_, anyhow::Error>(connection),
                Err(ConnectionError::ConnectionClosed(close))
                    if close.error_code == quinn::TransportErrorCode::CONNECTION_REFUSED =>
                {
                    tokio::task::yield_now().await;
                }
                Err(error) => break Err(error.into()),
            }
        }
    })
    .await
    .context("Closed client did not release capacity")??;
    assert!(replacement.close_reason().is_none());

    server.shutdown().await?;
    Ok(())
}
