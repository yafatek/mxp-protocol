//! Transport-level error types covering socket, packet, and crypto failures.

use super::crypto::CryptoError;
use super::packet::PacketError;
use super::socket::SocketError;
use core::fmt;

/// Unified error type for MXP transport operations.
#[derive(Debug)]
pub enum TransportError {
    /// Underlying socket failure.
    Socket(SocketError),
    /// Packet encoding/decoding failure.
    Packet(PacketError),
    /// Cryptographic failure (AEAD, HKDF, etc.).
    Crypto(CryptoError),
    /// Provided buffer was not large enough to hold the encoded packet.
    BufferTooSmall {
        /// Number of bytes required to encode the packet.
        required: usize,
        /// Number of bytes available in the supplied buffer.
        available: usize,
    },
    /// Payload length exceeds what can be encoded in a single packet.
    PayloadTooLarge {
        /// Length of the payload provided by the caller.
        len: usize,
        /// Maximum payload length supported by the transport.
        max: usize,
    },
    /// Packet was rejected as a replay.
    ReplayDetected {
        /// Packet number that triggered the replay detection.
        packet_number: u64,
        /// Highest packet number accepted so far.
        highest_seen: u64,
    },
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Socket(err) => write!(f, "socket error: {err:?}"),
            Self::Packet(err) => write!(f, "packet error: {err}"),
            Self::Crypto(err) => write!(f, "crypto error: {err:?}"),
            Self::BufferTooSmall {
                required,
                available,
            } => write!(
                f,
                "buffer too small: need {required} bytes, have {available}"
            ),
            Self::PayloadTooLarge { len, max } => {
                write!(f, "payload too large: {len} bytes (max {max})")
            }
            Self::ReplayDetected {
                packet_number,
                highest_seen,
            } => write!(
                f,
                "packet {packet_number} replayed (highest seen {highest_seen})"
            ),
        }
    }
}

impl std::error::Error for TransportError {}

impl From<SocketError> for TransportError {
    fn from(err: SocketError) -> Self {
        Self::Socket(err)
    }
}

impl From<PacketError> for TransportError {
    fn from(err: PacketError) -> Self {
        Self::Packet(err)
    }
}

impl From<CryptoError> for TransportError {
    fn from(err: CryptoError) -> Self {
        Self::Crypto(err)
    }
}
