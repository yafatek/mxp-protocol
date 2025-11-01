//! MXP message implementation

use bytes::Bytes;
use uuid::Uuid;

use super::{Flags, MessageHeader, MessageType};

/// MXP message
#[derive(Debug, Clone)]
pub struct Message {
    /// Message header
    header: MessageHeader,
    /// Message payload
    payload: Bytes,
}

impl Message {
    /// Create a new message
    pub fn new(msg_type: MessageType, payload: impl Into<Vec<u8>>) -> Self {
        let payload = Bytes::from(payload.into());
        let message_id = Self::generate_id();
        let trace_id = Self::generate_id();

        let header = MessageHeader::new(msg_type, message_id, trace_id, payload.len() as u64);

        Self { header, payload }
    }

    /// Create a new message with explicit IDs
    pub fn with_ids(
        msg_type: MessageType,
        message_id: u64,
        trace_id: u64,
        payload: impl Into<Bytes>,
    ) -> Self {
        let payload = payload.into();
        let header = MessageHeader::new(msg_type, message_id, trace_id, payload.len() as u64);

        Self { header, payload }
    }

    /// Get message type
    #[must_use]
    pub fn message_type(&self) -> Option<MessageType> {
        self.header.message_type()
    }

    /// Get message ID
    #[must_use]
    pub fn message_id(&self) -> u64 {
        self.header.message_id()
    }

    /// Get trace ID
    #[must_use]
    pub fn trace_id(&self) -> u64 {
        self.header.trace_id()
    }

    /// Get payload
    #[must_use]
    pub fn payload(&self) -> &Bytes {
        &self.payload
    }

    /// Get flags
    #[must_use]
    pub fn flags(&self) -> Flags {
        self.header.flags()
    }

    /// Set flags
    pub fn set_flags(&mut self, flags: Flags) {
        self.header.set_flags(flags);
    }

    /// Get header
    #[must_use]
    pub const fn header(&self) -> &MessageHeader {
        &self.header
    }

    /// Get mutable header
    pub fn header_mut(&mut self) -> &mut MessageHeader {
        &mut self.header
    }

    /// Generate a random message/trace ID
    fn generate_id() -> u64 {
        let uuid = Uuid::new_v4();
        let bytes = uuid.as_bytes();
        u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    }

    /// Encode message to bytes
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        super::encode(self)
    }

    /// Decode message from bytes
    pub fn decode(bytes: &[u8]) -> super::Result<Self> {
        super::decode(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::new(MessageType::Call, b"test payload");

        assert_eq!(msg.message_type(), Some(MessageType::Call));
        assert_eq!(msg.payload().as_ref(), b"test payload");
        assert_eq!(msg.header.payload_len(), 12);
    }

    #[test]
    fn test_message_roundtrip() {
        let original = Message::new(MessageType::Event, b"hello world");
        let encoded = original.encode();
        let decoded = Message::decode(&encoded).unwrap();

        assert_eq!(decoded.message_type(), original.message_type());
        assert_eq!(decoded.payload().as_ref(), original.payload().as_ref());
        assert_eq!(decoded.message_id(), original.message_id());
    }
}

