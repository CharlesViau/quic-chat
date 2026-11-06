use std::{net::SocketAddr, path::PathBuf};

use anyhow::{Context, ensure};
use quinn::rustls::pki_types::ServerName;

const SERVER_ADDR_ENV: &str = "QUIC_CHAT_SERVER_ADDR";
const SERVER_NAME_ENV: &str = "QUIC_CHAT_SERVER_NAME";
const CA_CERT_PATH_ENV: &str = "QUIC_CHAT_CA_CERT_PATH";

pub(super) struct ClientSettings {
    pub(super) server_addr: SocketAddr,
    pub(super) server_name: String,
    pub(super) ca_cert_path: PathBuf,
}

pub(super) fn from_env() -> anyhow::Result<ClientSettings> {
    Ok(ClientSettings {
        server_addr: read_server_addr()?,
        server_name: read_server_name()?,
        ca_cert_path: read_ca_cert_path()?,
    })
}

fn read_server_addr() -> anyhow::Result<SocketAddr> {
    let address = std::env::var(SERVER_ADDR_ENV)
        .with_context(|| format!("Cannot read {SERVER_ADDR_ENV}"))?
        .parse::<SocketAddr>()
        .with_context(|| format!("{SERVER_ADDR_ENV} must contain an IP address and port"))?;
    ensure!(
        address.port() != 0,
        "{SERVER_ADDR_ENV} must use a nonzero server port"
    );

    Ok(address)
}

fn read_server_name() -> anyhow::Result<String> {
    let name =
        std::env::var(SERVER_NAME_ENV).with_context(|| format!("Cannot read {SERVER_NAME_ENV}"))?;
    ServerName::try_from(name.as_str()).with_context(|| {
        format!("{SERVER_NAME_ENV} must contain a valid DNS name or IP address")
    })?;

    Ok(name)
}

fn read_ca_cert_path() -> anyhow::Result<PathBuf> {
    let path = std::env::var_os(CA_CERT_PATH_ENV)
        .with_context(|| format!("{CA_CERT_PATH_ENV} is not set"))?;
    ensure!(!path.is_empty(), "{CA_CERT_PATH_ENV} must not be empty");

    Ok(PathBuf::from(path))
}
