//! MXP message types and flags

use std::fmt;

/// MXP message types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageType {
    /// Register agent with mesh
    AgentRegister = 0x01,
    /// Discover agents by capability
    AgentDiscover = 0x02,
    /// Keep-alive / health check
    AgentHeartbeat = 0x03,

    /// Synchronous RPC call
    Call = 0x10,
    /// Response to Call
    Response = 0x11,
    /// Async event (fire-and-forget)
    Event = 0x12,

    /// Open new stream
    StreamOpen = 0x20,
    /// Stream data chunk
    StreamChunk = 0x21,
    /// Close stream
    StreamClose = 0x22,

    /// Acknowledgment
    Ack = 0xF0,
    /// Error response
    Error = 0xF1,
}

impl MessageType {
    /// Convert from byte
    #[must_use]
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::AgentRegister),
            0x02 => Some(Self::AgentDiscover),
            0x03 => Some(Self::AgentHeartbeat),
            0x10 => Some(Self::Call),
            0x11 => Some(Self::Response),
            0x12 => Some(Self::Event),
            0x20 => Some(Self::StreamOpen),
            0x21 => Some(Self::StreamChunk),
            0x22 => Some(Self::StreamClose),
            0xF0 => Some(Self::Ack),
            0xF1 => Some(Self::Error),
            _ => None,
        }
    }

    /// Convert to byte
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Check if this message type requires a response
    #[must_use]
    pub const fn requires_response(self) -> bool {
        matches!(self, Self::Call | Self::AgentRegister | Self::AgentDiscover)
    }

    /// Check if this message type is a response
    #[must_use]
    pub const fn is_response(self) -> bool {
        matches!(self, Self::Response | Self::Ack | Self::Error)
    }
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::AgentRegister => "AgentRegister",
            Self::AgentDiscover => "AgentDiscover",
            Self::AgentHeartbeat => "AgentHeartbeat",
            Self::Call => "Call",
            Self::Response => "Response",
            Self::Event => "Event",
            Self::StreamOpen => "StreamOpen",
            Self::StreamChunk => "StreamChunk",
            Self::StreamClose => "StreamClose",
            Self::Ack => "Ack",
            Self::Error => "Error",
        };
        write!(f, "{name}")
    }
}

/// Message flags
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags(u8);

impl Flags {
    /// Valid flag bits mask
    pub const VALID_MASK: u8 =
        Self::COMPRESSED | Self::ENCRYPTED | Self::REQUIRES_ACK | Self::FINAL;
    /// Payload is compressed (zstd)
    pub const COMPRESSED: u8 = 1 << 0;
    /// Payload is encrypted (E2E)
    pub const ENCRYPTED: u8 = 1 << 1;
    /// Sender wants acknowledgment
    pub const REQUIRES_ACK: u8 = 1 << 2;
    /// Last message in sequence
    pub const FINAL: u8 = 1 << 3;

    /// Create empty flags
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Create from byte
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        if value & !Self::VALID_MASK == 0 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Convert to byte
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    /// Set a flag
    #[must_use]
    pub const fn with(mut self, flag: u8) -> Self {
        debug_assert!(flag & !Self::VALID_MASK == 0, "invalid flag bit");
        self.0 |= flag;
        self
    }

    /// Check if flag is set
    #[must_use]
    pub const fn has(self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }

    /// Check if compressed
    #[must_use]
    pub const fn is_compressed(self) -> bool {
        self.has(Self::COMPRESSED)
    }

    /// Check if encrypted
    #[must_use]
    pub const fn is_encrypted(self) -> bool {
        self.has(Self::ENCRYPTED)
    }

    /// Check if requires acknowledgment
    #[must_use]
    pub const fn requires_ack(self) -> bool {
        self.has(Self::REQUIRES_ACK)
    }

    /// Check if final message
    #[must_use]
    pub const fn is_final(self) -> bool {
        self.has(Self::FINAL)
    }
}

impl fmt::Display for Flags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.is_compressed() {
            parts.push("COMPRESSED");
        }
        if self.is_encrypted() {
            parts.push("ENCRYPTED");
        }
        if self.requires_ack() {
            parts.push("REQUIRES_ACK");
        }
        if self.is_final() {
            parts.push("FINAL");
        }
        if parts.is_empty() {
            write!(f, "NONE")
        } else {
            write!(f, "{}", parts.join(" | "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_roundtrip() {
        let types = [
            MessageType::Call,
            MessageType::Response,
            MessageType::StreamOpen,
        ];

        for msg_type in types {
            let byte = msg_type.as_u8();
            let decoded = MessageType::from_u8(byte).unwrap();
            assert_eq!(msg_type, decoded);
        }
    }

    #[test]
    fn test_flags() {
        let flags = Flags::new()
            .with(Flags::COMPRESSED)
            .with(Flags::REQUIRES_ACK);

        assert!(flags.is_compressed());
        assert!(flags.requires_ack());
        assert!(!flags.is_encrypted());
        assert!(!flags.is_final());
    }
}
