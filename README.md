# MXP (Mesh eXchange Protocol)

**High-performance binary protocol for agent-to-agent communication**

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/mxp.svg)](https://crates.io/crates/mxp)
[![Documentation](https://docs.rs/mxp/badge.svg)](https://docs.rs/mxp)

## Overview

MXP (Mesh eXchange Protocol) is an open, high-performance binary protocol designed specifically for agent-to-agent communication in distributed systems. Built on QUIC (RFC 9000), MXP provides zero-round-trip-time connections, built-in observability, and native streaming support optimized for AI agent workloads.

### Key Features

- **Zero-RTT Connection Establishment** - Resume connections without handshake overhead
- **Built-in Distributed Tracing** - Every message includes trace context
- **Native Streaming** - First-class support for streaming data (e.g., LLM token streams)
- **Binary Wire Format** - Efficient 32-byte headers with zero-copy deserialization
- **Sub-millisecond Latency** - Optimized for datacenter and cross-region communication
- **QUIC Transport** - Leverages modern transport with multiplexing and congestion control

## Protocol Specification

See [SPEC.md](SPEC.md) for the complete wire format specification.

### Quick Reference

| Property | Value |
|----------|-------|
| Magic Number | `0x4D585031` ("MXP1") |
| Header Size | 32 bytes (cache-aligned) |
| Checksum Algorithm | XXHash3 (64-bit) |
| Transport Protocol | QUIC (RFC 9000) over UDP |
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

Add to your `Cargo.toml`:

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
let decoded = Message::decode(&bytes)?;
```

### Client-Server Communication

```rust
use mxp::{Endpoint, Message, MessageType};

// Client
let endpoint = Endpoint::client("127.0.0.1:0".parse()?)?;
let conn = endpoint.connect("127.0.0.1:9000".parse()?, "server").await?;

let request = Message::new(MessageType::Call, b"ping");
conn.send(&request).await?;

let response = conn.recv().await?.unwrap();
println!("Response: {:?}", response.payload());
```

See [examples/](examples/) for complete examples.

## Performance Characteristics

### Latency Targets

| Operation | Target |
|-----------|--------|
| Message encode | < 1 μs |
| Message decode | < 1 μs |
| QUIC send/receive | < 100 μs |
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
5. **Transport Agnostic** - While QUIC is recommended, protocol works over any reliable transport

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
│         QUIC Transport Layer            │
│  ├── 0-RTT connection establishment     │
│  ├── Stream multiplexing                │
│  └── TLS 1.3 encryption                 │
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
- QUIC transport via Quinn
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

- **Connection Setup**: MXP uses 0-RTT vs HTTP's 200-300ms for new connections
- **Observability**: Built-in tracing vs requires separate instrumentation
- **Streaming**: Native support vs Server-Sent Events or WebSocket
- **Overhead**: 40 bytes per message vs 100+ bytes of HTTP headers

### vs gRPC

- **Transport**: QUIC vs HTTP/2 over TCP
- **Agent Discovery**: Built into protocol vs requires external service mesh
- **Tracing**: Mandatory trace IDs vs optional metadata
- **Binary Format**: Custom optimized vs Protocol Buffers

## Security Considerations

- TLS 1.3 encryption mandatory (via QUIC)
- Certificate-based authentication
- Optional end-to-end payload encryption
- Rate limiting and quota enforcement at application layer

See [SPEC.md](SPEC.md) for detailed security considerations.

## License

**Protocol Specification**: Public domain (CC0)  
**Reference Implementation**: Dual-licensed under MIT OR Apache-2.0

This licensing allows:
- Anyone to implement MXP in any language
- Commercial use without restrictions
- No attribution required for protocol implementation
- Fork and extend as needed

## Resources

- **Technical Documentation**: [docs.getmxp.xyz](https://yafatek.github.io/mxp-protocol/) - Complete protocol guide
- **Specification**: [SPEC.md](SPEC.md) - Wire format specification
- **API Documentation**: [docs.rs/mxp](https://docs.rs/mxp) - Rust API reference
- **Examples**: [examples/](examples/) - Working code examples
- **Website**: [getmxp.xyz](https://getmxp.xyz) - Protocol overview
- **Crates.io**: [crates.io/crates/mxp](https://crates.io/crates/mxp) - Published crate

## Status

**Version**: 1.0.0-draft  
**Status**: In Development  
**Stability**: Experimental - Protocol may change before 1.0 release

## Contact

- **Protocol Discussion**: protocol@getmxp.xyz
- **Issues**: [GitHub Issues](https://github.com/yourusername/mxp-protocol/issues)
- **Security**: security@getmxp.xyz
