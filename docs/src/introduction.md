# MXP Protocol Documentation

Welcome to the MXP (Mesh eXchange Protocol) technical documentation.

## What is MXP?

MXP is a high-performance binary protocol designed specifically for agent-to-agent communication in distributed systems. Built on QUIC (RFC 9000), it provides:

- **Sub-millisecond latency** - Optimized for datacenter and cross-region communication
- **Zero-copy operations** - Minimal allocations in hot paths
- **Built-in observability** - Every message includes distributed tracing context
- **Native streaming** - First-class support for streaming data flows
- **0-RTT connections** - Resume connections without handshake overhead

## Protocol Overview

```
┌─────────────────────────────────────────┐
│         Application Layer               │
│    (Your AI agents and services)        │
└─────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│          MXP Protocol Layer             │
│  • 32-byte header (cache-aligned)       │
│  • XXHash3 checksums                    │
│  • 11 message types                     │
└─────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│         QUIC Transport Layer            │
│  • 0-RTT establishment                  │
│  • TLS 1.3 encryption                   │
│  • Stream multiplexing                  │
└─────────────────────────────────────────┘
```

## Quick Links

- **[Wire Format](./protocol/wire-format.md)** - Complete protocol specification
- **[Getting Started](./implementation/getting-started.md)** - Build your first MXP application
- **[API Reference](./reference/api.md)** - Complete API documentation
- **[Examples](./examples/basic.md)** - Working code examples

## Use Cases

MXP is designed for:

- **AI Agent Communication** - High-speed coordination between agents
- **Microservices** - Low-latency service-to-service communication
- **Real-time Systems** - Sub-millisecond message delivery
- **Streaming Applications** - LLM token streams, event processing
- **Distributed Systems** - Mesh architectures with built-in tracing

## Performance

| Metric | Target | Typical |
|--------|--------|---------|
| Message encode/decode | < 1 μs | 0.5 μs |
| P99 latency (same DC) | < 1 ms | 0.3 ms |
| Throughput | 100K msg/s | 150K msg/s |
| Connection setup | 0-RTT | Instant |

## Getting Help

- **GitHub Issues:** [Report bugs](https://github.com/yafatek/mxp-protocol/issues)
- **Discussions:** [Ask questions](https://github.com/yafatek/mxp-protocol/discussions)
- **Email:** protocol@getmxp.xyz

## License

Protocol specification: Public domain (CC0)  
Reference implementation: MIT OR Apache-2.0

