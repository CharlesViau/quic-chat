use std::{future::Future, task::Poll, time::Duration};

use anyhow::Context;
use quinn::{
    Connection, ConnectionError, Endpoint, ServerConfig, rustls::pki_types::PrivatePkcs8KeyDer,
};
use tempfile::TempDir;
use tokio::{sync::mpsc, time::timeout};

use super::ChatClient;
use crate::settings::ClientSettings;

const TEST_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn typed_lines_arrive_in_order_and_eof_waits_for_the_last_acknowledgement()
-> anyhow::Result<()> {
    let server = TestServer::start()?;
    let mut client = ChatClient::bind(&server.settings)?;
    let (input, mut messages) = mpsc::channel(4);
    let expected = ["Hello", "Bonjour 🌍", "  keep these spaces  "];
    for message in expected {
        input.send(Ok(message.to_string())).await?;
    }
    drop(input);

    let (client_outcome, server_outcome) = timeout(TEST_TIMEOUT, async {
        tokio::join!(client.run(&mut messages), async {
            let connection = server.accept().await?;
            let mut received = Vec::new();
            for _ in expected {
                let message = quic_chat::protocol::receive_message(&connection).await?;
                received.push(message.text().to_string());
                message.acknowledge().await?;
            }
            Ok::<_, anyhow::Error>((connection, received))
        })
    })
    .await
    .context("Typed messages and EOF did not finish")?;
    client_outcome?;
    let (_connection, received) = server_outcome?;
    assert_eq!(received, expected);
    timeout(TEST_TIMEOUT, client.shutdown()).await?;
    Ok(())
}

#[tokio::test]
async fn eof_without_messages_exits_without_waiting_for_server_shutdown() -> anyhow::Result<()> {
    let server = TestServer::start()?;
    let mut client = ChatClient::bind(&server.settings)?;
    let (input, mut messages) = mpsc::channel(1);
    drop(input);

    let (client_outcome, connection) = timeout(TEST_TIMEOUT, async {
        tokio::join!(client.run(&mut messages), server.accept())
    })
    .await?;
    client_outcome?;
    let _connection = connection?;
    timeout(TEST_TIMEOUT, client.shutdown()).await?;
    Ok(())
}

#[tokio::test]
async fn input_failures_are_returned_to_the_caller() -> anyhow::Result<()> {
    let server = TestServer::start()?;
    let mut client = ChatClient::bind(&server.settings)?;
    let (input, mut messages) = mpsc::channel(1);
    input
        .send(Err(std::io::Error::other("input device failed")))
        .await?;

    let (client_outcome, connection) = timeout(TEST_TIMEOUT, async {
        tokio::join!(client.run(&mut messages), server.accept())
    })
    .await?;
    let error = client_outcome.expect_err("Input failures must be reported");
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().to_string(),
        "input device failed"
    );
    let _connection = connection?;
    timeout(TEST_TIMEOUT, client.shutdown()).await?;
    Ok(())
}

struct TestServer {
    endpoint: Endpoint,
    settings: ClientSettings,
    _directory: TempDir,
}

impl TestServer {
    fn start() -> anyhow::Result<Self> {
        let directory = tempfile::tempdir()?;
        let identity = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
        let ca_cert_path = directory.path().join("ca.pem");
        std::fs::write(&ca_cert_path, identity.cert.pem())?;
        let private_key = PrivatePkcs8KeyDer::from(identity.signing_key.serialize_der());
        let config =
            ServerConfig::with_single_cert(vec![identity.cert.der().clone()], private_key.into())?;
        let endpoint = Endpoint::server(config, "127.0.0.1:0".parse()?)?;
        let settings = ClientSettings {
            server_addr: endpoint.local_addr()?,
            server_name: "localhost".to_string(),
            ca_cert_path,
        };

        Ok(Self {
            endpoint,
            settings,
            _directory: directory,
        })
    }

    async fn accept(&self) -> anyhow::Result<Connection> {
        self.endpoint
            .accept()
            .await
            .context("Test server stopped accepting connections")?
            .await
            .context("Test server handshake failed")
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.endpoint.close(0u32.into(), b"Test finished");
    }
}

#[tokio::test]
async fn a_trusted_server_can_connect_and_close_the_client_cleanly() -> anyhow::Result<()> {
    let server = TestServer::start()?;
    let mut client = ChatClient::bind(&server.settings)?;
    let (_input, mut messages) = mpsc::channel(1);

    let (client_outcome, server_outcome) = timeout(TEST_TIMEOUT, async {
        tokio::join!(client.run(&mut messages), async {
            let connection = server.accept().await?;
            connection.close(0u32.into(), b"Server shutting down");
            Ok::<_, anyhow::Error>(())
        })
    })
    .await
    .context("Trusted connection and server shutdown did not finish")?;

    timeout(TEST_TIMEOUT, client.shutdown())
        .await
        .context("Client cleanup did not finish")?;
    client_outcome?;
    server_outcome?;
    Ok(())
}

#[tokio::test]
async fn cancelling_the_client_run_still_allows_a_clean_shutdown() -> anyhow::Result<()> {
    let server = TestServer::start()?;
    let mut client = ChatClient::bind(&server.settings)?;
    let (_input, mut messages) = mpsc::channel(1);
    let mut running = Box::pin(client.run(&mut messages));
    let connection = timeout(TEST_TIMEOUT, async {
        tokio::select! {
            outcome = &mut running => anyhow::bail!("Client stopped before cancellation: {outcome:?}"),
            connection = server.accept() => connection,
        }
    })
    .await
    .context("Server did not receive a client connection")??;

    // The server has completed the handshake. Poll the client to retain that
    // connection before cancelling its wait for disconnection, as Ctrl+C does.
    std::future::poll_fn(|context| match running.as_mut().poll(context) {
        Poll::Pending => Poll::Ready(Ok(())),
        Poll::Ready(outcome) => Poll::Ready(Err(anyhow::anyhow!(
            "Client stopped before cancellation: {outcome:?}"
        ))),
    })
    .await?;
    drop(running);

    let (shutdown, closed) = tokio::join!(
        timeout(TEST_TIMEOUT, client.shutdown()),
        timeout(TEST_TIMEOUT, connection.closed()),
    );
    shutdown.context("Client shutdown did not finish")?;
    let reason = closed.context("Server did not observe client shutdown")?;
    match reason {
        ConnectionError::ApplicationClosed(close) => {
            assert_eq!(close.error_code, 0u32.into());
            assert_eq!(close.reason.as_ref(), b"Client shutting down");
        }
        other => anyhow::bail!("Expected explicit client shutdown, got {other}"),
    }
    Ok(())
}

#[tokio::test]
async fn a_server_application_error_is_preserved_for_the_caller() -> anyhow::Result<()> {
    let server = TestServer::start()?;
    let mut client = ChatClient::bind(&server.settings)?;
    let (_input, mut messages) = mpsc::channel(1);

    let (client_outcome, server_outcome) = timeout(TEST_TIMEOUT, async {
        tokio::join!(client.run(&mut messages), async {
            let connection = server.accept().await?;
            connection.close(42u32.into(), b"Chat service unavailable");
            Ok::<_, anyhow::Error>(())
        })
    })
    .await
    .context("Server error did not reach the client")?;
    timeout(TEST_TIMEOUT, client.shutdown())
        .await
        .context("Client cleanup did not finish")?;
    server_outcome?;

    let error = client_outcome.expect_err("A nonzero server close code must fail the client");
    match error.downcast_ref::<ConnectionError>() {
        Some(ConnectionError::ApplicationClosed(close)) => {
            assert_eq!(close.error_code, 42u32.into());
            assert_eq!(close.reason.as_ref(), b"Chat service unavailable");
        }
        other => anyhow::bail!("Expected the server application error, got {other:?}"),
    }
    Ok(())
}

#[tokio::test]
async fn a_pending_handshake_can_be_cancelled_and_shut_down() -> anyhow::Result<()> {
    let server = TestServer::start()?;
    let mut client = ChatClient::bind(&server.settings)?;
    let (_input, mut messages) = mpsc::channel(1);
    let mut running = Box::pin(client.run(&mut messages));
    let incoming = timeout(TEST_TIMEOUT, async {
        tokio::select! {
            outcome = &mut running => anyhow::bail!("Client stopped before cancellation: {outcome:?}"),
            incoming = server.endpoint.accept() => incoming.context("Server stopped accepting"),
        }
    })
    .await
    .context("Server did not receive the handshake attempt")??;

    // Hold Incoming without accepting it, so the handshake cannot complete.
    drop(running);
    timeout(TEST_TIMEOUT, client.shutdown())
        .await
        .context("Cancelling a pending handshake did not release the client")?;
    drop(incoming);
    Ok(())
}

#[tokio::test]
async fn a_certificate_for_a_different_server_name_is_rejected() -> anyhow::Result<()> {
    let mut server = TestServer::start()?;
    server.settings.server_name = "wrong.example".to_string();
    assert_certificate_rejected(&server).await
}

#[tokio::test]
async fn a_server_signed_by_an_untrusted_authority_is_rejected() -> anyhow::Result<()> {
    let server = TestServer::start()?;
    let unrelated_identity = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
    std::fs::write(&server.settings.ca_cert_path, unrelated_identity.cert.pem())?;
    assert_certificate_rejected(&server).await
}

async fn assert_certificate_rejected(server: &TestServer) -> anyhow::Result<()> {
    let mut client = ChatClient::bind(&server.settings)?;
    let (_input, mut messages) = mpsc::channel(1);
    let (client_outcome, server_outcome) = timeout(TEST_TIMEOUT, async {
        tokio::join!(client.run(&mut messages), server.accept())
    })
    .await
    .context("Certificate rejection did not finish")?;
    timeout(TEST_TIMEOUT, client.shutdown())
        .await
        .context("Rejected client cleanup did not finish")?;

    let error = client_outcome.expect_err("Client must reject the server certificate");
    assert!(
        matches!(error.downcast_ref::<ConnectionError>(),
            Some(ConnectionError::TransportError(error))
                if (0x100..0x200).contains(&u64::from(error.code))),
        "Expected a TLS handshake failure, got {error:#}"
    );
    assert!(server_outcome.is_err(), "Server handshake must not succeed");
    Ok(())
}

#[tokio::test]
async fn an_empty_ca_file_is_rejected_before_connecting() -> anyhow::Result<()> {
    let server = TestServer::start()?;
    std::fs::write(&server.settings.ca_cert_path, "")?;
    assert!(ChatClient::bind(&server.settings).is_err());
    Ok(())
}

#[tokio::test]
async fn a_malformed_ca_file_is_rejected_before_connecting() -> anyhow::Result<()> {
    let server = TestServer::start()?;
    std::fs::write(
        &server.settings.ca_cert_path,
        "-----BEGIN CERTIFICATE-----\nnot-valid-base64!\n-----END CERTIFICATE-----\n",
    )?;
    assert!(ChatClient::bind(&server.settings).is_err());
    Ok(())
}
