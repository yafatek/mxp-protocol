//! QUIC connection wrapper for MXP

use quinn::{Connection as QuinnConnection, RecvStream, SendStream};

use crate::protocol::{Error, Message, Result};

/// MXP connection over QUIC
pub struct Connection {
    inner: QuinnConnection,
}

impl Connection {
    /// Create a new connection wrapper
    #[must_use]
    pub(super) const fn new(inner: QuinnConnection) -> Self {
        Self { inner }
    }

    /// Send a message over a new unidirectional stream
    ///
    /// # Errors
    ///
    /// Returns error if unable to open stream or send message
    pub async fn send(&self, message: &Message) -> Result<()> {
        let mut stream = self
            .inner
            .open_uni()
            .await
            .map_err(|e| Error::Connection(format!("Failed to open stream: {e}")))?;

        let bytes = message.encode();

        stream
            .write_all(&bytes)
            .await
            .map_err(|e| Error::Stream(format!("Write failed: {e}")))?;

        stream
            .finish()
            .map_err(|e| Error::Stream(format!("Finish failed: {e}")))?;

        Ok(())
    }

    /// Receive a message from a unidirectional stream
    ///
    /// # Errors
    ///
    /// Returns error if unable to accept stream or read message
    pub async fn recv(&self) -> Result<Option<Message>> {
        match self.inner.accept_uni().await {
            Ok(stream) => {
                let message = Self::read_message(stream).await?;
                Ok(Some(message))
            }
            Err(e) => Err(Error::Connection(format!("Accept stream failed: {e}"))),
        }
    }

    /// Open a bidirectional stream
    ///
    /// # Errors
    ///
    /// Returns error if unable to open stream
    pub async fn open_bi(&self) -> Result<(SendStream, RecvStream)> {
        self.inner
            .open_bi()
            .await
            .map_err(|e| Error::Connection(format!("Failed to open bidirectional stream: {e}")))
    }

    /// Accept a bidirectional stream
    ///
    /// # Errors
    ///
    /// Returns error if unable to accept stream
    pub async fn accept_bi(&self) -> Result<Option<(SendStream, RecvStream)>> {
        match self.inner.accept_bi().await {
            Ok(streams) => Ok(Some(streams)),
            Err(e) => Err(Error::Connection(format!("Accept bi stream failed: {e}"))),
        }
    }

    /// Send a message and wait for response (call/response pattern)
    ///
    /// # Errors
    ///
    /// Returns error if unable to send/receive
    pub async fn call(&self, request: &Message) -> Result<Message> {
        let (mut send, recv) = self.open_bi().await?;

        // Send request
        let bytes = request.encode();
        send.write_all(&bytes)
            .await
            .map_err(|e| Error::Stream(format!("Write failed: {e}")))?;
        send.finish()
            .map_err(|e| Error::Stream(format!("Finish failed: {e}")))?;

        // Receive response
        Self::read_message(recv).await
    }

    /// Read a complete message from a stream
    async fn read_message(mut stream: RecvStream) -> Result<Message> {
        // Read all bytes from stream
        let bytes = stream
            .read_to_end(16 * 1024 * 1024) // 16 MB max
            .await
            .map_err(|e| Error::Stream(format!("Read failed: {e}")))?;

        // Decode message
        Message::decode(&bytes)
    }

    /// Get remote address
    #[must_use]
    pub fn remote_address(&self) -> std::net::SocketAddr {
        self.inner.remote_address()
    }

    /// Close the connection
    pub fn close(&self, error_code: u32, reason: &[u8]) {
        self.inner.close(error_code.into(), reason);
    }

    /// Check if connection is closed
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.inner.close_reason().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MessageType, transport::Endpoint};

    async fn setup_test_connection() -> Result<(Connection, Connection)> {
        // Install crypto provider for tests
        let _ = rustls::crypto::ring::default_provider().install_default();

        // Generate self-signed certificate for testing
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let cert_der = cert.cert.der().clone();
        let key_der =
            rustls::pki_types::PrivateKeyDer::try_from(cert.signing_key.serialize_der()).unwrap();

        // Create server
        let server_addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server = Endpoint::server(server_addr, cert_der, key_der)?;
        let server_addr = server.local_addr().unwrap();

        // Create client
        let client_addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
        let client = Endpoint::client(client_addr)?;

        // Connect
        let client_conn = client.connect(server_addr, "localhost").await?;

        // Accept
        let server_conn = server.accept().await?.unwrap();

        Ok((client_conn, server_conn))
    }

    #[tokio::test]
    #[ignore = "Requires network setup, run manually with: cargo test -- --ignored"]
    async fn test_send_recv() {
        let (client, server) = setup_test_connection().await.unwrap();

        let message = Message::new(MessageType::Event, b"test message");

        // Send from client
        tokio::spawn(async move {
            client.send(&message).await.unwrap();
        });

        // Receive on server
        let received = server.recv().await.unwrap().unwrap();
        assert_eq!(received.payload().as_ref(), b"test message");
    }

    #[tokio::test]
    #[ignore = "Requires network setup, run manually with: cargo test -- --ignored"]
    async fn test_call_response() {
        let (client, server) = setup_test_connection().await.unwrap();

        let request = Message::new(MessageType::Call, b"ping");

        // Handle request on server
        tokio::spawn(async move {
            let (mut send, mut recv) = server.accept_bi().await.unwrap().unwrap();
            let req_bytes = recv.read_to_end(1024).await.unwrap();
            let req = Message::decode(&req_bytes).unwrap();

            assert_eq!(req.payload().as_ref(), b"ping");

            // Send response
            let response = Message::new(MessageType::Response, b"pong");
            let resp_bytes = response.encode();
            send.write_all(&resp_bytes).await.unwrap();
            send.finish().unwrap();
        });

        // Call from client
        let response = client.call(&request).await.unwrap();
        assert_eq!(response.payload().as_ref(), b"pong");
    }
}
