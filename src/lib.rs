//! MXP (Mesh eXchange Protocol) - High-performance protocol for agent-to-agent communication
//!
//! This library provides a reference implementation of the MXP protocol specification.
//! It includes zero-copy encoding/decoding, QUIC transport, and built-in observability.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use mxp::{Message, MessageType, MessageHeader};
//!
//! // Create a message
//! let msg = Message::new(MessageType::Call, b"Hello, agent!");
//!
//! // Encode to bytes (zero-copy)
//! let bytes = msg.encode();
//!
//! // Decode from bytes
//! let decoded = Message::decode(&bytes)?;
//! # Ok::<(), mxp::Error>(())
//! ```
//!
//! # Features
//!
//! - **Zero-copy encoding/decoding** - Direct memory mapping for performance
//! - **Type-safe message types** - Rust enums for protocol messages
//! - **Built-in checksums** - `XXHash3` for fast validation
//! - **QUIC transport** - 0-RTT connections via Quinn
//!
//! # Protocol Specification
//!
//! See [SPEC.md](https://github.com/yourusername/mxp-protocol/blob/main/SPEC.md)
//! or visit [getmxp.xyz](https://getmxp.xyz) for the complete protocol specification.

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

pub mod protocol;
pub mod transport;

pub use protocol::{
    Error, Flags, MAGIC_NUMBER, MAX_PAYLOAD_SIZE, Message, MessageHeader, MessageType, Result,
};
pub use transport::{Connection, Endpoint};

/// MXP protocol version
pub const VERSION: &str = "1.0.0-draft";

/// Default MXP port
pub const DEFAULT_PORT: u16 = 9000;
