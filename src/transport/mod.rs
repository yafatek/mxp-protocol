//! MXP custom transport (work in progress)

mod buffer;
mod crypto;
mod handshake;
mod packet;
mod session;
mod socket;
mod transport;

pub use buffer::{Buffer, BufferPool};
pub use crypto::{
    AEAD_KEY_LEN, AEAD_NONCE_LEN, AEAD_TAG_LEN, AeadKey, AeadNonce, AeadTag, CryptoError,
    HandshakeState, PRIVATE_KEY_LEN, PUBLIC_KEY_LEN, PrivateKey, PublicKey, SHARED_SECRET_LEN,
    SessionKeys, SharedSecret, decrypt, encrypt,
};
pub use handshake::{
    AntiReplayStore, HandshakeError, HandshakeMessage, HandshakeMessageKind, Initiator, Responder,
    ResponderOutcome, nonce_from_packet_number,
};
pub use packet::{Frame, FrameType, PacketFlags, PacketHeader};
pub use session::{SessionTicket, SessionTicketManager, TICKET_ID_LEN, TICKET_SECRET_LEN};
pub use socket::{SocketBinding, SocketError};
pub use transport::{Transport, TransportConfig, TransportHandle};
