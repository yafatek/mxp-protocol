//! MXP custom transport (work in progress)

mod ack;
mod buffer;
mod crypto;
mod error;
mod handshake;
mod loss;
mod packet;
mod packet_crypto;
mod session;
mod socket;
mod transport;

pub use ack::{AckError, AckFrame, AckRange, DEFAULT_MAX_ACK_RANGES, ReceiveHistory};
pub use buffer::{Buffer, BufferPool};
pub use crypto::{
    AEAD_KEY_LEN, AEAD_NONCE_LEN, AEAD_TAG_LEN, AeadKey, AeadNonce, AeadTag, CryptoError,
    HEADER_PROTECTION_KEY_LEN, HEADER_PROTECTION_MASK_LEN, HEADER_PROTECTION_SAMPLE_LEN,
    HandshakeState, HeaderProtectionKey, PRIVATE_KEY_LEN, PUBLIC_KEY_LEN, PrivateKey, PublicKey,
    SHARED_SECRET_LEN, SessionKeys, SharedSecret, decrypt, encrypt, header_protection_mask,
};
pub use error::TransportError;
pub use handshake::{
    AntiReplayStore, HandshakeError, HandshakeMessage, HandshakeMessageKind, Initiator, Responder,
    ResponderOutcome, nonce_from_packet_number,
};
pub use loss::{AckOutcome, LossConfig, LossManager, SentPacketInfo};
pub use packet::{Frame, FrameType, PacketFlags, PacketHeader};
pub use packet_crypto::{DecryptedPacket, PacketCipher};
pub use session::{SessionTicket, SessionTicketManager, TICKET_ID_LEN, TICKET_SECRET_LEN};
pub use socket::{SocketBinding, SocketError};
pub use transport::{Transport, TransportConfig, TransportHandle};
