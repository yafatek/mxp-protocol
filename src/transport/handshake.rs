//! Handshake state machines for the MXP custom transport.

use std::collections::{HashSet, VecDeque};
use std::time::{Duration, SystemTime};

use super::crypto::{
    derive_session_keys, x25519_diffie_hellman, AeadKey, AeadNonce, AeadTag, CryptoError,
    HandshakeState, PrivateKey, PublicKey, SessionKeys, AEAD_NONCE_LEN, PUBLIC_KEY_LEN,
    SHARED_SECRET_LEN,
};

/// Different handshake messages exchanged between peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeMessageKind {
    /// Initiator hello (includes ephemeral public key).
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

    /// Encode message into bytes. Format: [kind (1)][ephemeral (32)][len (u16 LE)][payload].
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
        let kind = HandshakeMessageKind::from_byte(bytes[0]).ok_or(HandshakeError::MalformedMessage)?;
        let mut key_bytes = [0u8; PUBLIC_KEY_LEN];
        key_bytes.copy_from_slice(&bytes[1..1 + PUBLIC_KEY_LEN]);
        let payload_len = u16::from_le_bytes([
            bytes[1 + PUBLIC_KEY_LEN],
            bytes[1 + PUBLIC_KEY_LEN + 1],
        ]) as usize;
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

/// Stages of the initiator handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitiatorStage {
    Ready,
    AwaitingResponse,
    AwaitingFinalization,
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
    pub fn initiate(&mut self) -> HandshakeMessage {
        let local_ephemeral = self
            .state
            .local_static()
            .derive_ephemeral(0x11);
        self.state.set_local_ephemeral(local_ephemeral.clone());
        let public_ephemeral = local_ephemeral.public_key();

        // Mix remote static into chaining key as placeholder prologue.
        self.state
            .mix_key(self.remote_static.as_bytes());

        self.stage = InitiatorStage::AwaitingResponse;
        HandshakeMessage::new(HandshakeMessageKind::InitiatorHello, public_ephemeral, Vec::new())
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

        self.anti_replay
            .record(message.payload())?
            ;

        self.state.set_remote_ephemeral(message.ephemeral().clone());

        let local_ephemeral = self
            .state
            .local_ephemeral()
            .ok_or(HandshakeError::MissingKeyMaterial)?;

        let shared = x25519_diffie_hellman(local_ephemeral, message.ephemeral())?;
        self.state.mix_key(shared.as_bytes());

        // Incorporate payload into chaining key as confirmation data.
        self.state.mix_key(message.payload());

        let confirmation = self.make_confirmation_payload();
        let final_message = HandshakeMessage::new(
            HandshakeMessageKind::InitiatorFinish,
            local_ephemeral.public_key(),
            confirmation,
        );

        let session_keys = derive_session_keys(
            self.state.chaining_key(),
            self.state.temp_key(),
            true,
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
}

impl Responder {
    /// Create a new responder with its static key.
    #[must_use]
    pub fn new(local_static: PrivateKey) -> Self {
        Self {
            state: HandshakeState::new(local_static),
            stage: ResponderStage::Ready,
            anti_replay: AntiReplayStore::new(512, Duration::from_secs(60)),
        }
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

        self.anti_replay.record(&message.encode())?;

        self.state.set_remote_ephemeral(message.ephemeral().clone());

        let local_ephemeral = self
            .state
            .local_static()
            .derive_ephemeral(0x22);
        self.state.set_local_ephemeral(local_ephemeral.clone());

        let shared = x25519_diffie_hellman(&local_ephemeral, message.ephemeral())?;
        self.state.mix_key(shared.as_bytes());

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
    ) -> Result<SessionKeys, HandshakeError> {
        if self.stage != ResponderStage::AwaitingFinal
            || message.kind() != HandshakeMessageKind::InitiatorFinish
        {
            return Err(HandshakeError::UnexpectedMessage);
        }

        self.anti_replay.record(message.payload())?;

        self.state.set_remote_ephemeral(message.ephemeral().clone());

        let local_ephemeral = self
            .state
            .local_ephemeral()
            .ok_or(HandshakeError::MissingKeyMaterial)?;

        let shared = x25519_diffie_hellman(local_ephemeral, message.ephemeral())?;
        self.state.mix_key(shared.as_bytes());
        self.state.mix_key(message.payload());

        let session_keys = derive_session_keys(
            self.state.chaining_key(),
            self.state.temp_key(),
            false,
        );

        self.stage = ResponderStage::Complete;
        Ok(session_keys)
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
        while let Some((entry, timestamp)) = self.order.front() {
            if timestamp.elapsed().unwrap_or_default() > self.ttl {
                let (entry, _) = self.order.pop_front().unwrap();
                self.seen.remove(&entry);
            } else {
                break;
            }
        }
    }
}

/// Utility function to derive a nonce from packet numbers.
#[must_use]
pub fn nonce_from_packet_number(packet_number: u64) -> AeadNonce {
    let mut bytes = [0u8; AEAD_NONCE_LEN];
    for (idx, byte) in bytes.iter_mut().enumerate() {
        *byte = packet_number.to_le_bytes()[idx % 8].wrapping_add((idx * 17) as u8);
    }
    AeadNonce::from_array(bytes)
}

