# MXP (Mesh eXchange Protocol)

> **Open protocol specification for agent-to-agent communication**  
> The networking layer for the agentic era.

[![License: CC0](https://img.shields.io/badge/License-CC0-green.svg)](LICENSE)

## 🌐 What is MXP?

MXP (Mesh eXchange Protocol) is an open, high-performance binary protocol designed specifically for agent-to-agent communication. Built on QUIC, it provides:

- **0-RTT connections** - Zero round-trip time for existing connections
- **Built-in observability** - Every message is automatically traced
- **Native streaming** - Perfect for LLM token streams
- **Sub-millisecond latency** - <1ms p99 in same datacenter
- **100x faster than HTTP** - Purpose-built for agents, not documents

## 🎯 Why MXP?

HTTP was designed for document retrieval in 1991. AI agents need something better:

| Feature | HTTP/REST | gRPC | MXP |
|---------|-----------|------|-----|
| Connection Setup | 200-300ms | 100ms | 0ms (0-RTT) |
| Built-in Tracing | ❌ | ❌ | ✅ |
| Native Streaming | 🟡 Bolted on | ✅ | ✅ |
| Agent Discovery | ❌ | ❌ | ✅ |
| Latency (p99) | 50-200ms | 20-50ms | <1ms |

## 📚 Specification

See [SPEC.md](SPEC.md) for the complete protocol specification.

Quick overview:
- **Magic Number:** `0x4D585031` ("MXP1")
- **Header Size:** 32 bytes (cache-aligned)
- **Checksum:** XXHash3 (8 bytes)
- **Transport:** QUIC over UDP
- **Default Port:** 9000
- **Max Payload:** 16 MB

## 🚀 Implementations

### Reference Implementation (Rust)
The reference implementation is in this repository:
- Zero-copy message encoding/decoding
- QUIC transport using Quinn
- Full protocol compliance
- Comprehensive benchmarks

### Other Implementations
We welcome implementations in any language! See [IMPLEMENTATIONS.md](IMPLEMENTATIONS.md) for:
- Implementation guidelines
- Compliance test suite
- Language-specific considerations

## 📖 Documentation

- [Protocol Specification](SPEC.md) - Complete wire format and semantics
- [Design Decisions](docs/design-decisions.md) - Why we made each choice
- [Implementation Guide](docs/implementation-guide.md) - How to implement MXP
- [Benchmarks](docs/benchmarks.md) - Performance characteristics

## 🏗️ Project Structure

```
mxp-protocol/
├── SPEC.md                    # Protocol specification
├── src/                       # Reference implementation (Rust)
│   ├── lib.rs
│   ├── protocol/              # Wire format, codec
│   ├── transport/             # QUIC layer
│   └── types.rs
├── examples/                  # Example usage
├── benches/                   # Performance benchmarks
├── tests/                     # Compliance tests
└── docs/                      # Additional documentation
```

## 🎯 Design Goals

1. **Performance** - Sub-millisecond latency, high throughput
2. **Observability** - Every message is traceable
3. **Simplicity** - Easy to implement and debug
4. **Extensibility** - Forward-compatible design
5. **Zero-Copy** - Minimize memory allocations

## 📊 Performance Targets

- Message encode/decode: **< 1μs**
- QUIC send/receive: **< 100μs**
- P99 latency: **< 1ms** (same datacenter)
- Throughput: **100K msg/sec** per connection

## 🤝 Contributing

MXP is an open protocol. We welcome:
- Protocol improvements and extensions
- Implementations in other languages
- Documentation improvements
- Performance optimizations
- Test cases

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📄 License

The MXP protocol specification is released under **CC0 (public domain)**.

The reference implementation is dual-licensed under **MIT OR Apache-2.0**.

This means:
- ✅ Anyone can implement MXP in any language
- ✅ No attribution required for the protocol
- ✅ Commercial use is encouraged
- ✅ Fork and extend as needed

## 🌐 Links

- **Protocol Site:** [getmxp.xyz](https://getmxp.xyz)
- **Reference Impl:** [github.com/yourusername/mxp-protocol](https://github.com/yourusername/mxp-protocol)
- **Relay Platform:** [relaymxp.xyz](https://relaymxp.xyz) - Agent deployment platform using MXP

## 🙏 Acknowledgments

Inspired by the need for purpose-built infrastructure in the AI era.

Built by engineers frustrated with HTTP for agent communication.

---

**Version:** 1.0.0-draft  
**Status:** In Development  
**Contact:** protocol@getmxp.xyz

**Star us on GitHub if you believe in this vision!** ⭐

