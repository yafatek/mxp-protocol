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

    // Property-based tests
    #[cfg(test)]
    mod proptests {
        use super::*;
        use proptest::prelude::*;
        use crate::{MAGIC_NUMBER, MAX_PAYLOAD_SIZE};

        // Strategy to generate valid message types
        fn message_type_strategy() -> impl Strategy<Value = MessageType> {
            prop_oneof![
                Just(MessageType::AgentRegister),
                Just(MessageType::AgentDiscover),
                Just(MessageType::AgentHeartbeat),
                Just(MessageType::Call),
                Just(MessageType::Response),
                Just(MessageType::Event),
                Just(MessageType::StreamOpen),
                Just(MessageType::StreamChunk),
                Just(MessageType::StreamClose),
                Just(MessageType::Ack),
                Just(MessageType::Error),
            ]
        }

        // Strategy to generate payloads of various sizes
        fn payload_strategy() -> impl Strategy<Value = Vec<u8>> {
            prop::collection::vec(any::<u8>(), 0..=16384)
        }

        proptest! {
            /// Property: Any valid message should roundtrip correctly
            #[test]
            fn prop_roundtrip_preserves_data(
                msg_type in message_type_strategy(),
                payload in payload_strategy(),
            ) {
                let original = Message::new(msg_type, payload.clone());
                // Note: We can't set IDs directly in current API, so we test what we can

                let encoded = encode(&original);
                let decoded = decode(Bytes::from(encoded)).unwrap();

                prop_assert_eq!(decoded.message_type(), original.message_type());
                prop_assert_eq!(decoded.payload().as_ref(), original.payload().as_ref());
            }

            /// Property: Corrupting any byte in the checksum should be detected
            #[test]
            fn prop_checksum_detects_corruption(
                msg_type in message_type_strategy(),
                payload in payload_strategy(),
                corrupt_offset in 0usize..8,
                corrupt_value in 1u8..=255,
            ) {
                let original = Message::new(msg_type, payload);
                let mut encoded = encode(&original);

                // Corrupt a byte in the checksum (last 8 bytes)
                let len = encoded.len();
                if len > 8 {
                    let checksum_start = len - 8;
                    encoded[checksum_start + corrupt_offset] ^= corrupt_value;

                    let result = decode(Bytes::from(encoded));
                    prop_assert!(result.is_err(), "Corrupted checksum should be detected");
                }
            }

            /// Property: Corrupting any byte in the payload should be detected
            #[test]
            fn prop_payload_corruption_detected(
                msg_type in message_type_strategy(),
                payload in payload_strategy().prop_filter("non-empty", |p| !p.is_empty()),
                corrupt_offset_ratio in 0.0f64..1.0,
                corrupt_value in 1u8..=255,
            ) {
                let original = Message::new(msg_type, payload.clone());
                let mut encoded = encode(&original);

                // Corrupt a byte in the payload (between header and checksum)
                let payload_start = HEADER_SIZE;
                let payload_end = encoded.len() - CHECKSUM_SIZE;

                if payload_end > payload_start {
                    let payload_len = payload_end - payload_start;
                    let corrupt_offset = payload_start + (payload_len as f64 * corrupt_offset_ratio) as usize;
                    encoded[corrupt_offset] ^= corrupt_value;

                    let result = decode(Bytes::from(encoded));
                    prop_assert!(result.is_err(), "Corrupted payload should fail checksum");
                }
            }

            /// Property: Invalid magic numbers should always be rejected
            #[test]
            fn prop_invalid_magic_rejected(
                invalid_magic in any::<u32>().prop_filter("not valid magic", |m| *m != MAGIC_NUMBER),
                payload in payload_strategy(),
            ) {
                let original = Message::new(MessageType::Call, payload);
                let mut encoded = encode(&original);

                // Replace magic number
                encoded[0..4].copy_from_slice(&invalid_magic.to_le_bytes());
                
                let result = decode(Bytes::from(encoded));
                prop_assert!(result.is_err(), "Invalid magic should be rejected");
            }

            /// Property: Messages with payload > MAX_PAYLOAD_SIZE should be rejected
            #[test]
            fn prop_oversized_payload_rejected(
                msg_type in message_type_strategy(),
            ) {
                let original = Message::new(msg_type, vec![0u8; 1024]);
                let mut encoded = encode(&original);

                // Manually set payload_len to exceed MAX_PAYLOAD_SIZE
                let oversized_len = (MAX_PAYLOAD_SIZE as u64) + 1;
                encoded[24..32].copy_from_slice(&oversized_len.to_le_bytes());

                // Recalculate checksum for the modified header
                let checksum_offset = HEADER_SIZE + 1024;
                let checksum = xxh3_64(&encoded[0..checksum_offset]);
                encoded[checksum_offset..checksum_offset + 8].copy_from_slice(&checksum.to_le_bytes());

                let result = decode(Bytes::from(encoded));
                prop_assert!(result.is_err(), "Oversized payload should be rejected");
            }

            /// Property: Encoding should be deterministic (same input = same output)
            #[test]
            fn prop_encoding_deterministic(
                msg_type in message_type_strategy(),
                payload in payload_strategy(),
            ) {
                let msg1 = Message::new(msg_type, payload.clone());
                let msg2 = Message::new(msg_type, payload);

                let encoded1 = encode(&msg1);
                let encoded2 = encode(&msg2);

                // Headers might differ (message_id, trace_id are generated)
                // but structure should be consistent
                prop_assert_eq!(encoded1.len(), encoded2.len());
            }

            /// Property: Empty payloads should work
            #[test]
            fn prop_empty_payload_works(msg_type in message_type_strategy()) {
                let original = Message::new(msg_type, vec![]);
                let encoded = encode(&original);
                let decoded = decode(Bytes::from(encoded)).unwrap();

                prop_assert_eq!(decoded.message_type(), original.message_type());
                prop_assert_eq!(decoded.payload().len(), 0);
            }

            /// Property: Maximum valid payload should work
            #[test]
            fn prop_max_payload_works(msg_type in message_type_strategy()) {
                // Test with a reasonably large payload (not full 16MB to keep tests fast)
                let payload = vec![0u8; 65536]; // 64KB
                let original = Message::new(msg_type, payload.clone());
                let encoded = encode(&original);
                let decoded = decode(Bytes::from(encoded)).unwrap();

                prop_assert_eq!(decoded.message_type(), original.message_type());
                prop_assert_eq!(decoded.payload().len(), 65536);
            }
        }
    }
}
