//! MXP custom transport (work in progress)

mod buffer;
mod crypto;
mod handshake;
mod packet;
mod socket;
mod transport;

pub use buffer::{Buffer, BufferPool};
pub use crypto::{
    decrypt, encrypt, AeadKey, AeadNonce, AeadTag, CryptoError, HandshakeState, PrivateKey,
    PublicKey, SessionKeys, SharedSecret, AEAD_KEY_LEN, AEAD_NONCE_LEN, AEAD_TAG_LEN,
    PRIVATE_KEY_LEN, PUBLIC_KEY_LEN, SHARED_SECRET_LEN,
};
pub use handshake::{
    nonce_from_packet_number, AntiReplayStore, HandshakeError, HandshakeMessage,
    HandshakeMessageKind, Initiator, Responder,
};
pub use packet::{Frame, FrameType, PacketFlags, PacketHeader};
pub use socket::{SocketBinding, SocketError};
pub use transport::{Transport, TransportConfig, TransportHandle};
