# MXP (Mesh eXchange Protocol) Specification

**Version:** 1.0.0-draft  
**Status:** In Development  
**Author:** Feras  
**Reference Implementation:** [mxp-protocol](https://github.com/yourusername/mxp-protocol) (Rust)  
**Official Site:** [getmxp.xyz](https://getmxp.xyz)

## Overview

MXP (Mesh eXchange Protocol) is a binary protocol designed for high-performance agent-to-agent communication. It runs over QUIC (UDP) and provides built-in observability, streaming, and efficient serialization.

## Design Goals

1. **Performance:** Sub-millisecond latency, high throughput
2. **Observability:** Every message is traceable
3. **Simplicity:** Easy to implement and debug
4. **Extensibility:** Forward-compatible design
5. **Zero-Copy:** Minimize memory allocations

## Transport Layer

### QUIC
- **Protocol:** QUIC (RFC 9000)
- **Port:** 9000 (default, configurable)
- **Encryption:** TLS 1.3 (mandatory)
- **Streams:** Bidirectional and unidirectional
- **Connection ID:** 128-bit random

### Why QUIC?
- 0-RTT connection establishment
- Multiplexing without head-of-line blocking
- Built-in encryption
- Connection migration
- Modern, battle-tested

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
QUIC send                   < 100 μs
Network (RTT)               ~ 50 ms
QUIC receive                < 100 μs
Total (same datacenter)     < 1 ms
Total (cross-region)        ~ 50 ms
```

### Throughput
- Target: 100K messages/second per connection
- Max payload size: 16 MB
- Typical payload: 1-64 KB

## Security Considerations

### Transport Security
- TLS 1.3 mandatory (via QUIC)
- Perfect forward secrecy
- Certificate validation required

### Message Security
- Optional E2E encryption (flag 0x02)
- Message signing (future enhancement)
- Rate limiting per agent

## Observability

### Tracing
Every message includes:
- Trace ID (64-bit)
- Timestamp (implicit from QUIC)
- Source and destination agent IDs

### Metrics
Automatically collected:
- Message count by type
- Latency distribution
- Error rate
- Connection count
- Stream count

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
- Header is 32 bytes (cache-line aligned)
- Improves CPU cache performance

## Future Enhancements

### Version 1.1
- Compression support (zstd)
- Multiplexing multiple messages per QUIC stream
- Priority queues

### Version 2.0
- WebAssembly agent runtime
- Browser support via WebTransport
- Cross-mesh federation

## References

- RFC 9000 - QUIC: A UDP-Based Multiplexed and Secure Transport
- RFC 8446 - The Transport Layer Security (TLS) Protocol Version 1.3
- XXHash - Extremely fast hash algorithm
- OpenTelemetry - Distributed tracing specification

---

**Official Site:** [getmxp.xyz](https://getmxp.xyz)  
**Implementation:** [mxp-protocol](https://github.com/yourusername/mxp-protocol)  
**Contact:** protocol@getmxp.xyz

