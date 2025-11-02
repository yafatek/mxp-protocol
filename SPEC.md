# MXP (Mesh eXchange Protocol) Specification

**Version:** 1.0.0-draft  
**Status:** In Development  
**Author:** Feras  
**Reference Implementation:** [mxp-protocol](https://github.com/yourusername/mxp-protocol) (Rust)  
**Official Site:** [getmxp.xyz](https://getmxp.xyz)

## Overview

MXP (Mesh eXchange Protocol) is a binary protocol designed for high-performance agent-to-agent communication. It features a custom UDP-based transport layer with integrated encryption, reliable streams, and efficient serialization optimized specifically for AI agent workloads.

## Design Goals

1. **Performance:** Sub-millisecond latency, high throughput
2. **Observability:** Every message is traceable
3. **Simplicity:** Easy to implement and debug
4. **Extensibility:** Forward-compatible design
5. **Zero-Copy:** Minimize memory allocations

## Transport Layer

### MXP Custom Transport
- **Carrier Protocol:** UDP
- **Port:** 9000 (default, configurable)
- **Encryption:** ChaCha20-Poly1305 / AES-GCM (AEAD, mandatory)
- **Handshake:** Noise IK-inspired pattern with X25519
- **Streams:** Bidirectional and unidirectional reliable streams
- **Datagrams:** Unreliable message support
- **Connection ID:** 64-bit identifier
- **Packet Number:** 64-bit monotonic counter

### Why Custom Transport?
- **Zero external dependencies:** Pure Rust implementation, no QUIC library overhead
- **Agent-optimized:** Designed specifically for AI agent communication patterns
- **Minimal handshake:** Fast connection establishment with session resumption
- **Built-in observability:** Native tracing and metrics without external instrumentation
- **Predictable performance:** No black-box behavior, full control over scheduling and congestion
- **Tunable reliability:** Mix reliable streams with unreliable datagrams as needed

## Wire Format

### Message Structure

Every message consists of three parts:

```
┌────────────────────────────────┐
│  HEADER (32 bytes, aligned)    │
├────────────────────────────────┤
│  PAYLOAD (variable length)     │
├────────────────────────────────┤
│  CHECKSUM (8 bytes)            │
└────────────────────────────────┘
```

### Header Format (32 bytes)

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        Magic Number (4 bytes)                 |
|                         0x4D585031 ("MXP1")                   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Message Type  |     Flags     |          Reserved             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                      Message ID (8 bytes)                     +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                      Trace ID (8 bytes)                       +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                   Payload Length (8 bytes)                    +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

**Fields:**
- **Magic Number (4 bytes):** 0x4D585031 ("MXP1" in ASCII)
- **Message Type (1 byte):** Type of message (see below)
- **Flags (1 byte):** Bit flags for message properties
- **Reserved (2 bytes):** For future use, must be 0
- **Message ID (8 bytes):** Unique message identifier
- **Trace ID (8 bytes):** Distributed tracing identifier
- **Payload Length (8 bytes):** Length of payload in bytes

### Message Types

```rust
0x01 - AgentRegister      // Register agent with mesh
0x02 - AgentDiscover      // Discover agents by capability
0x03 - AgentHeartbeat     // Keep-alive / health check

0x10 - Call               // Synchronous RPC call
0x11 - Response           // Response to Call
0x12 - Event              // Async event (fire-and-forget)

0x20 - StreamOpen         // Open new stream
0x21 - StreamChunk        // Stream data chunk
0x22 - StreamClose        // Close stream

0xF0 - Ack                // Acknowledgment
0xF1 - Error              // Error response
```

### Flags (1 byte)

```
Bit 0: COMPRESSED   - Payload is compressed (zstd)
Bit 1: ENCRYPTED    - Payload is encrypted (E2E)
Bit 2: REQUIRES_ACK - Sender wants acknowledgment
Bit 3: FINAL        - Last message in sequence
Bit 4-7: Reserved   - Must be 0
```

### Checksum (8 bytes)

- **Algorithm:** XXHash3 (64-bit)
- **Coverage:** Header + Payload (everything except checksum itself)
- **Purpose:** Detect corruption, fast verification

## URL Scheme

MXP uses a custom URL scheme:

```
mxp://host:port/agent-id

Examples:
- mxp://localhost:9000
- mxp://mesh.example.com:9000/agent-123
- mxp://10.0.1.5:9000
```

## Message Types Detail

### 0x01 - AgentRegister

Register an agent with the mesh.

**Payload Format:**
```
┌──────────────────────────────────┐
│ Agent ID (16 bytes, UUID)        │
├──────────────────────────────────┤
│ Name Length (2 bytes, u16)       │
├──────────────────────────────────┤
│ Agent Name (variable, UTF-8)     │
├──────────────────────────────────┤
│ Capabilities Count (2 bytes, u16)│
├──────────────────────────────────┤
│ Capability 1 Length (2 bytes)    │
│ Capability 1 (variable, UTF-8)   │
│ ...                              │
├──────────────────────────────────┤
│ Endpoint (6 bytes, IP:Port)      │
└──────────────────────────────────┘
```

### 0x10 - Call

Synchronous RPC call to another agent.

**Payload Format:**
```
┌──────────────────────────────────┐
│ Target Agent ID (16 bytes, UUID) │
├──────────────────────────────────┤
│ Timeout (4 bytes, u32 seconds)   │
├──────────────────────────────────┤
│ Call Data (variable, bytes)      │
└──────────────────────────────────┘
```

### 0x20 - StreamOpen

Open a new stream for multi-chunk data transfer.

**Payload Format:**
```
┌──────────────────────────────────┐
│ Stream ID (16 bytes, UUID)       │
├──────────────────────────────────┤
│ Target Agent ID (16 bytes, UUID) │
├──────────────────────────────────┤
│ Stream Type (1 byte)             │
│   0x01 - Unidirectional          │
│   0x02 - Bidirectional           │
└──────────────────────────────────┘
```

## Performance Characteristics

### Message Overhead
- Header: 32 bytes
- Checksum: 8 bytes
- **Total:** 40 bytes per message

### Latency Budget
```
Operation                    Target
─────────────────────────────────────
Message encode              < 1 μs
Message decode              < 1 μs
Transport send              < 100 μs
Network (RTT)               ~ 50 ms
Transport receive           < 100 μs
Total (same datacenter)     < 1 ms
Total (cross-region)        ~ 50 ms
```

### Throughput
- Target: 100K messages/second per connection
- Max payload size: 16 MB
- Typical payload: 1-64 KB

## Security Considerations

### Transport Security
- **Handshake:** Noise IK-inspired pattern (3-message handshake)
- **Key Exchange:** X25519 (Curve25519 Diffie-Hellman)
- **AEAD Cipher:** ChaCha20-Poly1305 or AES-GCM (mandatory for all transport payloads)
- **Header Protection:** ChaCha20-based header masking (obfuscates packet numbers and flags)
- **Perfect Forward Secrecy:** Ephemeral keys for each connection
- **Anti-Replay:** Connection-level packet number tracking and anti-replay store
- **Session Resumption:** Optional session tickets for fast reconnection

### Handshake Flow
```
Initiator                         Responder
   |                                  |
   |------ InitiatorHello -------->  |  (ephemeral public key)
   |                                  |
   |<----- ResponderHello ---------  |  (ephemeral public key + confirmation)
   |                                  |
   |------ InitiatorFinish -------->  |  (confirmation data)
   |                                  |
   |<==== Encrypted Data =========>  |  (ChaCha20-Poly1305/AES-GCM)
```

### Message Security
- Optional E2E encryption (flag 0x02) on top of transport encryption
- Message signing (future enhancement)
- Rate limiting per agent

## Observability

### Tracing
Every message includes:
- Trace ID (64-bit)
- Message ID (64-bit) 
- Timestamp (implicit from send time)
- Source and destination agent IDs (in payload)

### Metrics
Automatically collected by transport layer:
- Message count by type (per MessageType enum)
- Latency distribution (send/receive, max, average)
- Error rate
- Connection count (active connections)
- Stream count (active streams)
- Flow control: bytes consumed, window updates
- Datagram queue: enqueued/sent counts and bytes
- Scheduler: priority class distribution (Control/Interactive/Bulk)

## Compatibility

### Versioning
- Magic number includes implicit version ("MXP1")
- Future versions: "MXP2", "MXP3", etc.
- Backward compatibility guaranteed within major version

### Extension Points
- Reserved header bytes
- Custom message types (0x80-0xEF)
- Flags for new features

## Implementation Notes

### Packet Structure (Transport Layer)

In addition to the MXP message format above, the transport layer uses its own packet structure:

```
Transport Packet Format (32-byte header + encrypted payload):
┌─────────────────────────────────────────┐
│ Connection ID (8 bytes)                 │
├─────────────────────────────────────────┤
│ Packet Number (8 bytes)                 │
├─────────────────────────────────────────┤
│ Flags (1 byte) | Reserved (1 byte)      │
├─────────────────────────────────────────┤
│ Payload Length (2 bytes)                │
├─────────────────────────────────────────┤
│ Nonce (12 bytes)                        │
├─────────────────────────────────────────┤
│ Encrypted Payload (variable)            │
├─────────────────────────────────────────┤
│ AEAD Tag (16 bytes)                     │
└─────────────────────────────────────────┘
```

**Transport Packet Flags:**
- `HANDSHAKE (0x01)`: Packet contains handshake data
- `ACK_ELICITING (0x02)`: Packet requires acknowledgment
- `ACK (0x04)`: Packet carries acknowledgments
- `KEY_PHASE (0x08)`: Key phase transition signal
- `PROBE (0x10)`: Keepalive/path validation

**Transport Frames** (inside encrypted payload):
- `StreamOpen`: Open a new reliable stream
- `StreamData`: Carry stream data at offset
- `StreamFin`: Close a stream with FIN
- `Datagram`: Unreliable datagram payload
- `Ack`: Acknowledgment ranges
- `Crypto`: Handshake data
- `Control`: Connection control messages
- `StreamMaxData`: Per-stream flow control credit
- `ConnectionMaxData`: Connection-level flow control

### Zero-Copy Optimization
```rust
// Header is directly cast from bytes (unsafe but fast)
let header: &MessageHeader = unsafe {
    &*(bytes.as_ptr() as *const MessageHeader)
};

// Payload is a zero-copy slice
let payload = &bytes[32..32 + header.payload_len];
```

### Endianness
- All multi-byte fields are **little-endian**
- Matches x86/ARM architecture

### Alignment
- **MXP Message Header:** 32 bytes (cache-line aligned)
- **Transport Packet Header:** 32 bytes (cache-line aligned)
- Both headers fit in half a cache line, improving CPU cache performance

### Cryptographic Primitives
MXP implements its own cryptographic primitives (no external crypto libraries):
- **X25519:** Elliptic Curve Diffie-Hellman (key exchange)
- **ChaCha20:** Stream cipher (encryption)
- **Poly1305:** MAC (authentication)
- **HKDF:** HMAC-based key derivation
- **HMAC-SHA256:** Key derivation PRF
- **XXHash3:** Fast checksumming for MXP messages

### Buffer Management
- **Buffer Pool:** Reusable buffers to minimize allocations
- **Zero-Copy Slicing:** Uses `bytes::Bytes` for shared ownership without copying
- **Configurable Buffer Size:** Default 2048 bytes, max pool size 1024 buffers

## Future Enhancements

### Version 1.1
- Compression support (zstd)
- Connection migration
- Improved congestion control algorithms
- NAT traversal and hole punching

### Version 2.0
- WebAssembly agent runtime
- Browser support via WebTransport
- Cross-mesh federation

## References

- **X25519:** Curve25519 key exchange (RFC 7748)
- **ChaCha20-Poly1305:** AEAD cipher (RFC 7539)
- **Noise Protocol:** Handshake patterns (noiseprotocol.org)
- **XXHash:** Extremely fast non-cryptographic hash
- **OpenTelemetry:** Distributed tracing specification

---

**Official Site:** [getmxp.xyz](https://getmxp.xyz)  
**Implementation:** [mxp-protocol](https://github.com/yourusername/mxp-protocol)  
**Contact:** protocol@getmxp.xyz

