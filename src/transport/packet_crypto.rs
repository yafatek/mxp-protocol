//! Packet sealing and opening using ChaCha20-Poly1305 session keys.

use super::crypto::{
    AEAD_TAG_LEN, AeadKey, AeadNonce, AeadTag, HEADER_PROTECTION_MASK_LEN,
    HEADER_PROTECTION_SAMPLE_LEN, HeaderProtectionKey, SessionKeys, decrypt, encrypt,
    header_protection_mask,
};
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

fn build_header_sample(body: &[u8]) -> [u8; HEADER_PROTECTION_SAMPLE_LEN] {
    let mut sample = [0u8; HEADER_PROTECTION_SAMPLE_LEN];
    let take = body.len().min(HEADER_PROTECTION_SAMPLE_LEN);
    sample[..take].copy_from_slice(&body[..take]);
    sample
}

fn apply_header_mask(bytes: &mut [u8], mask: &[u8; HEADER_PROTECTION_MASK_LEN]) {
    bytes[16] ^= mask[0];
    for (idx, slot) in bytes[8..16].iter_mut().enumerate() {
        *slot ^= mask[1 + idx];
    }
}

/// Maintains state for sealing and opening packets with session keys.
#[derive(Debug, Clone)]
pub struct PacketCipher {
    send_key: AeadKey,
    receive_key: AeadKey,
    send_hp: HeaderProtectionKey,
    receive_hp: HeaderProtectionKey,
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
            send_hp: keys.send_hp().clone(),
            receive_hp: keys.receive_hp().clone(),
            send_packet_number: 0,
            highest_received: None,
        }
    }

    /// Set the initial packet numbers for send and receive directions.
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

        let body_len = ciphertext.len() + AEAD_TAG_LEN;
        let sample = build_header_sample(&rest[..body_len]);
        let mask = header_protection_mask(&self.send_hp, &sample);
        apply_header_mask(head, &mask);

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
        if body.len() < HEADER_PROTECTION_SAMPLE_LEN {
            return Err(TransportError::BufferTooSmall {
                required: HEADER_PROTECTION_SAMPLE_LEN,
                available: body.len(),
            });
        }

        let sample = build_header_sample(body);
        let mask = header_protection_mask(&self.receive_hp, &sample);

        let mut unmasked_header = [0u8; HEADER_SIZE];
        unmasked_header.copy_from_slice(header_bytes);
        apply_header_mask(&mut unmasked_header, &mask);

        let header = PacketHeader::decode(&unmasked_header).map_err(TransportError::from)?;
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

        let cipher_len = payload_len - AEAD_TAG_LEN;
        let ciphertext = &body[..cipher_len];
        let tag_bytes = &body[cipher_len..cipher_len + AEAD_TAG_LEN];

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

        let plaintext = decrypt(
            &self.receive_key,
            &nonce,
            ciphertext,
            &unmasked_header,
            &tag,
        )?;
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
    use crate::transport::crypto::{
        AEAD_KEY_LEN, AeadKey, HEADER_PROTECTION_KEY_LEN, HeaderProtectionKey,
    };

    #[test]
    fn seal_and_open_roundtrip() {
        let client_keys = SessionKeys::new(
            AeadKey::from_array([0x11u8; AEAD_KEY_LEN]),
            AeadKey::from_array([0x22u8; AEAD_KEY_LEN]),
            HeaderProtectionKey::from_array([0x33u8; HEADER_PROTECTION_KEY_LEN]),
            HeaderProtectionKey::from_array([0x44u8; HEADER_PROTECTION_KEY_LEN]),
        );
        let server_keys = SessionKeys::new(
            AeadKey::from_array([0x22u8; AEAD_KEY_LEN]),
            AeadKey::from_array([0x11u8; AEAD_KEY_LEN]),
            HeaderProtectionKey::from_array([0x44u8; HEADER_PROTECTION_KEY_LEN]),
            HeaderProtectionKey::from_array([0x33u8; HEADER_PROTECTION_KEY_LEN]),
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

    #[test]
    fn header_is_masked_on_wire_and_restored_on_receive() {
        let client_keys = SessionKeys::new(
            AeadKey::from_array([0xAA; AEAD_KEY_LEN]),
            AeadKey::from_array([0xBB; AEAD_KEY_LEN]),
            HeaderProtectionKey::from_array([0xCC; HEADER_PROTECTION_KEY_LEN]),
            HeaderProtectionKey::from_array([0xDD; HEADER_PROTECTION_KEY_LEN]),
        );
        let server_keys = SessionKeys::new(
            AeadKey::from_array([0xBB; AEAD_KEY_LEN]),
            AeadKey::from_array([0xAA; AEAD_KEY_LEN]),
            HeaderProtectionKey::from_array([0xDD; HEADER_PROTECTION_KEY_LEN]),
            HeaderProtectionKey::from_array([0xCC; HEADER_PROTECTION_KEY_LEN]),
        );

        let mut send_cipher = PacketCipher::new(client_keys);
        let mut recv_cipher = PacketCipher::new(server_keys);

        let mut buffer = vec![0u8; 128];
        let payload = b"hp";
        let (pn, len) = send_cipher
            .seal_into(
                0xABCD,
                PacketFlags::from_bits(PacketFlags::ACK_ELICITING),
                payload,
                &mut buffer,
            )
            .expect("seal");
        assert_eq!(pn, 0);

        let header_on_wire = &buffer[..HEADER_SIZE];

        let mut expected_header = PacketHeader::new(
            0xABCD,
            0,
            (payload.len() + AEAD_TAG_LEN) as u16,
            PacketFlags::from_bits(PacketFlags::ACK_ELICITING),
        );
        let nonce = nonce_from_packet_number(0);
        expected_header.set_nonce(*nonce.as_bytes());
        let mut expected_bytes = [0u8; HEADER_SIZE];
        expected_header.encode(&mut expected_bytes).unwrap();

        assert_ne!(header_on_wire, expected_bytes);

        let packet = &buffer[..len];
        let decrypted = recv_cipher.open(packet).expect("open");
        assert_eq!(decrypted.header().conn_id(), 0xABCD);
        assert_eq!(decrypted.header().packet_number(), 0);
        assert_eq!(decrypted.payload(), payload);
    }

    #[test]
    fn empty_payload_uses_tag_for_sample() {
        let client_keys = SessionKeys::new(
            AeadKey::from_array([0x01; AEAD_KEY_LEN]),
            AeadKey::from_array([0x02; AEAD_KEY_LEN]),
            HeaderProtectionKey::from_array([0x03; HEADER_PROTECTION_KEY_LEN]),
            HeaderProtectionKey::from_array([0x04; HEADER_PROTECTION_KEY_LEN]),
        );
        let server_keys = SessionKeys::new(
            AeadKey::from_array([0x02; AEAD_KEY_LEN]),
            AeadKey::from_array([0x01; AEAD_KEY_LEN]),
            HeaderProtectionKey::from_array([0x04; HEADER_PROTECTION_KEY_LEN]),
            HeaderProtectionKey::from_array([0x03; HEADER_PROTECTION_KEY_LEN]),
        );

        let mut send_cipher = PacketCipher::new(client_keys);
        let mut recv_cipher = PacketCipher::new(server_keys);

        let mut buffer = vec![0u8; 128];
        let (pn, len) = send_cipher
            .seal_into(0xCAFE, PacketFlags::from_bits(0), &[], &mut buffer)
            .expect("seal");
        assert_eq!(pn, 0);

        let packet = &buffer[..len];
        let decrypted = recv_cipher.open(packet).expect("open");
        assert!(decrypted.payload().is_empty());
    }
}
