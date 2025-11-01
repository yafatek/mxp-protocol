//! Packet sealing and opening using ChaCha20-Poly1305 session keys.

use super::crypto::{AEAD_TAG_LEN, AeadKey, AeadNonce, AeadTag, SessionKeys, decrypt, encrypt};
use super::error::TransportError;
use super::handshake::nonce_from_packet_number;
use super::packet::{HEADER_SIZE, PacketError, PacketFlags, PacketHeader};

/// Result of decrypting an inbound packet.
#[derive(Debug)]
pub struct DecryptedPacket {
    header: PacketHeader,
    payload: Vec<u8>,
}

impl DecryptedPacket {
    /// Access the decoded header.
    #[must_use]
    pub fn header(&self) -> &PacketHeader {
        &self.header
    }

    /// Borrow the plaintext payload.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Consume the structure, returning header and payload.
    #[must_use]
    pub fn into_parts(self) -> (PacketHeader, Vec<u8>) {
        (self.header, self.payload)
    }
}

/// Maintains state for sealing and opening packets with session keys.
#[derive(Debug, Clone)]
pub struct PacketCipher {
    send_key: AeadKey,
    receive_key: AeadKey,
    send_packet_number: u64,
    highest_received: Option<u64>,
}

impl PacketCipher {
    /// Create a cipher instance from negotiated session keys.
    #[must_use]
    pub fn new(keys: SessionKeys) -> Self {
        Self {
            send_key: keys.send().clone(),
            receive_key: keys.receive().clone(),
            send_packet_number: 0,
            highest_received: None,
        }
    }

    /// Set the initial packet numbers for sent and receive directions.
    #[must_use]
    pub fn with_initial_numbers(mut self, send: u64, highest_received: Option<u64>) -> Self {
        self.send_packet_number = send;
        self.highest_received = highest_received;
        self
    }

    /// Seal the provided payload into the given buffer.
    ///
    /// Returns the packet number used for this transmission and the total encoded length.
    pub fn seal_into(
        &mut self,
        conn_id: u64,
        flags: PacketFlags,
        payload: &[u8],
        buffer: &mut [u8],
    ) -> Result<(u64, usize), TransportError> {
        let max_payload = u16::MAX as usize - AEAD_TAG_LEN;
        if payload.len() > max_payload {
            return Err(TransportError::PayloadTooLarge {
                len: payload.len(),
                max: max_payload,
            });
        }

        let total_len = HEADER_SIZE + payload.len() + AEAD_TAG_LEN;
        if buffer.len() < total_len {
            return Err(TransportError::BufferTooSmall {
                required: total_len,
                available: buffer.len(),
            });
        }

        let packet_number = self.send_packet_number;
        self.send_packet_number = self.send_packet_number.wrapping_add(1);

        let nonce = nonce_from_packet_number(packet_number);

        let mut header = PacketHeader::new(
            conn_id,
            packet_number,
            (payload.len() + AEAD_TAG_LEN) as u16,
            flags,
        );
        header.set_nonce(*nonce.as_bytes());

        let (head, rest) = buffer.split_at_mut(HEADER_SIZE);
        header.encode(head).map_err(TransportError::from)?;

        let (ciphertext, tag) = encrypt(&self.send_key, &nonce, payload, head);

        let (cipher_slice, tag_slice) = rest.split_at_mut(ciphertext.len());
        cipher_slice.copy_from_slice(&ciphertext);
        tag_slice[..AEAD_TAG_LEN].copy_from_slice(tag.as_bytes());

        Ok((packet_number, total_len))
    }

    /// Try to open an inbound packet, returning the header and plaintext payload.
    pub fn open(&mut self, packet: &[u8]) -> Result<DecryptedPacket, TransportError> {
        if packet.len() < HEADER_SIZE + AEAD_TAG_LEN {
            return Err(TransportError::Packet(PacketError::BufferTooSmall {
                expected: HEADER_SIZE + AEAD_TAG_LEN,
                actual: packet.len(),
            }));
        }

        let (header_bytes, body) = packet.split_at(HEADER_SIZE);
        let header = PacketHeader::decode(header_bytes).map_err(TransportError::from)?;
        let payload_len = header.payload_len() as usize;

        if payload_len < AEAD_TAG_LEN {
            return Err(TransportError::Packet(PacketError::BufferTooSmall {
                expected: AEAD_TAG_LEN,
                actual: payload_len,
            }));
        }

        if body.len() < payload_len {
            return Err(TransportError::Packet(PacketError::BufferTooSmall {
                expected: payload_len,
                actual: body.len(),
            }));
        }

        let body = &body[..payload_len];
        let (ciphertext, tag_bytes) = body.split_at(body.len() - AEAD_TAG_LEN);

        let tag = AeadTag::from_bytes(tag_bytes).map_err(TransportError::from)?;
        let nonce = AeadNonce::from_array(*header.nonce());

        if let Some(highest) = self.highest_received {
            if header.packet_number() <= highest {
                return Err(TransportError::ReplayDetected {
                    packet_number: header.packet_number(),
                    highest_seen: highest,
                });
            }
        }

        let plaintext = decrypt(&self.receive_key, &nonce, ciphertext, header_bytes, &tag)?;
        let new_highest = match self.highest_received {
            Some(prev) => prev.max(header.packet_number()),
            None => header.packet_number(),
        };
        self.highest_received = Some(new_highest);

        Ok(DecryptedPacket {
            header,
            payload: plaintext,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::crypto::{AEAD_KEY_LEN, AeadKey};

    #[test]
    fn seal_and_open_roundtrip() {
        let client_keys = SessionKeys::new(
            AeadKey::from_array([0x11u8; AEAD_KEY_LEN]),
            AeadKey::from_array([0x22u8; AEAD_KEY_LEN]),
        );
        let server_keys = SessionKeys::new(
            AeadKey::from_array([0x22u8; AEAD_KEY_LEN]),
            AeadKey::from_array([0x11u8; AEAD_KEY_LEN]),
        );

        let mut send_cipher = PacketCipher::new(client_keys);
        let mut recv_cipher = PacketCipher::new(server_keys);

        let mut buffer = vec![0u8; 2048];
        let payload = b"hello secure world";
        let (pn, len) = send_cipher
            .seal_into(0xAA55, PacketFlags::from_bits(0), payload, &mut buffer)
            .expect("seal");
        assert_eq!(pn, 0);

        let packet = &buffer[..len];
        let decrypted = recv_cipher.open(packet).expect("open");
        assert_eq!(decrypted.header().conn_id(), 0xAA55);
        assert_eq!(decrypted.payload(), payload);

        // Replay should fail.
        let err = recv_cipher.open(packet).expect_err("replay must fail");
        match err {
            TransportError::ReplayDetected { .. } => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
