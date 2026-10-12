//! A throwaway certificate authority standing in for Riot's root.
//!
//! The real client and game serve a certificate signed by Riot's root whose
//! name does not match `127.0.0.1`. The mock does the same (its leaf is named
//! `rclient`), so the app's "trust only this root, tolerate the name" path is
//! what gets exercised. A new key pair is made on every start and never
//! written anywhere except the root's public PEM.

use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::sync::Arc;

/// The server side of the mock's TLS, plus the root the app must trust.
#[derive(Debug)]
pub struct TestPki {
    /// PEM of the mock root; give it to the app as its "Riot" root.
    pub root_pem: String,
    pub server_config: Arc<rustls::ServerConfig>,
}

impl TestPki {
    pub fn generate() -> Result<Self, String> {
        let err = |e: rcgen::Error| e.to_string();

        let mut root = CertificateParams::new(Vec::<String>::new()).map_err(err)?;
        root.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        root.distinguished_name
            .push(DnType::CommonName, "Mock League Test Root (not Riot)");
        root.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let root_key = KeyPair::generate().map_err(err)?;
        let root_cert = root.self_signed(&root_key).map_err(err)?;
        let root_pem = root_cert.pem();
        let issuer = Issuer::new(root, root_key);

        // Named like the real client's certificate, so it does not match 127.0.0.1.
        let mut leaf = CertificateParams::new(vec!["rclient".to_owned()]).map_err(err)?;
        leaf.distinguished_name.push(DnType::CommonName, "rclient");
        leaf.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let leaf_key = KeyPair::generate().map_err(err)?;
        let leaf_cert = leaf.signed_by(&leaf_key, &issuer).map_err(err)?;

        let chain = vec![CertificateDer::from(leaf_cert.der().to_vec())];
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der()));
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let server_config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| e.to_string())?
            .with_no_client_auth()
            .with_single_cert(chain, key)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            root_pem,
            server_config: Arc::new(server_config),
        })
    }
}
