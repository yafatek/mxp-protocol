//! MXP protocol core implementation
//!
//! This module provides the wire format, message types, and codec for MXP.

mod codec;
mod error;
mod header;
mod message;
mod types;

pub use codec::{decode, encode};
pub use error::{Error, Result};
pub use header::MessageHeader;
pub use message::Message;
pub use types::{Flags, MessageType};

/// MXP magic number: "MXP1" in ASCII
pub const MAGIC_NUMBER: u32 = 0x4D58_5031;

/// Maximum payload size (16 MB)
pub const MAX_PAYLOAD_SIZE: usize = 16 * 1024 * 1024;

/// Header size in bytes (cache-aligned)
pub const HEADER_SIZE: usize = 32;

/// Checksum size in bytes
pub const CHECKSUM_SIZE: usize = 8;

/// Minimum message size (header + checksum)
pub const MIN_MESSAGE_SIZE: usize = HEADER_SIZE + CHECKSUM_SIZE;
