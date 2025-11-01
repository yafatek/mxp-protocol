//! MXP error types

use thiserror::Error;

/// MXP protocol errors
#[derive(Error, Debug)]
pub enum Error {
    /// Invalid magic number
    #[error("invalid magic number: expected 0x4D585031, got {found:#x}")]
    InvalidMagic {
        /// Found magic number
        found: u32,
    },

    /// Invalid message type
    #[error("invalid message type: {type_byte:#x}")]
    InvalidMessageType {
        /// Invalid type byte
        type_byte: u8,
    },

    /// Checksum mismatch
    #[error("checksum mismatch: expected {expected:#x}, got {found:#x}")]
    ChecksumMismatch {
        /// Expected checksum
        expected: u64,
        /// Found checksum
        found: u64,
    },

    /// Payload too large
    #[error("payload too large: {size} bytes (max {max})")]
    PayloadTooLarge {
        /// Payload size
        size: usize,
        /// Maximum allowed
        max: usize,
    },

    /// Buffer too small
    #[error("buffer too small: need {needed} bytes, got {got}")]
    BufferTooSmall {
        /// Needed size
        needed: usize,
        /// Actual size
        got: usize,
    },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// QUIC connection error
    #[error("QUIC connection error: {0}")]
    Connection(String),

    /// QUIC stream error
    #[error("QUIC stream error: {0}")]
    Stream(String),

    /// Invalid UTF-8
    #[error("invalid UTF-8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),

    /// Other error
    #[error("{0}")]
    Other(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;
