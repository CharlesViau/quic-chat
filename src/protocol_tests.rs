use std::{sync::Arc, time::Duration};

use anyhow::Context;
use quinn::{
    ClientConfig, Connection, Endpoint, ServerConfig,
    rustls::{RootCertStore, pki_types::PrivatePkcs8KeyDer},
};
use tokio::time::timeout;

use super::*;

const TEST_TIMEOUT: Duration = Duration::from_secs(10);

struct Peers {
    client_endpoint: Endpoint,
    server_endpoint: Endpoint,
    client: Connection,
    server: Connection,
}

impl Peers {
    async fn connect() -> anyhow::Result<Self> {
        let identity = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
        let private_key = PrivatePkcs8KeyDer::from(identity.signing_key.serialize_der());
        let server_config =
            ServerConfig::with_single_cert(vec![identity.cert.der().clone()], private_key.into())?;
        let server_endpoint = Endpoint::server(server_config, "127.0.0.1:0".parse()?)?;
        let mut roots = RootCertStore::empty();
        roots.add(identity.cert.der().clone())?;
        let mut client_endpoint = Endpoint::client("127.0.0.1:0".parse()?)?;
        client_endpoint
            .set_default_client_config(ClientConfig::with_root_certificates(Arc::new(roots))?);
        let connecting = client_endpoint.connect(server_endpoint.local_addr()?, "localhost")?;

        let (client, server) = timeout(TEST_TIMEOUT, async {
            tokio::try_join!(
                async { connecting.await.context("Client handshake failed") },
                async {
                    server_endpoint
                        .accept()
                        .await
                        .context("Server stopped accepting")?
                        .await
                        .context("Server handshake failed")
                },
            )
        })
        .await
        .context("Loopback handshake timed out")??;

        Ok(Self {
            client_endpoint,
            server_endpoint,
            client,
            server,
        })
    }
}

impl Drop for Peers {
    fn drop(&mut self) {
        self.client_endpoint.close(0u32.into(), b"Test finished");
        self.server_endpoint.close(0u32.into(), b"Test finished");
    }
}

#[tokio::test]
async fn unicode_and_spaces_are_preserved_until_the_server_acknowledges() -> anyhow::Result<()> {
    let peers = Peers::connect().await?;
    let text = "  Bonjour, 世界!  ";

    timeout(TEST_TIMEOUT, async {
        tokio::try_join!(send_message(&peers.client, text), async {
            let received = receive_message(&peers.server).await?;
            assert_eq!(received.text(), text);
            received.acknowledge().await
        })
    })
    .await
    .context("Message exchange timed out")??;
    Ok(())
}

#[tokio::test]
async fn a_message_at_the_byte_limit_is_accepted() -> anyhow::Result<()> {
    let peers = Peers::connect().await?;
    let text = "a".repeat(MAX_MESSAGE_BYTES);

    timeout(TEST_TIMEOUT, async {
        tokio::try_join!(send_message(&peers.client, &text), async {
            let received = receive_message(&peers.server).await?;
            assert_eq!(received.text(), text);
            received.acknowledge().await
        })
    })
    .await??;
    Ok(())
}

#[tokio::test]
async fn invalid_utf8_is_rejected_without_a_success_acknowledgement() -> anyhow::Result<()> {
    let peers = Peers::connect().await?;
    let (raw_outcome, server_outcome) = timeout(TEST_TIMEOUT, async {
        tokio::join!(
            send_raw_message(&peers.client, &[0xff, 0xfe]),
            receive_message(&peers.server),
        )
    })
    .await?;

    let error = server_outcome.err().context("Invalid UTF-8 was accepted")?;
    assert!(error.downcast_ref::<std::string::FromUtf8Error>().is_some());
    assert!(
        raw_outcome?.is_empty(),
        "Invalid messages must not be acknowledged"
    );
    Ok(())
}

#[tokio::test]
async fn an_oversized_network_message_is_rejected_without_success() -> anyhow::Result<()> {
    let peers = Peers::connect().await?;
    let bytes = vec![b'a'; MAX_MESSAGE_BYTES + 1];
    let (raw_outcome, server_outcome) = timeout(TEST_TIMEOUT, async {
        tokio::join!(
            send_raw_message(&peers.client, &bytes),
            receive_message(&peers.server),
        )
    })
    .await?;

    let error = server_outcome
        .err()
        .context("Oversized message was accepted")?;
    assert!(matches!(
        error.downcast_ref::<quinn::ReadToEndError>(),
        Some(quinn::ReadToEndError::TooLong)
    ));
    assert!(
        raw_outcome?.is_empty(),
        "Oversized messages must not be acknowledged"
    );
    Ok(())
}

async fn send_raw_message(connection: &Connection, bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let (mut send, mut acknowledgement) = connection.open_bi().await?;
    send.write_all(bytes).await?;
    send.finish()?;
    Ok(acknowledgement.read_to_end(1).await?)
}

#[tokio::test]
async fn an_incorrect_acknowledgement_is_not_treated_as_delivery() -> anyhow::Result<()> {
    assert_invalid_acknowledgement(&[1]).await
}

#[tokio::test]
async fn a_missing_acknowledgement_is_not_treated_as_delivery() -> anyhow::Result<()> {
    assert_invalid_acknowledgement(&[]).await
}

async fn assert_invalid_acknowledgement(reply: &[u8]) -> anyhow::Result<()> {
    let peers = Peers::connect().await?;
    let (client_outcome, server_outcome) = timeout(TEST_TIMEOUT, async {
        tokio::join!(send_message(&peers.client, "Hello"), async {
            let (mut acknowledgement, mut receive) = peers.server.accept_bi().await?;
            assert_eq!(receive.read_to_end(MAX_MESSAGE_BYTES).await?, b"Hello");
            acknowledgement.write_all(reply).await?;
            acknowledgement.finish()?;
            Ok::<_, anyhow::Error>(())
        })
    })
    .await?;
    server_outcome?;
    let error = client_outcome.expect_err("An invalid acknowledgement must fail delivery");
    assert_eq!(error.to_string(), "Invalid message acknowledgement");
    Ok(())
}

#[tokio::test]
async fn oversized_outgoing_text_is_rejected_before_opening_a_stream() -> anyhow::Result<()> {
    let peers = Peers::connect().await?;
    peers.client.close(0u32.into(), b"Closed before validation");
    let text = "a".repeat(MAX_MESSAGE_BYTES + 1);

    let error = send_message(&peers.client, &text)
        .await
        .expect_err("Oversized outgoing text must be rejected");
    assert_eq!(
        error.to_string(),
        format!("Message exceeds {MAX_MESSAGE_BYTES} bytes")
    );
    Ok(())
}
