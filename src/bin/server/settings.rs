use std::{net::SocketAddr, path::PathBuf};

use anyhow::Context;

const CERT_PATH_ENV: &str = "QUIC_CHAT_CERT_PATH";
const KEY_PATH_ENV: &str = "QUIC_CHAT_KEY_PATH";
const LISTEN_ADDR_ENV: &str = "QUIC_CHAT_LISTEN_ADDR";

pub(super) struct ServerSettings {
    pub(super) cert_path: PathBuf,
    pub(super) key_path: PathBuf,
    pub(super) listen_addr: SocketAddr,
}

pub(super) fn from_env() -> anyhow::Result<ServerSettings> {
    Ok(ServerSettings {
        cert_path: read_cert_path()?,
        key_path: read_key_path()?,
        listen_addr: read_listen_addr()?,
    })
}

fn read_cert_path() -> anyhow::Result<PathBuf> {
    let path =
        std::env::var_os(CERT_PATH_ENV).with_context(|| format!("{CERT_PATH_ENV} is not set"))?;

    Ok(PathBuf::from(path))
}

fn read_key_path() -> anyhow::Result<PathBuf> {
    let path =
        std::env::var_os(KEY_PATH_ENV).with_context(|| format!("{KEY_PATH_ENV} is not set"))?;

    Ok(PathBuf::from(path))
}

fn read_listen_addr() -> anyhow::Result<SocketAddr> {
    std::env::var(LISTEN_ADDR_ENV)
        .with_context(|| format!("Cannot read {LISTEN_ADDR_ENV}"))?
        .parse::<SocketAddr>()
        .with_context(|| format!("{LISTEN_ADDR_ENV} must contain an IP address and port"))
}
