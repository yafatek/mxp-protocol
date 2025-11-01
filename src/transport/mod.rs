//! MXP custom transport (work in progress)

mod ack;
mod anti_amplification;
mod buffer;
mod congestion;
mod crypto;
mod datagram;
mod error;
mod flow;
mod handshake;
mod loss;
mod packet;
mod packet_crypto;
mod scheduler;
mod session;
mod socket;
mod stream;
mod transport;

pub use ack::{AckError, AckFrame, AckRange, DEFAULT_MAX_ACK_RANGES, ReceiveHistory};
pub use anti_amplification::{
    AmplificationConfig, AntiAmplificationGuard, DEFAULT_AMPLIFICATION_FACTOR,
};
pub use buffer::{Buffer, BufferPool};
pub use congestion::{CongestionConfig, CongestionController};
pub use crypto::{
    AEAD_KEY_LEN, AEAD_NONCE_LEN, AEAD_TAG_LEN, AeadKey, AeadNonce, AeadTag, CryptoError,
    HEADER_PROTECTION_KEY_LEN, HEADER_PROTECTION_MASK_LEN, HEADER_PROTECTION_SAMPLE_LEN,
    HandshakeState, HeaderProtectionKey, PRIVATE_KEY_LEN, PUBLIC_KEY_LEN, PrivateKey, PublicKey,
    SHARED_SECRET_LEN, SessionKeys, SharedSecret, decrypt, encrypt, header_protection_mask,
};
pub use datagram::{
    DEFAULT_DATAGRAM_MAX_PAYLOAD, DEFAULT_DATAGRAM_QUEUE, DatagramConfig, DatagramError,
    DatagramQueue,
};
pub use error::TransportError;
pub use flow::{FlowControlError, FlowController, FlowWindow};
pub use handshake::{
    AntiReplayStore, HandshakeError, HandshakeMessage, HandshakeMessageKind, Initiator, Responder,
    ResponderOutcome, nonce_from_packet_number,
};
pub use loss::{AckOutcome, LossConfig, LossManager, SentPacketInfo};
pub use packet::{Frame, FrameType, HEADER_SIZE, PacketFlags, PacketHeader};
pub use packet_crypto::{DecryptedPacket, PacketCipher};
pub use scheduler::{PriorityClass, Scheduler};
pub use session::{SessionTicket, SessionTicketManager, TICKET_ID_LEN, TICKET_SECRET_LEN};
pub use socket::{SocketBinding, SocketError};
pub use stream::{
    EndpointRole, SendChunk, Stream, StreamError, StreamId, StreamKind, StreamManager,
};
pub use transport::{Transport, TransportConfig, TransportHandle};
