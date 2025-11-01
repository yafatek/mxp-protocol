//! MXP message header
//!
//! The header is 32 bytes and cache-aligned for performance.

use super::{Flags, MAGIC_NUMBER, MessageType};

/// MXP message header (32 bytes, cache-aligned)
///
/// # Wire Format
///
/// ```text
/// 0                   1                   2                   3
/// 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                        Magic Number (4)                       |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// | Message Type  |     Flags     |          Reserved             |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                                                               |
/// +                      Message ID (8)                           +
/// |                                                               |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                                                               |
/// +                      Trace ID (8)                             +
/// |                                                               |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                                                               |
/// +                   Payload Length (8)                          +
/// |                                                               |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// ```
#[repr(C, align(32))]
#[derive(Debug, Clone, Copy)]
pub struct MessageHeader {
    magic: u32,
    msg_type: u8,
    flags: u8,
    reserved: u16,
    message_id: u64,
    trace_id: u64,
    payload_len: u64,
}

impl MessageHeader {
    /// Create a new message header
    #[must_use]
    pub fn new(msg_type: MessageType, message_id: u64, trace_id: u64, payload_len: u64) -> Self {
        Self {
            magic: MAGIC_NUMBER,
            msg_type: msg_type.as_u8(),
            flags: 0,
            reserved: 0,
            message_id,
            trace_id,
            payload_len,
        }
    }

    /// Get magic number
    #[must_use]
    pub const fn magic(&self) -> u32 {
        self.magic
    }

    /// Get message type byte
    #[must_use]
    pub const fn msg_type_byte(&self) -> u8 {
        self.msg_type
    }

    /// Get message type
    #[must_use]
    pub fn message_type(&self) -> Option<MessageType> {
        MessageType::from_u8(self.msg_type)
    }

    /// Get flags byte
    #[must_use]
    pub const fn flags_byte(&self) -> u8 {
        self.flags
    }

    /// Get flags
    #[must_use]
    pub fn flags(&self) -> Flags {
        Flags::from_u8(self.flags).expect("flags validated during parsing")
    }

    /// Set flags
    pub fn set_flags(&mut self, flags: Flags) {
        self.flags = flags.as_u8();
    }

    /// Get message ID
    #[must_use]
    pub const fn message_id(&self) -> u64 {
        self.message_id
    }

    /// Get trace ID
    #[must_use]
    pub const fn trace_id(&self) -> u64 {
        self.trace_id
    }

    /// Get payload length
    #[must_use]
    pub const fn payload_len(&self) -> u64 {
        self.payload_len
    }

    /// Validate header
    pub fn validate(&self) -> super::Result<()> {
        // Check magic number
        if self.magic != MAGIC_NUMBER {
            return Err(super::Error::InvalidMagic { found: self.magic });
        }

        // Check reserved bits
        if self.reserved != 0 {
            return Err(super::Error::ReservedFieldNonZero {
                field: "header.reserved",
                value: u64::from(self.reserved),
            });
        }

        // Check message type
        if self.message_type().is_none() {
            return Err(super::Error::InvalidMessageType {
                type_byte: self.msg_type,
            });
        }

        // Validate flags
        if Flags::from_u8(self.flags).is_none() {
            return Err(super::Error::InvalidFlags { flags: self.flags });
        }

        // Check payload size
        let payload_len = self.payload_len;
        if payload_len > super::MAX_PAYLOAD_SIZE as u64 {
            return Err(super::Error::PayloadTooLarge {
                size: payload_len as usize,
                max: super::MAX_PAYLOAD_SIZE,
            });
        }

        Ok(())
    }

    /// Convert to bytes (little-endian)
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];

        bytes[0..4].copy_from_slice(&self.magic.to_le_bytes());
        bytes[4] = self.msg_type;
        bytes[5] = self.flags;
        bytes[6..8].copy_from_slice(&self.reserved.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.message_id.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.trace_id.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.payload_len.to_le_bytes());

        bytes
    }

    /// Parse from bytes (little-endian)
    ///
    /// # Safety
    ///
    /// Caller must ensure the slice is at least 32 bytes.
    pub fn from_bytes(bytes: &[u8]) -> super::Result<Self> {
        if bytes.len() < 32 {
            return Err(super::Error::BufferTooSmall {
                needed: 32,
                got: bytes.len(),
            });
        }

        let header = Self {
            magic: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            msg_type: bytes[4],
            flags: bytes[5],
            reserved: u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
            message_id: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            trace_id: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            payload_len: u64::from_le_bytes(bytes[24..32].try_into().unwrap()),
        };

        header.validate()?;
        Ok(header)
    }
}

impl Default for MessageHeader {
    fn default() -> Self {
        Self {
            magic: MAGIC_NUMBER,
            msg_type: 0,
            flags: 0,
            reserved: 0,
            message_id: 0,
            trace_id: 0,
            payload_len: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size() {
        assert_eq!(std::mem::size_of::<MessageHeader>(), 32);
    }

    #[test]
    fn test_header_roundtrip() {
        let header = MessageHeader::new(MessageType::Call, 123, 456, 789);
        let bytes = header.to_bytes();
        let decoded = MessageHeader::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.magic(), MAGIC_NUMBER);
        assert_eq!(decoded.msg_type_byte(), MessageType::Call.as_u8());
        assert_eq!(decoded.message_id(), 123);
        assert_eq!(decoded.trace_id(), 456);
        assert_eq!(decoded.payload_len(), 789);
    }

    #[test]
    fn test_invalid_magic() {
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&0xDEAD_BEEF_u32.to_le_bytes());

        let result = MessageHeader::from_bytes(&bytes);
        assert!(matches!(
            result,
            Err(super::super::Error::InvalidMagic { .. })
        ));
    }
}
