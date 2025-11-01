//! QUIC endpoint for MXP

use std::net::SocketAddr;
use std::sync::Arc;

use quinn::crypto::rustls::QuicClientConfig;
use quinn::crypto::rustls::QuicServerConfig;
use quinn::{ClientConfig, Endpoint as QuinnEndpoint, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::protocol::{Error, Result};

/// MXP endpoint (client or server)
pub struct Endpoint {
    inner: QuinnEndpoint,
}

impl Endpoint {
    /// Create a client endpoint
    ///
    /// # Errors
    ///
    /// Returns error if unable to bind to address
    pub fn client(bind_addr: SocketAddr) -> Result<Self> {
        let mut endpoint = QuinnEndpoint::client(bind_addr)
            .map_err(|e| Error::Connection(format!("Failed to create client endpoint: {e}")))?;

        // Configure client with insecure TLS for development
        // TODO: Add proper certificate validation for production
        let crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(SkipServerVerification::new())
            .with_no_client_auth();

        let mut client_config = ClientConfig::new(Arc::new(QuicClientConfig::try_from(crypto).unwrap()));
        let mut transport = quinn::TransportConfig::default();

        // Enable 0-RTT
        transport.max_idle_timeout(Some(std::time::Duration::from_secs(30).try_into().unwrap()));

        client_config.transport_config(Arc::new(transport));
        endpoint.set_default_client_config(client_config);

        Ok(Self { inner: endpoint })
    }

    /// Create a server endpoint
    ///
    /// # Errors
    ///
    /// Returns error if unable to bind or configure TLS
    pub fn server(
        bind_addr: SocketAddr,
        cert: CertificateDer<'static>,
        key: PrivateKeyDer<'static>,
    ) -> Result<Self> {
        let server_config = Self::configure_server(cert, key)?;

        let endpoint = QuinnEndpoint::server(server_config, bind_addr)
            .map_err(|e| Error::Connection(format!("Failed to create server endpoint: {e}")))?;

        Ok(Self { inner: endpoint })
    }

    /// Connect to a remote endpoint
    ///
    /// # Errors
    ///
    /// Returns error if connection fails
    pub async fn connect(&self, addr: SocketAddr, server_name: &str) -> Result<super::Connection> {
        let connection = self
            .inner
            .connect(addr, server_name)
            .map_err(|e| Error::Connection(format!("Connect failed: {e}")))?
            .await
            .map_err(|e| Error::Connection(format!("Connect await failed: {e}")))?;

        Ok(super::Connection::new(connection))
    }

    /// Accept an incoming connection
    ///
    /// # Errors
    ///
    /// Returns error if accept fails
    pub async fn accept(&self) -> Result<Option<super::Connection>> {
        match self.inner.accept().await {
            Some(connecting) => {
                let connection = connecting
                    .await
                    .map_err(|e| Error::Connection(format!("Accept failed: {e}")))?;
                Ok(Some(super::Connection::new(connection)))
            }
            None => Ok(None),
        }
    }

    /// Get local address
    #[must_use]
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.inner.local_addr().ok()
    }

    /// Close the endpoint
    pub fn close(&self) {
        self.inner.close(0u32.into(), b"endpoint closed");
    }

    fn configure_server(
        cert: CertificateDer<'static>,
        key: PrivateKeyDer<'static>,
    ) -> Result<ServerConfig> {
        let crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .map_err(|e| Error::Connection(format!("TLS config error: {e}")))?;

        let mut server_config = ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(crypto).unwrap()));
        let mut transport = quinn::TransportConfig::default();

        // Enable 0-RTT
        transport.max_idle_timeout(Some(std::time::Duration::from_secs(30).try_into().unwrap()));

        server_config.transport_config(Arc::new(transport));

        Ok(server_config)
    }
}

/// Development-only: Skip server certificate verification
///
/// **WARNING:** This is insecure and should only be used for development/testing
#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
    }
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_endpoint() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let endpoint = Endpoint::client(addr).unwrap();
        assert!(endpoint.local_addr().is_some());
    }
}

