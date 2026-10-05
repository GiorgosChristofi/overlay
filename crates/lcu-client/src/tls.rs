//! TLS for the local Riot endpoints.
//!
//! The client and game serve certificates signed by Riot's own root
//! (<https://static.developer.riotgames.com/docs/lol/riotgames.pem>). We trust
//! only that root and verify the chain, but accept the certificate's host
//! name, because the local services present names that do not match
//! `127.0.0.1`. Connections are only ever made to `127.0.0.1`.

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::ring;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    CertificateError, ClientConfig, DigitallySignedStruct, Error, RootCertStore, SignatureScheme,
};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TlsError {
    #[error("Riot root certificate is missing or invalid: {0}")]
    BadRoot(String),
    #[error("could not build TLS config: {0}")]
    Config(#[from] Error),
}

/// Builds a client config that trusts only the Riot root certificate.
pub fn riot_client_config(root_pem: &[u8]) -> Result<Arc<ClientConfig>, TlsError> {
    let mut roots = RootCertStore::empty();
    for cert in CertificateDer::pem_slice_iter(root_pem) {
        let cert = cert.map_err(|e| TlsError::BadRoot(e.to_string()))?;
        roots.add(cert)?;
    }
    if roots.is_empty() {
        return Err(TlsError::BadRoot("no certificate found in PEM".into()));
    }
    let provider = Arc::new(ring::default_provider());
    let inner = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider.clone())
        .build()
        .map_err(|e| TlsError::BadRoot(e.to_string()))?;
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(RiotVerifier { inner }))
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// Verifies the chain against the Riot root; tolerates a host name mismatch only.
#[derive(Debug)]
struct RiotVerifier {
    inner: Arc<WebPkiServerVerifier>,
}

impl ServerCertVerifier for RiotVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        match self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        ) {
            Ok(ok) => Ok(ok),
            Err(Error::InvalidCertificate(
                CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. },
            )) => Ok(ServerCertVerified::assertion()),
            Err(e) => Err(e),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_pem() {
        assert!(matches!(riot_client_config(b""), Err(TlsError::BadRoot(_))));
    }
}
