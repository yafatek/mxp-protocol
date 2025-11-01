//! Core packet and frame definitions for the MXP transport.

use std::convert::TryInto;
use std::fmt;

use super::ack::{AckError, AckFrame};
use super::stream::StreamId;

/// Size of an encoded packet header in bytes.
pub const HEADER_SIZE: usize = 32;

/// Size of the AEAD authentication tag in bytes.
// pub const AUTH_TAG_SIZE: usize = 16;

/// Size of the nonce carried in the header (12 bytes for ChaCha20/AES).
pub const NONCE_SIZE: usize = 12;

/// Flags describing packet semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketFlags(u8);

impl PacketFlags {
    /// Packet contains handshake data.
    pub const HANDSHAKE: u8 = 1 << 0;
    /// Packet elicits acknowledgements.
    pub const ACK_ELICITING: u8 = 1 << 1;
    /// Packet carries only acknowledgements.
    pub const ACK: u8 = 1 << 2;
    /// Packet signals a key phase transition.
    pub const KEY_PHASE: u8 = 1 << 3;
    /// Packet contains probe/keepalive data.
    pub const PROBE: u8 = 1 << 4;

    /// Create a new flag set from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    /// Return the underlying bit representation.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Check whether the given flag is set.
    #[must_use]
    pub const fn contains(self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }

    /// Insert a flag into the set.
    pub fn insert(&mut self, flag: u8) {
        self.0 |= flag;
    }

    /// Clear a flag from the set.
    pub fn remove(&mut self, flag: u8) {
        self.0 &= !flag;
    }
}

impl Default for PacketFlags {
    fn default() -> Self {
        Self(0)
    }
}

/// Errors produced when encoding/decoding packet headers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacketError {
    /// Input buffer does not contain enough bytes.
    BufferTooSmall { expected: usize, actual: usize },
    /// Payload length exceeds self-imposed limits.
    PayloadTooLarge { len: usize, max: usize },
    /// Reserved bits set unexpectedly.
    ReservedBitsSet(u8),
}

impl fmt::Display for PacketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall { expected, actual } => {
                write!(f, "buffer too small: need {expected} bytes, got {actual}")
            }
            Self::PayloadTooLarge { len, max } => {
                write!(f, "payload too large: {len} bytes (max {max})")
            }
            Self::ReservedBitsSet(bits) => {
                write!(f, "reserved bits set in packet flags: {bits:#010b}")
            }
        }
    }
}

impl std::error::Error for PacketError {}

/// High-level packet header used by the transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketHeader {
    conn_id: u64,
    packet_number: u64,
    flags: PacketFlags,
    payload_len: u16,
    reserved: u8,
    nonce: [u8; NONCE_SIZE],
}

impl PacketHeader {
    /// Create a new packet header.
    #[must_use]
    pub fn new(conn_id: u64, packet_number: u64, payload_len: u16, flags: PacketFlags) -> Self {
        Self {
            conn_id,
            packet_number,
            flags,
            payload_len,
            reserved: 0,
            nonce: [0u8; NONCE_SIZE],
        }
    }

    /// Set the nonce associated with the packet.
    pub fn set_nonce(&mut self, nonce: [u8; NONCE_SIZE]) {
        self.nonce = nonce;
    }

    /// Encode the header into the provided buffer (must be at least 32 bytes).
    pub fn encode(&self, out: &mut [u8]) -> Result<(), PacketError> {
        if out.len() < HEADER_SIZE {
            return Err(PacketError::BufferTooSmall {
                expected: HEADER_SIZE,
                actual: out.len(),
            });
        }

        out.fill(0);
        out[0..8].copy_from_slice(&self.conn_id.to_le_bytes());
        out[8..16].copy_from_slice(&self.packet_number.to_le_bytes());
        out[16] = self.flags.bits();
        out[17] = self.reserved;
        out[18..20].copy_from_slice(&self.payload_len.to_le_bytes());
        out[20..32].copy_from_slice(&self.nonce);
        Ok(())
    }

    /// Decode a packet header from raw bytes.
    #[must_use]
    pub fn decode(buf: &[u8]) -> Result<Self, PacketError> {
        if buf.len() < HEADER_SIZE {
            return Err(PacketError::BufferTooSmall {
                expected: HEADER_SIZE,
                actual: buf.len(),
            });
        }

        let flags = PacketFlags::from_bits(buf[16]);
        let reserved = buf[17];
        if reserved != 0 {
            return Err(PacketError::ReservedBitsSet(reserved));
        }

        let payload_len = u16::from_le_bytes([buf[18], buf[19]]);

        let mut nonce = [0u8; NONCE_SIZE];
        nonce.copy_from_slice(&buf[20..32]);

        Ok(Self {
            conn_id: u64::from_le_bytes(buf[0..8].try_into().unwrap()),
            packet_number: u64::from_le_bytes(buf[8..16].try_into().unwrap()),
            flags,
            payload_len,
            reserved,
            nonce,
        })
    }

    /// Connection identifier accessor.
    #[must_use]
    pub const fn conn_id(&self) -> u64 {
        self.conn_id
    }

    /// Packet number accessor.
    #[must_use]
    pub const fn packet_number(&self) -> u64 {
        self.packet_number
    }

    /// Payload length accessor.
    #[must_use]
    pub const fn payload_len(&self) -> u16 {
        self.payload_len
    }

    /// Flags accessor.
    #[must_use]
    pub const fn flags(&self) -> PacketFlags {
        self.flags
    }

    /// Nonce accessor.
    #[must_use]
    pub const fn nonce(&self) -> &[u8; NONCE_SIZE] {
        &self.nonce
    }
}

/// Enumerates available frame kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    /// Opens a new reliable stream.
    StreamOpen,
    /// Carries stream data.
    StreamData,
    /// Closes a reliable stream.
    StreamFin,
    /// Unreliable datagram payload.
    Datagram,
    /// Acknowledgement data.
    Ack,
    /// Handshake/crypto data.
    Crypto,
    /// Control messages (window updates, migration tokens, etc.).
    Control,
    /// Stream flow-control credit (per-stream MAX_DATA equivalent).
    StreamMaxData,
    /// Connection-level flow-control credit.
    ConnectionMaxData,
}

/// Transport frame abstraction.
#[derive(Debug, Clone)]
pub struct Frame {
    frame_type: FrameType,
    payload: Vec<u8>,
}

impl Frame {
    /// Create a new frame instance.
    #[must_use]
    pub fn new(frame_type: FrameType, payload: Vec<u8>) -> Self {
        Self {
            frame_type,
            payload,
        }
    }

    /// Create an ACK frame by encoding the provided structure.
    pub fn from_ack(frame: &AckFrame) -> Self {
        let mut payload = Vec::new();
        frame.encode(&mut payload);
        Self::new(FrameType::Ack, payload)
    }

    /// Create a stream control frame carrying flow-control credits.
    pub fn stream_max_data(stream: StreamId, new_limit: u64) -> Self {
        let mut payload = Vec::with_capacity(8 + 8);
        payload.extend_from_slice(&stream.as_u64().to_le_bytes());
        payload.extend_from_slice(&new_limit.to_le_bytes());
        Self::new(FrameType::StreamMaxData, payload)
    }

    /// Create a connection-level MAX_DATA frame.
    pub fn connection_max_data(new_limit: u64) -> Self {
        Self::new(
            FrameType::ConnectionMaxData,
            new_limit.to_le_bytes().to_vec(),
        )
    }

    /// Frame type accessor.
    #[must_use]
    pub const fn frame_type(&self) -> FrameType {
        self.frame_type
    }

    /// Borrow the payload contents.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Consume the frame and return the payload.
    #[must_use]
    pub fn into_payload(self) -> Vec<u8> {
        self.payload
    }

    /// Attempt to decode the payload as an ACK frame.
    pub fn decode_ack(&self) -> Result<AckFrame, AckError> {
        if self.frame_type != FrameType::Ack {
            return Err(AckError::UnexpectedFrameType);
        }
        AckFrame::decode(&self.payload)
    }

    /// Decode a stream MAX_DATA frame payload.
    pub fn decode_stream_max_data(&self) -> Result<(StreamId, u64), AckError> {
        if self.frame_type != FrameType::StreamMaxData {
            return Err(AckError::UnexpectedFrameType);
        }
        if self.payload.len() != 16 {
            return Err(AckError::UnexpectedFrameType);
        }
        let stream = StreamId::from_raw(u64::from_le_bytes(self.payload[0..8].try_into().unwrap()));
        let limit = u64::from_le_bytes(self.payload[8..16].try_into().unwrap());
        Ok((stream, limit))
    }

    /// Decode a connection MAX_DATA frame payload.
    pub fn decode_connection_max_data(&self) -> Result<u64, AckError> {
        if self.frame_type != FrameType::ConnectionMaxData {
            return Err(AckError::UnexpectedFrameType);
        }
        if self.payload.len() != 8 {
            return Err(AckError::UnexpectedFrameType);
        }
        Ok(u64::from_le_bytes(self.payload[0..8].try_into().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::stream::{EndpointRole, StreamKind};

    #[test]
    fn stream_max_data_roundtrip() {
        let stream = StreamId::new(EndpointRole::Client, StreamKind::Bidirectional, 3);
        let frame = Frame::stream_max_data(stream, 512);
        assert_eq!(frame.frame_type(), FrameType::StreamMaxData);
        let (decoded, limit) = frame.decode_stream_max_data().expect("decode");
        assert_eq!(decoded, stream);
        assert_eq!(limit, 512);
    }

    #[test]
    fn connection_max_data_roundtrip() {
        let frame = Frame::connection_max_data(2048);
        assert_eq!(frame.frame_type(), FrameType::ConnectionMaxData);
        let limit = frame.decode_connection_max_data().expect("decode");
        assert_eq!(limit, 2048);
    }
}
