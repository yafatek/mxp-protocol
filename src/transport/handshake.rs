//! Handshake state machines for the MXP custom transport.

use std::collections::{HashSet, VecDeque};
use std::time::{Duration, SystemTime};

use super::crypto::{
    AEAD_NONCE_LEN, AeadNonce, CryptoError, HandshakeState, PUBLIC_KEY_LEN, PrivateKey, PublicKey,
    SHARED_SECRET_LEN, SessionKeys, derive_session_keys, x25519_diffie_hellman,
};
use super::session::{SessionTicket, SessionTicketManager};

/// Different handshake messages exchanged between peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeMessageKind {
    /// Initiator hello (includes an ephemeral public key).
    InitiatorHello = 0x01,
    /// Responder hello (includes responder ephemeral key and confirmation data).
    ResponderHello = 0x02,
    /// Initiator finish (confirms key material and completes handshake).
    InitiatorFinish = 0x03,
}

impl HandshakeMessageKind {
    #[must_use]
    fn from_byte(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::InitiatorHello),
            0x02 => Some(Self::ResponderHello),
            0x03 => Some(Self::InitiatorFinish),
            _ => None,
        }
    }
}

/// Errors produced by handshake processing.
#[derive(Debug)]
pub enum HandshakeError {
    /// Unexpected message type for the current stage.
    UnexpectedMessage,
    /// Message payload malformed.
    MalformedMessage,
    /// Required key material missing.
    MissingKeyMaterial,
    /// Cryptographic failure.
    Crypto(CryptoError),
    /// Anti-replay filter rejected the message.
    ReplayDetected,
}

impl From<CryptoError> for HandshakeError {
    fn from(err: CryptoError) -> Self {
        Self::Crypto(err)
    }
}

/// Serialized handshake message.
#[derive(Debug, Clone)]
pub struct HandshakeMessage {
    kind: HandshakeMessageKind,
    ephemeral: PublicKey,
    payload: Vec<u8>,
}

impl HandshakeMessage {
    /// Create a new handshake message.
    #[must_use]
    pub fn new(kind: HandshakeMessageKind, ephemeral: PublicKey, payload: Vec<u8>) -> Self {
        Self {
            kind,
            ephemeral,
            payload,
        }
    }

    /// Encode a message into bytes. Format: [kind (1)][ephemeral (32)][len (u16 LE)][payload].
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + PUBLIC_KEY_LEN + 2 + self.payload.len());
        out.push(self.kind as u8);
        out.extend_from_slice(self.ephemeral.as_bytes());
        let len = u16::try_from(self.payload.len()).unwrap_or(0);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&self.payload);
        out
    }

    /// Decode message from bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, HandshakeError> {
        if bytes.len() < 1 + PUBLIC_KEY_LEN + 2 {
            return Err(HandshakeError::MalformedMessage);
        }
        let kind =
            HandshakeMessageKind::from_byte(bytes[0]).ok_or(HandshakeError::MalformedMessage)?;
        let mut key_bytes = [0u8; PUBLIC_KEY_LEN];
        key_bytes.copy_from_slice(&bytes[1..1 + PUBLIC_KEY_LEN]);
        let payload_len =
            u16::from_le_bytes([bytes[1 + PUBLIC_KEY_LEN], bytes[1 + PUBLIC_KEY_LEN + 1]]) as usize;
        if bytes.len() < 1 + PUBLIC_KEY_LEN + 2 + payload_len {
            return Err(HandshakeError::MalformedMessage);
        }
        let payload_start = 1 + PUBLIC_KEY_LEN + 2;
        let payload = bytes[payload_start..payload_start + payload_len].to_vec();

        Ok(Self {
            kind,
            ephemeral: PublicKey::from_array(key_bytes),
            payload,
        })
    }

    /// Access the message kind.
    #[must_use]
    pub const fn kind(&self) -> HandshakeMessageKind {
        self.kind
    }

    /// Access the ephemeral public key.
    #[must_use]
    pub fn ephemeral(&self) -> &PublicKey {
        &self.ephemeral
    }

    /// Borrow payload bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

fn mix_static_prologue(
    state: &mut HandshakeState,
    local_public: &PublicKey,
    remote_public: &PublicKey,
) -> Result<(), HandshakeError> {
    let (first, second) = if local_public.as_bytes() <= remote_public.as_bytes() {
        (local_public.as_bytes(), remote_public.as_bytes())
    } else {
        (remote_public.as_bytes(), local_public.as_bytes())
    };

    let mut combined = [0u8; PUBLIC_KEY_LEN * 2];
    combined[..PUBLIC_KEY_LEN].copy_from_slice(first);
    combined[PUBLIC_KEY_LEN..].copy_from_slice(second);
    state.mix_key(&combined)?;
    Ok(())
}

/// Stages of the initiator handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitiatorStage {
    Ready,
    AwaitingResponse,
    Complete,
}

/// Stages of the responder handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResponderStage {
    Ready,
    AwaitingFinal,
    Complete,
}

/// Represents the initiator side of the handshake.
#[derive(Debug, Clone)]
pub struct Initiator {
    state: HandshakeState,
    stage: InitiatorStage,
    remote_static: PublicKey,
    anti_replay: AntiReplayStore,
}

impl Initiator {
    /// Create a new initiator.
    #[must_use]
    pub fn new(local_static: PrivateKey, remote_static: PublicKey) -> Self {
        let mut state = HandshakeState::new(local_static);
        state.set_remote_static(remote_static.clone());
        Self {
            state,
            stage: InitiatorStage::Ready,
            remote_static,
            anti_replay: AntiReplayStore::new(512, Duration::from_secs(60)),
        }
    }

    /// Initiate the handshake by sending the first message.
    pub fn initiate(&mut self) -> Result<HandshakeMessage, HandshakeError> {
        let local_ephemeral = self.state.local_static().derive_ephemeral(0x11);
        self.state.set_local_ephemeral(local_ephemeral.clone());
        let public_ephemeral = local_ephemeral.public_key();

        let local_public = self.state.local_static().public_key();
        mix_static_prologue(&mut self.state, &local_public, &self.remote_static)?;

        self.stage = InitiatorStage::AwaitingResponse;
        Ok(HandshakeMessage::new(
            HandshakeMessageKind::InitiatorHello,
            public_ephemeral,
            Vec::new(),
        ))
    }

    /// Process the responder hello and produce the final message along with session keys.
    pub fn handle_response(
        &mut self,
        message: &HandshakeMessage,
    ) -> Result<(HandshakeMessage, SessionKeys), HandshakeError> {
        if self.stage != InitiatorStage::AwaitingResponse
            || message.kind() != HandshakeMessageKind::ResponderHello
        {
            return Err(HandshakeError::UnexpectedMessage);
        }

        self.anti_replay.record(message.payload())?;

        let remote_ephemeral = message.ephemeral().clone();
        self.state.set_remote_ephemeral(remote_ephemeral.clone());

        let local_ephemeral = self
            .state
            .local_ephemeral()
            .cloned()
            .ok_or(HandshakeError::MissingKeyMaterial)?;

        let shared = x25519_diffie_hellman(&local_ephemeral, &remote_ephemeral)?;
        self.state.mix_key(shared.as_bytes())?;

        let session_keys = derive_session_keys(&self.state, true)?;

        // Incorporate payload into a chaining key as confirmation data.
        let payload_clone = message.payload().to_vec();
        self.state.mix_key(&payload_clone)?;

        let confirmation = self.make_confirmation_payload();
        let final_message = HandshakeMessage::new(
            HandshakeMessageKind::InitiatorFinish,
            local_ephemeral.public_key(),
            confirmation,
        );

        self.stage = InitiatorStage::Complete;
        Ok((final_message, session_keys))
    }

    fn make_confirmation_payload(&self) -> Vec<u8> {
        let chaining = self.state.chaining_key();
        chaining.iter().cloned().take(16).collect()
    }
}

/// Represents the responder side of the handshake.
#[derive(Debug, Clone)]
pub struct Responder {
    state: HandshakeState,
    stage: ResponderStage,
    anti_replay: AntiReplayStore,
    tickets: SessionTicketManager,
}

impl Responder {
    /// Create a new responder with its static key and optional peer static key.
    pub fn new(
        local_static: PrivateKey,
        remote_static: Option<PublicKey>,
    ) -> Result<Self, HandshakeError> {
        let mut state = HandshakeState::new(local_static);
        if let Some(peer) = remote_static {
            let local_public = state.local_static().public_key();
            mix_static_prologue(&mut state, &local_public, &peer)?;
            state.set_remote_static(peer);
        }

        Ok(Self {
            state,
            stage: ResponderStage::Ready,
            anti_replay: AntiReplayStore::new(512, Duration::from_secs(60)),
            tickets: SessionTicketManager::new(Duration::from_secs(600), 1024),
        })
    }

    /// Process the initiator hello and produce responder hello.
    pub fn handle_initiator_hello(
        &mut self,
        message: &HandshakeMessage,
    ) -> Result<HandshakeMessage, HandshakeError> {
        if self.stage != ResponderStage::Ready
            || message.kind() != HandshakeMessageKind::InitiatorHello
        {
            return Err(HandshakeError::UnexpectedMessage);
        }

        let encoded = message.encode();
        self.anti_replay.record(&encoded)?;

        self.state.set_remote_ephemeral(message.ephemeral().clone());

        let local_ephemeral = self.state.local_static().derive_ephemeral(0x22);
        self.state.set_local_ephemeral(local_ephemeral.clone());

        let shared = x25519_diffie_hellman(&local_ephemeral, message.ephemeral())?;
        self.state.mix_key(shared.as_bytes())?;

        let mut payload = Vec::with_capacity(SHARED_SECRET_LEN);
        payload.extend_from_slice(self.state.temp_key());

        self.stage = ResponderStage::AwaitingFinal;
        Ok(HandshakeMessage::new(
            HandshakeMessageKind::ResponderHello,
            local_ephemeral.public_key(),
            payload,
        ))
    }

    /// Process the initiator finish message and finalize the handshake.
    pub fn handle_initiator_finish(
        &mut self,
        message: &HandshakeMessage,
    ) -> Result<ResponderOutcome, HandshakeError> {
        if self.stage != ResponderStage::AwaitingFinal
            || message.kind() != HandshakeMessageKind::InitiatorFinish
        {
            return Err(HandshakeError::UnexpectedMessage);
        }

        self.anti_replay.record(message.payload())?;

        // Remote ephemeral was already set during InitiatorHello; do not overwrite.
        let session_keys = derive_session_keys(&self.state, false)?;

        let payload_clone = message.payload().to_vec();
        self.state.mix_key(&payload_clone)?;

        let ticket = self.tickets.issue(self.state.chaining_key());

        self.stage = ResponderStage::Complete;
        Ok(ResponderOutcome {
            session_keys,
            session_ticket: ticket,
        })
    }
}

/// Simple anti-replay store using a hash set and queue for eviction.
#[derive(Debug, Clone)]
pub struct AntiReplayStore {
    seen: HashSet<Vec<u8>>,
    order: VecDeque<(Vec<u8>, SystemTime)>,
    capacity: usize,
    ttl: Duration,
}

impl AntiReplayStore {
    /// Create a new anti-replay store.
    #[must_use]
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            seen: HashSet::new(),
            order: VecDeque::new(),
            capacity,
            ttl,
        }
    }

    /// Record a message payload; returns error if replay detected.
    pub fn record(&mut self, payload: &[u8]) -> Result<(), HandshakeError> {
        self.evict_expired();
        let entry = payload.to_vec();
        if self.seen.contains(&entry) {
            return Err(HandshakeError::ReplayDetected);
        }
        if self.order.len() >= self.capacity {
            if let Some((old, _)) = self.order.pop_front() {
                self.seen.remove(&old);
            }
        }
        self.seen.insert(entry.clone());
        self.order.push_back((entry, SystemTime::now()));
        Ok(())
    }

    fn evict_expired(&mut self) {
        while let Some((_, timestamp)) = self.order.front() {
            if timestamp.elapsed().unwrap_or_default() > self.ttl {
                let (entry, _) = self.order.pop_front().unwrap();
                self.seen.remove(&entry);
            } else {
                break;
            }
        }
    }
}

/// Utility function to derive nonce from packet numbers.
#[must_use]
pub fn nonce_from_packet_number(packet_number: u64) -> AeadNonce {
    let mut bytes = [0u8; AEAD_NONCE_LEN];
    for (idx, byte) in bytes.iter_mut().enumerate() {
        *byte = packet_number.to_le_bytes()[idx % 8].wrapping_add((idx * 17) as u8);
    }
    AeadNonce::from_array(bytes)
}

/// Outcome of a responder-side handshake.
#[derive(Debug, Clone)]
pub struct ResponderOutcome {
    /// Session keys negotiated during the handshake.
    pub session_keys: SessionKeys,
    /// Ticket for future resumption attempts.
    pub session_ticket: SessionTicket,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::crypto::AeadKey;
    use crate::transport::{AEAD_KEY_LEN, PRIVATE_KEY_LEN};

    fn fixed_private(seed: u8) -> PrivateKey {
        let mut bytes = [0u8; PRIVATE_KEY_LEN];
        for (idx, byte) in bytes.iter_mut().enumerate() {
            *byte = seed.wrapping_add(idx as u8);
        }
        PrivateKey::from_array(bytes)
    }

    #[test]
    fn initiator_responder_handshake_roundtrip() {
        let initiator_static = fixed_private(0x10);
        let initiator_public = initiator_static.public_key();
        let responder_static = fixed_private(0x40);
        let responder_public = responder_static.public_key();

        let mut initiator = Initiator::new(initiator_static.clone(), responder_public.clone());
        let mut responder = Responder::new(responder_static, Some(initiator_public.clone()))
            .expect("responder init");

        let msg_init = initiator.initiate().expect("initiator hello");
        let msg_resp = responder
            .handle_initiator_hello(&msg_init)
            .expect("responder hello");
        let (msg_final, initiator_keys) = initiator
            .handle_response(&msg_resp)
            .expect("initiator finish");
        let outcome = responder
            .handle_initiator_finish(&msg_final)
            .expect("responder finish");

        assert_eq!(
            initiator_keys.send().as_bytes(),
            outcome.session_keys.receive().as_bytes()
        );
        assert_eq!(
            initiator_keys.receive().as_bytes(),
            outcome.session_keys.send().as_bytes()
        );
        assert!(outcome.session_ticket.is_valid());
        assert!(outcome.session_ticket.issued_at() <= outcome.session_ticket.expires_at());
    }

    #[test]
    fn initiator_rejects_wrong_message_kind() {
        let initiator_static = fixed_private(0x21);
        let responder_static = fixed_private(0x63);
        let responder_public = responder_static.public_key();

        let mut initiator = Initiator::new(initiator_static.clone(), responder_public.clone());
        let mut responder = Responder::new(responder_static, Some(initiator_static.public_key()))
            .expect("responder init");

        let msg_init = initiator.initiate().expect("initiator hello");
        let msg_resp = responder
            .handle_initiator_hello(&msg_init)
            .expect("responder hello");

        let bogus = HandshakeMessage::new(
            HandshakeMessageKind::InitiatorFinish,
            msg_resp.ephemeral().clone(),
            msg_resp.payload().to_vec(),
        );

        let err = initiator
            .handle_response(&bogus)
            .expect_err("unexpected message should fail");
        assert!(matches!(err, HandshakeError::UnexpectedMessage));
    }

    #[test]
    fn responder_rejects_wrong_message_kind() {
        let initiator_static = fixed_private(0x11);
        let initiator_public = initiator_static.public_key();
        let responder_static = fixed_private(0x51);
        let responder_public = responder_static.public_key();

        let mut initiator = Initiator::new(initiator_static, responder_public);
        let mut responder =
            Responder::new(responder_static, Some(initiator_public)).expect("responder init");

        let msg_init = initiator.initiate().expect("initiator hello");
        let msg_resp = responder
            .handle_initiator_hello(&msg_init)
            .expect("responder hello");
        let (msg_final, _) = initiator
            .handle_response(&msg_resp)
            .expect("initiator finish");

        let bogus = HandshakeMessage::new(
            HandshakeMessageKind::ResponderHello,
            msg_final.ephemeral().clone(),
            msg_final.payload().to_vec(),
        );

        let err = responder
            .handle_initiator_finish(&bogus)
            .expect_err("unexpected finish should fail");
        assert!(matches!(err, HandshakeError::UnexpectedMessage));
    }

    #[test]
    fn anti_replay_store_rejects_duplicates() {
        let mut store = AntiReplayStore::new(8, Duration::from_secs(10));
        let payload = b"handshake payload";

        store.record(payload).expect("first insert ok");
        let err = store.record(payload).expect_err("replay must be rejected");
        assert!(matches!(err, HandshakeError::ReplayDetected));
    }

    #[test]
    fn responder_session_resumption_validates_secret() {
        let mut manager = SessionTicketManager::new(Duration::from_secs(60), 4);
        let seed = [0xAAu8; SHARED_SECRET_LEN];
        let ticket = manager.issue(&seed);

        let resume = manager
            .resume(ticket.id(), &seed)
            .expect("ticket should resume");

        assert_eq!(resume.id(), ticket.id());
        assert_eq!(resume.secret(), ticket.secret());
    }

    #[test]
    fn nonce_derivation_varies_with_packet_number() {
        let nonce_a = nonce_from_packet_number(1);
        let nonce_b = nonce_from_packet_number(2);
        assert_ne!(nonce_a.as_bytes(), nonce_b.as_bytes());

        // Basic sanity that derived nonce size matches AEAD requirements.
        let key = AeadKey::from_array([0x11u8; AEAD_KEY_LEN]);
        let plaintext = [0x22u8; 8];
        let (cipher, tag) = super::super::crypto::encrypt(&key, &nonce_a, &plaintext, &[]);
        let decrypted =
            super::super::crypto::decrypt(&key, &nonce_a, &cipher, &[], &tag).expect("decrypt");
        assert_eq!(plaintext.to_vec(), decrypted);
    }
}
