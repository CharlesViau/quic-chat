use std::net::SocketAddr;

use anyhow::Context;
use quinn::{Endpoint, Incoming};
use tokio::task::{JoinError, JoinSet};

use super::{session, settings::ServerSettings, tls};

const MAX_CONNECTIONS: usize = 64;

pub(super) struct ChatServer {
    endpoint: Endpoint,
    connections: JoinSet<()>,
}

impl ChatServer {
    pub(super) fn bind(settings: &ServerSettings) -> anyhow::Result<Self> {
        let server_config = tls::build_server_config(settings)?;
        let endpoint = Endpoint::server(server_config, settings.listen_addr)
            .with_context(|| format!("Cannot listen on {}", settings.listen_addr))?;

        Ok(Self {
            endpoint,
            connections: JoinSet::new(),
        })
    }

    pub(super) fn local_addr(&self) -> anyhow::Result<SocketAddr> {
        self.endpoint
            .local_addr()
            .context("Cannot read the listening address")
    }

    pub(super) async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                // Reclaim finished tasks before checking capacity for new arrivals.
                biased;
                Some(result) = self.connections.join_next(), if !self.connections.is_empty() => {
                    Self::report_task_result(result);
                }
                incoming = self.endpoint.accept() => {
                    let incoming = incoming.context("QUIC endpoint closed unexpectedly")?;
                    self.start_connection(incoming);
                }
            }

            // Keep shutdown responsive even when incoming attempts are always ready.
            tokio::task::coop::consume_budget().await;
        }
    }

    pub(super) async fn shutdown(mut self) {
        self.endpoint.close(0u32.into(), b"Server shutting down");
        self.wait_for_connections().await;
        self.endpoint.wait_idle().await;
    }

    fn start_connection(&mut self, incoming: Incoming) {
        if self.connections.len() >= MAX_CONNECTIONS {
            incoming.refuse();
            return;
        }

        self.connections.spawn(session::run(incoming));
    }

    fn report_task_result(result: Result<(), JoinError>) {
        if let Err(error) = result {
            eprintln!("Connection task failed: {error}");
        }
    }

    async fn wait_for_connections(&mut self) {
        while let Some(result) = self.connections.join_next().await {
            Self::report_task_result(result);
        }
    }
}

#[cfg(test)]
#[path = "chat_server_tests.rs"]
mod tests;
