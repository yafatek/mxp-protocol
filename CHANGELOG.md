# Changelog

All notable changes to the MXP protocol implementation will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2025-11-07

### Added
- **Comprehensive benchmark suite** for codec performance
  - Encode/decode benchmarks across multiple payload sizes (64B-16KB)
  - Message type-specific benchmarks
  - Comparison benchmarks (MXP vs JSON/Bincode/MessagePack)
- **Property-based testing** with proptest
  - 8 new property tests covering roundtrip, corruption detection, validation
  - ~2,048 random test cases per test run
- **Performance documentation**
  - `docs/PERFORMANCE_NOTES.md` - Detailed baseline analysis
  - `docs/BENCHMARK_REPRODUCIBILITY.md` - Reproducibility verification
  - `docs/COMPARISON_BENCHMARKS.md` - Protocol comparison analysis

### Changed
- **README.md** updated with verified performance claims
  - Added real benchmark data (37x faster than JSON)
  - Honest comparisons with other protocols
  - Clear positioning: agent-native features + performance
  - Added "What Makes MXP Different?" section
- Version bumped to 0.2.0

### Performance
- **Codec performance verified**: 60ns total (encode + decode) for 256-byte messages
  - 27ns encode time (37x faster than 1μs target)
  - 14ns decode time
- **Throughput**: 16.6M messages/second (single-threaded)
- **Comparison results** (256-byte messages):
  - 37.5x faster than JSON
  - 19.5x faster than MessagePack
  - 3.7x faster than Bincode

### Testing
- All 77 tests passing (69 original + 8 property-based)
- 100% reproducibility (±2% variance across runs)
- Zero test failures

### Notes
- Transport benchmarks placeholder added (will be enabled when APIs are public)
- This release focuses on codec performance validation and documentation

## [0.1.0] - 2025-11-06

### Added
- Initial MXP protocol implementation
- Core message codec (encode/decode)
- Custom UDP transport layer
- Agent lifecycle management (Register, Discover, Heartbeat)
- Built-in distributed tracing
- ChaCha20-Poly1305 encryption
- 69 passing tests
- Complete wire format specification

[0.2.0]: https://github.com/yafatek/mxp-protocol/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/yafatek/mxp-protocol/releases/tag/v0.1.0
