//! MXP message codec (encode/decode)
//!
//! This module provides zero-copy encoding and decoding of MXP messages.

use bytes::Bytes;
use xxhash_rust::xxh3::xxh3_64;

use super::{CHECKSUM_SIZE, Error, HEADER_SIZE, MIN_MESSAGE_SIZE, Message, MessageHeader, Result};

/// Encode a message to bytes
///
/// # Format
///
/// ```text
/// [HEADER (32 bytes)] [PAYLOAD (variable)] [CHECKSUM (8 bytes)]
/// ```
///
/// # Performance
///
/// This operation should complete in < 1μs for typical payloads.
#[must_use]
pub fn encode(message: &Message) -> Vec<u8> {
    let header = message.header();
    let payload = message.payload();

    // Calculate total size
    let total_size = HEADER_SIZE + payload.len() + CHECKSUM_SIZE;
    let mut bytes = Vec::with_capacity(total_size);

    // Write header
    bytes.extend_from_slice(&header.to_bytes());

    // Write payload
    bytes.extend_from_slice(payload);

    // Calculate checksum (header + payload)
    let checksum = xxh3_64(&bytes);

    // Write checksum
    bytes.extend_from_slice(&checksum.to_le_bytes());

    bytes
}

/// Decode a message from bytes
///
/// # Format
///
/// ```text
/// [HEADER (32 bytes)] [PAYLOAD (variable)] [CHECKSUM (8 bytes)]
/// ```
///
/// # Performance
///
/// This operation should complete in < 1μs for typical payloads.
///
/// # Errors
///
/// Returns an error if:
/// - Buffer is too small
/// - Magic number is invalid
/// - Message type is unknown
/// - Checksum doesn't match
/// - Payload is too large
pub fn decode(bytes: Bytes) -> Result<Message> {
    let total_available = bytes.len();

    // Check minimum size
    if total_available < MIN_MESSAGE_SIZE {
        return Err(Error::BufferTooSmall {
            needed: MIN_MESSAGE_SIZE,
            got: total_available,
        });
    }

    // Parse header
    let header = MessageHeader::from_bytes(&bytes[0..HEADER_SIZE])?;

    // Calculate expected total size
    let payload_len = header.payload_len() as usize;
    let total_size = HEADER_SIZE + payload_len + CHECKSUM_SIZE;

    if total_available < total_size {
        return Err(Error::BufferTooSmall {
            needed: total_size,
            got: total_available,
        });
    }

    // Extract payload
    let payload = bytes.slice(HEADER_SIZE..HEADER_SIZE + payload_len);

    // Extract checksum
    let checksum_offset = HEADER_SIZE + payload_len;
    let checksum_slice = &bytes[checksum_offset..checksum_offset + CHECKSUM_SIZE];
    let stored_checksum = u64::from_le_bytes(checksum_slice.try_into().unwrap());

    // Verify checksum
    let calculated_checksum = xxh3_64(&bytes[0..checksum_offset]);

    if stored_checksum != calculated_checksum {
        return Err(Error::ChecksumMismatch {
            expected: calculated_checksum,
            found: stored_checksum,
        });
    }

    // Create message
    Ok(Message::from_parts(header, payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MessageType;
    use bytes::Bytes;

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = Message::new(MessageType::Call, b"test payload");
        let encoded = encode(&original);
        let decoded = decode(Bytes::from(encoded)).unwrap();

        assert_eq!(decoded.message_type(), original.message_type());
        assert_eq!(decoded.payload().as_ref(), original.payload().as_ref());
        assert_eq!(decoded.message_id(), original.message_id());
        assert_eq!(decoded.trace_id(), original.trace_id());
    }

    #[test]
    fn test_decode_invalid_magic() {
        let mut bytes = vec![0u8; MIN_MESSAGE_SIZE];
        bytes[0..4].copy_from_slice(&0xDEAD_BEEF_u32.to_le_bytes());

        let result = decode(Bytes::from(bytes));
        assert!(matches!(result, Err(Error::InvalidMagic { .. })));
    }

    #[test]
    fn test_decode_checksum_mismatch() {
        let original = Message::new(MessageType::Call, b"test");
        let mut encoded = encode(&original);

        // Corrupt the checksum
        let len = encoded.len();
        encoded[len - 1] ^= 0xFF;

        let result = decode(Bytes::from(encoded));
        assert!(matches!(result, Err(Error::ChecksumMismatch { .. })));
    }

    #[test]
    fn test_decode_buffer_too_small() {
        let bytes = vec![0u8; 10]; // Too small
        let result = decode(Bytes::from(bytes));
        assert!(matches!(result, Err(Error::BufferTooSmall { .. })));
    }

    #[test]
    fn test_encode_performance() {
        use std::time::Instant;

        let message = Message::new(MessageType::Call, vec![0u8; 1024]);

        let start = Instant::now();
        for _ in 0..1000 {
            let _ = encode(&message);
        }
        let elapsed = start.elapsed();

        let avg_micros = elapsed.as_micros() / 1000;
        println!("Average encode time: {avg_micros}μs");

        // Should be reasonably fast (< 100μs on CI)
        assert!(avg_micros < 100, "Encode too slow: {avg_micros}μs");
    }

    #[test]
    fn test_decode_performance() {
        use std::time::Instant;

        let message = Message::new(MessageType::Call, vec![0u8; 1024]);
        let encoded = encode(&message);
        let encoded_bytes = Bytes::from(encoded);

        let start = Instant::now();
        for _ in 0..1000 {
            let _ = decode(encoded_bytes.clone()).unwrap();
        }
        let elapsed = start.elapsed();

        let avg_micros = elapsed.as_micros() / 1000;
        println!("Average decode time: {avg_micros}μs");

        // Should be reasonably fast (< 100μs on CI)
        assert!(avg_micros < 100, "Decode too slow: {avg_micros}μs");
    }
}
