use std::{
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
};

use anyhow::Context;
use quinn::{Connection, Endpoint};
use tokio::sync::mpsc::Receiver;

use super::{session, settings::ClientSettings, tls};

pub(super) struct ChatClient {
    endpoint: Endpoint,
    connection: Option<Connection>,
    server_addr: SocketAddr,
    server_name: String,
}

impl ChatClient {
    pub(super) fn bind(settings: &ClientSettings) -> anyhow::Result<Self> {
        Ok(Self {
            endpoint: Self::create_endpoint(settings)?,
            connection: None,
            server_addr: settings.server_addr,
            server_name: settings.server_name.clone(),
        })
    }

    pub(super) async fn run(
        &mut self,
        messages: &mut Receiver<io::Result<String>>,
    ) -> anyhow::Result<()> {
        let connection = self.connect().await?;
        let connection = self.connection.insert(connection);

        session::run(connection, messages).await
    }

    pub(super) async fn shutdown(self) {
        self.endpoint.close(0u32.into(), b"Client shutting down");
        self.endpoint.wait_idle().await;
    }

    fn create_endpoint(settings: &ClientSettings) -> anyhow::Result<Endpoint> {
        let config = tls::build_client_config(settings)?;
        let address = Self::local_bind_address(settings.server_addr);
        let mut endpoint = Endpoint::client(address)
            .with_context(|| format!("Cannot bind client endpoint on {address}"))?;
        endpoint.set_default_client_config(config);

        Ok(endpoint)
    }

    fn local_bind_address(server_addr: SocketAddr) -> SocketAddr {
        match server_addr {
            SocketAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
            SocketAddr::V6(_) => (Ipv6Addr::UNSPECIFIED, 0).into(),
        }
    }

    async fn connect(&self) -> anyhow::Result<Connection> {
        self.endpoint
            .connect(self.server_addr, &self.server_name)
            .context("Cannot start the QUIC connection")?
            .await
            .with_context(|| {
                format!(
                    "Cannot connect to {} as {}",
                    self.server_addr, self.server_name
                )
            })
    }
}

#[cfg(test)]
#[path = "chat_client_tests.rs"]
mod tests;
