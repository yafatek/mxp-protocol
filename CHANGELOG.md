# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-11-01

### Added
- Initial release of MXP Protocol
- Complete wire format specification (32-byte header, cache-aligned)
- 11 message types (Register, Discover, Heartbeat, Call, Response, Event, Stream operations, Ack, Error)
- Zero-copy message encoding/decoding with XXHash3 checksums
- QUIC transport layer using Quinn
- Built-in distributed tracing (trace IDs in every message)
- Comprehensive test suite (14 passing tests)
- Performance benchmarks
- Example: ping-pong message exchange
- Full documentation and SPEC.md
- GitHub Actions CI/CD (tests, clippy, formatting, security audit)
- Automatic crates.io publishing on tag

### Performance
- Message encode/decode: < 100μs (average ~14μs)
- Sub-millisecond latency target
- 100K+ messages/sec throughput capacity

### Documentation
- Complete README with protocol overview
- SPEC.md with detailed wire format
- API documentation with examples
- Contributing guidelines
- Code of Conduct

[Unreleased]: https://github.com/yafatek/mxp-protocol/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yafatek/mxp-protocol/releases/tag/v0.1.0

