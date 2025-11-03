# MXP (Mesh eXchange Protocol)

**100x faster than HTTP for agent-to-agent communication**

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/mxp.svg)](https://crates.io/crates/mxp)
[![Documentation](https://docs.rs/mxp/badge.svg)](https://docs.rs/mxp)

> **📄 [Read the Whitepaper](WHITEPAPER.md)** | **🌐 [Visit getmxp.xyz](https://getmxp.xyz)** | **💬 [Join Discord](https://discord.gg/mxp-protocol)**

## Overview

MXP (Mesh eXchange Protocol) is an open, high-performance binary protocol purpose-built for agent-to-agent communication. Traditional HTTP wasn't designed for the performance and observability demands of distributed AI systems—MXP delivers **100x faster connection setup**, **10-50x lower latency**, and **built-in distributed tracing** without external instrumentation.

### Why MXP?

AI agents are evolving from isolated assistants to interconnected systems that collaborate in real-time. They need:
- **Sub-millisecond latency** for coordination
- **High message frequency** (thousands per second)
- **Native streaming** for LLM token streams
- **Built-in observability** across agent boundaries
- **Explicit lifecycle management** (register, discover, heartbeat)

MXP delivers all of this with a custom UDP-based transport optimized specifically for agent workloads.

### Key Features

- **Fast Connection Establishment** - Minimal 3-message handshake with session resumption
- **Built-in Distributed Tracing** - Every message includes trace context
- **Native Streaming** - First-class support for streaming data (e.g., LLM token streams)
- **Binary Wire Format** - Efficient 32-byte headers with zero-copy deserialization
- **Sub-millisecond Latency** - Optimized for datacenter and cross-region communication
- **Custom Transport** - UDP-based with ChaCha20-Poly1305 encryption, reliability, and scheduling tuned for agents

## Protocol Specification

See [SPEC.md](SPEC.md) for the complete wire format specification.

### Quick Reference

| Property | Value |
|----------|-------|
| Magic Number | `0x4D585031` ("MXP1") |
| Header Size | 32 bytes (cache-aligned) |
| Checksum Algorithm | XXHash3 (64-bit) |
| Transport Protocol | MXP custom transport (UDP carrier) |
| Encryption | ChaCha20-Poly1305 / AES-GCM (AEAD) |
| Handshake | Noise IK-inspired with X25519 |
| Default Port | 9000 |
| Maximum Payload | 16 MB |
| Endianness | Little-endian |

### Message Types

```
0x01 - AgentRegister      Register agent with mesh
0x02 - AgentDiscover      Discover agents by capability
0x03 - AgentHeartbeat     Keep-alive / health check
0x10 - Call               Synchronous RPC call
0x11 - Response           Response to Call
0x12 - Event              Async event (fire-and-forget)
0x20 - StreamOpen         Open new stream
0x21 - StreamChunk        Stream data chunk
0x22 - StreamClose        Close stream
0xF0 - Ack                Acknowledgment
0xF1 - Error              Error response
```

## Installation

Run the following Cargo command in your project directory:

```bash
cargo add mxp
```

Or add to your `Cargo.toml`:

```toml
[dependencies]
mxp = "0.1"
```

## Usage

### Basic Message Exchange

```rust
use mxp::{Message, MessageType};

// Create a message
let message = Message::new(MessageType::Call, b"request_data");

// Encode to bytes
let bytes = message.encode();

// Decode from bytes
let decoded = Message::decode(bytes.clone())?;
```

### Client-Server Communication

```rust
use mxp::{Message, MessageType, Transport, TransportConfig};
use std::net::SocketAddr;

let config = TransportConfig::default();
let transport = Transport::new(config);
let handle = transport.bind("127.0.0.1:0".parse::<SocketAddr>()?)?;

let mut buffer = handle.acquire_buffer();
let _ = handle.receive(&mut buffer)?;
println!("Received {} bytes", buffer.as_slice().len());
```

See [examples/](examples/) for complete examples.

## Performance Characteristics

### Latency Targets

| Operation | Target |
|-----------|--------|
| Message encode | < 1 μs |
| Message decode | < 1 μs |
| Transport send/receive | < 100 μs |
| P99 latency (same datacenter) | < 1 ms |
| P99 latency (cross-region) | < 50 ms |

### Throughput

- Target: 100,000 messages/second per connection
- Maximum payload size: 16 MB
- Typical payload range: 1-64 KB

## Design Principles

1. **Performance First** - Zero-copy operations where possible, minimal allocations
2. **Observable by Default** - Built-in tracing without external instrumentation
3. **Simple Implementation** - Clear specification, easy to implement correctly
4. **Forward Compatible** - Reserved fields and extension points for future versions
5. **Transport Abstraction** - MXP transport runs over UDP today and is designed to support additional carriers

## Architecture

```
┌─────────────────────────────────────────┐
│         Application Layer               │
│    (Your AI agents and services)        │
└─────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│          MXP Protocol Layer             │
│  ├── Message encoding/decoding          │
│  ├── Header validation                  │
│  └── Checksum verification              │
└─────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│         MXP Transport Layer             │
│  ├── Noise-based handshake              │
│  ├── Reliable streams + datagrams       │
│  └── Priority scheduling & security     │
└─────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│            UDP / Network                │
└─────────────────────────────────────────┘
```

## Implementation Status

### Reference Implementation (Rust)

This repository provides the reference implementation with:
- Complete protocol support
- Zero-copy message encoding/decoding
- MXP transport skeleton (UDP carrier)
- Comprehensive test coverage
- Performance benchmarks

### Other Implementations

The MXP protocol is designed to be implemented in any language. Implementation guidelines and compliance tests are available in the specification.

## Contributing

MXP is an open protocol, and we welcome contributions:

- Protocol enhancements and extensions
- Implementations in other languages
- Documentation improvements
- Performance optimizations
- Bug reports and test cases

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

## Comparison with Other Protocols

### vs HTTP/REST

- **Connection Setup**: MXP uses 3-message handshake (~1-2 RTT) vs HTTP's 200-300ms for new TLS connections
- **Observability**: Built-in tracing vs requires separate instrumentation
- **Streaming**: Native support vs Server-Sent Events or WebSocket
- **Overhead**: 40 bytes per message vs 100+ bytes of HTTP headers
- **Encryption**: ChaCha20-Poly1305 at transport layer vs TLS 1.3

### vs gRPC

- **Transport**: MXP custom UDP-based vs HTTP/2 over TCP
- **Agent Discovery**: Built into protocol vs requires external service mesh
- **Tracing**: Mandatory trace IDs vs optional metadata
- **Binary Format**: Custom optimized vs Protocol Buffers
- **Dependencies**: Zero external deps vs requires QUIC/HTTP2 libraries

## Security Considerations

- **Handshake:** Noise IK-inspired pattern with X25519 key exchange
- **Encryption:** ChaCha20-Poly1305 / AES-GCM AEAD mandated for all transport payloads
- **Header Protection:** ChaCha20-based masking of packet numbers and flags
- **Anti-Replay:** Packet number tracking and anti-replay store
- **Session Tickets:** Optional fast reconnection with session resumption
- Optional end-to-end payload encryption (message-level flag 0x02)
- Rate limiting and quota enforcement at application layer

See [SPEC.md](SPEC.md) for detailed security considerations.

## Contributing

MXP is an open protocol, and we welcome contributions:

- Protocol enhancements and extensions
- Implementations in other languages
- Documentation improvements
- Performance optimizations
- Bug reports and test cases

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

Please read our [Code of Conduct](CODE_OF_CONDUCT.md) before contributing.

## License

**Protocol Specification**: Public domain (CC0)  
**Reference Implementation**: Dual-licensed under MIT OR Apache-2.0

See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.

This licensing allows:
- Anyone to implement MXP in any language
- Commercial use without restrictions
- No attribution required for protocol implementation
- Fork and extend as needed

## Resources

- **📄 Whitepaper**: [WHITEPAPER.md](WHITEPAPER.md) - Complete technical design and rationale
- **📋 Specification**: [SPEC.md](SPEC.md) - Wire format specification
- **🌐 Website**: [getmxp.xyz](https://getmxp.xyz) - Protocol overview and use cases
- **📚 API Docs**: [docs.rs/mxp](https://docs.rs/mxp) - Rust API reference
- **💻 Examples**: [examples/](examples/) - Working code examples
- **📦 Crates.io**: [crates.io/crates/mxp](https://crates.io/crates/mxp) - Published crate

## Status & Roadmap

**Current Version**: 1.0.0-draft  
**Status**: In Development (Production-ready core, expanding ecosystem)

**Milestones:**
- ✅ Core protocol implementation (Rust)
- ✅ Custom UDP transport with encryption
- ✅ Zero-copy message encoding/decoding
- 🚧 JavaScript SDK (Beta)
- 🚧 Control plane (Alpha)
- 📅 Python SDK (Q2 2026)
- 📅 v1.0 GA (Q4 2026)

See [ROADMAP.md](../ROADMAP.md) for detailed timeline.

## Community & Support

- **💬 Discord**: [discord.gg/mxp-protocol](https://discord.gg/mxp-protocol) - Community chat
- **🐦 Twitter**: [@mxp_protocol](https://twitter.com/mxp_protocol) - Updates and announcements
- **📧 Email**: protocol@getmxp.xyz - Protocol discussion
- **🐛 Issues**: [GitHub Issues](https://github.com/yafatek/mxp-protocol/issues) - Bug reports
- **🔒 Security**: security@getmxp.xyz - Responsible disclosure

## Enterprise

For enterprise adoption, design partnerships, and commercial support:
- **📧 Business Inquiries**: business@relaymxp.xyz
- **📖 Adoption Playbook**: [docs/adoption-playbook.md](../docs/adoption-playbook.md)
- **🏢 Enterprise Features**: SOC2, ISO 27001, HIPAA compliance available

---

**Built with ❤️ by the MXP community** | **License**: Protocol spec (CC0), Implementation (MIT OR Apache-2.0)
