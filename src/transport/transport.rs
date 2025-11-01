//! High-level transport facade built on the MXP custom transport stack.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use super::buffer::{Buffer, BufferPool};
use super::error::TransportError;
use super::packet::PacketFlags;
use super::packet_crypto::{DecryptedPacket, PacketCipher};
use super::socket::{SocketBinding, SocketError};

/// Transport configuration options.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Size of each reusable buffer in bytes.
    pub buffer_size: usize,
    /// Maximum number of buffers maintained by the pool.
    pub max_buffers: usize,
    /// Optional read timeout for sockets.
    pub read_timeout: Option<Duration>,
    /// Optional write timeout for sockets.
    pub write_timeout: Option<Duration>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            buffer_size: 2048,
            max_buffers: 1024,
            read_timeout: None,
            write_timeout: None,
        }
    }
}

/// Handle used by callers to interact with the transport.
#[derive(Clone, Debug)]
pub struct TransportHandle {
    inner: Arc<TransportInner>,
}

#[derive(Debug)]
struct TransportInner {
    socket: SocketBinding,
    buffers: BufferPool,
}

impl TransportHandle {
    /// Acquire a reusable buffer for outbound or inbound data.
    pub fn acquire_buffer(&self) -> Buffer {
        self.inner.buffers.acquire()
    }

    /// Send data to the specified remote address.
    pub fn send(&self, buffer: &[u8], addr: SocketAddr) -> Result<usize, SocketError> {
        self.inner.socket.send_to(buffer, addr)
    }

    /// Receive data into the provided buffer (blocking call).
    pub fn receive(&self, buffer: &mut Buffer) -> Result<(usize, SocketAddr), SocketError> {
        let raw = buffer.as_mut_slice();
        let (len, addr) = self.inner.socket.recv_from(raw)?;
        buffer.set_len(len);
        Ok((len, addr))
    }

    /// Seal and send an encrypted packet using the provided cipher state.
    pub fn send_packet(
        &self,
        cipher: &mut PacketCipher,
        conn_id: u64,
        flags: PacketFlags,
        payload: &[u8],
        addr: SocketAddr,
        buffer: &mut Buffer,
    ) -> Result<u64, TransportError> {
        buffer.reset();
        let (packet_number, total_len) =
            cipher.seal_into(conn_id, flags, payload, buffer.as_mut_slice())?;
        buffer.set_len(total_len);
        self.inner
            .socket
            .send_to(buffer.as_slice(), addr)
            .map_err(TransportError::from)?;
        Ok(packet_number)
    }

    /// Receive and decrypt a packet into plaintext payload using the provided cipher.
    pub fn receive_packet(
        &self,
        cipher: &mut PacketCipher,
        buffer: &mut Buffer,
    ) -> Result<(DecryptedPacket, SocketAddr), TransportError> {
        buffer.reset();
        let (len, addr) = self
            .inner
            .socket
            .recv_from(buffer.as_mut_slice())
            .map_err(TransportError::from)?;
        buffer.set_len(len);
        let packet = buffer.as_slice();
        let decrypted = cipher.open(packet)?;
        Ok((decrypted, addr))
    }

    /// Expose the local socket address.
    pub fn local_addr(&self) -> Result<SocketAddr, SocketError> {
        self.inner.socket.local_addr()
    }
}

/// Transport builder responsible for binding sockets and configuring resources.
#[derive(Debug)]
pub struct Transport {
    config: TransportConfig,
    pool: BufferPool,
}

impl Transport {
    /// Create a new transport with the given configuration.
    pub fn new(config: TransportConfig) -> Self {
        let pool = BufferPool::new(config.buffer_size, config.max_buffers);
        Self { config, pool }
    }

    /// Bind an endpoint on the provided address.
    pub fn bind(&self, addr: SocketAddr) -> Result<TransportHandle, SocketError> {
        let socket = SocketBinding::bind(addr)?;
        if let Some(timeout) = self.config.read_timeout {
            socket.set_read_timeout(Some(timeout))?;
        }
        if let Some(timeout) = self.config.write_timeout {
            socket.set_write_timeout(Some(timeout))?;
        }
        Ok(self.build_handle(socket))
    }

    fn build_handle(&self, socket: SocketBinding) -> TransportHandle {
        let buffers = self.pool.clone();
        TransportHandle {
            inner: Arc::new(TransportInner { socket, buffers }),
        }
    }
}

impl Default for Transport {
    fn default() -> Self {
        Self::new(TransportConfig::default())
    }
}
