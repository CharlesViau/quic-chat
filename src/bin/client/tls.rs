use std::{path::Path, sync::Arc, time::Duration};

use anyhow::{Context, ensure};
use quinn::{
    ClientConfig, TransportConfig,
    rustls::{
        RootCertStore,
        pki_types::{CertificateDer, pem::PemObject},
    },
};

use super::settings::ClientSettings;

const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(5);

pub(super) fn build_client_config(settings: &ClientSettings) -> anyhow::Result<ClientConfig> {
    let roots = load_root_certificates(&settings.ca_cert_path)?;
    let mut config = ClientConfig::with_root_certificates(Arc::new(roots))
        .context("Cannot configure client certificate verification")?;
    config.transport_config(Arc::new(build_transport_config()));

    Ok(config)
}

fn load_root_certificates(path: &Path) -> anyhow::Result<RootCertStore> {
    let certificates = CertificateDer::pem_file_iter(path)
        .with_context(|| format!("Cannot open CA certificate file {}", path.display()))?;
    let mut roots = RootCertStore::empty();

    for certificate in certificates {
        let certificate = certificate.context("Cannot parse CA certificate PEM")?;
        roots.add(certificate).context("Invalid CA certificate")?;
    }
    ensure!(
        !roots.is_empty(),
        "CA certificate file contains no certificates"
    );

    Ok(roots)
}

fn build_transport_config() -> TransportConfig {
    let mut config = TransportConfig::default();
    config.keep_alive_interval(Some(KEEP_ALIVE_INTERVAL));

    config
}
