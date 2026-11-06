use std::path::Path;

use anyhow::Context;
use quinn::rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};

use super::settings::ServerSettings;

pub(super) fn build_server_config(
    settings: &ServerSettings,
) -> anyhow::Result<quinn::ServerConfig> {
    let certificates = load_certificate_chain(&settings.cert_path)?;
    let private_key = load_private_key(&settings.key_path)?;

    quinn::ServerConfig::with_single_cert(certificates, private_key)
        .context("Invalid server certificate or private key")
}

fn load_certificate_chain(path: &Path) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    CertificateDer::pem_file_iter(path)
        .context("Cannot open certificate file")?
        .collect::<Result<Vec<_>, _>>()
        .context("Cannot parse certificate chain")
}

fn load_private_key(path: &Path) -> anyhow::Result<PrivateKeyDer<'static>> {
    PrivateKeyDer::from_pem_file(path).context("Cannot read or parse private key")
}
