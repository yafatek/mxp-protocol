//! Minimal UDP socket wrapper for MXP transport.

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

/// Error type for socket operations.
#[derive(Debug)]
pub enum SocketError {
    /// Underlying I/O error
    Io(io::Error),
}

impl From<io::Error> for SocketError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// Binding for a UDP socket.
#[derive(Debug, Clone)]
pub struct SocketBinding {
    socket: Arc<UdpSocket>,
}

impl SocketBinding {
    /// Bind to the provided address.
    pub fn bind(addr: SocketAddr) -> Result<Self, SocketError> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(false)?;
        Ok(Self {
            socket: Arc::new(socket),
        })
    }

    /// Set socket read timeout.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<(), SocketError> {
        self.socket.set_read_timeout(timeout)?;
        Ok(())
    }

    /// Set socket write timeout.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> Result<(), SocketError> {
        self.socket.set_write_timeout(timeout)?;
        Ok(())
    }

    /// Adjust the non-blocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<(), SocketError> {
        self.socket.set_nonblocking(nonblocking)?;
        Ok(())
    }

    /// Send bytes to a remote address.
    pub fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize, SocketError> {
        Ok(self.socket.send_to(buf, addr)?)
    }

    /// Receive bytes into the provided buffer.
    pub fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), SocketError> {
        Ok(self.socket.recv_from(buf)?)
    }

    /// Access the local address for this binding.
    pub fn local_addr(&self) -> Result<SocketAddr, SocketError> {
        Ok(self.socket.local_addr()?)
    }
}

