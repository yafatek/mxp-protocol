# MXP Custom Transport Test Plan

This plan enumerates test categories, scenarios, and coverage requirements for the bespoke MXP transport. Each test entry references implementation milestones (see `docs/transport/plan.md`) and contributes evidence toward the acceptance criteria in `docs/transport/targets.md`.

## 1. Test Categories Overview

| Category | Purpose | Tooling |
|----------|---------|---------|
| Unit tests | Validate deterministic behaviour of individual modules | `cargo test`, custom harnesses |
| Property tests | Explore state-space (handshake, scheduler) with random inputs | `proptest`, custom generators |
| Fuzzing | Discover parser/decoder bugs and state machine crashes | `cargo fuzz`, libFuzzer integration |
| Integration tests | Exercise multi-module flows with simulated networks | Custom synchronous harness, mock sockets |
| Soak tests | Detect leaks, drifts, long-term instability | Headless runners, metrics capture |
| Performance benchmarks | Verify throughput/latency targets | `criterion`-style harness, standalone binary |
| Security tests | Replay, DoS, key-rotation, compliance checks | Custom adversarial scripts |

All tests must run without external packages beyond standard Rust tooling; custom harnesses will be implemented in-house.

## 2. Unit Tests

- **Handshake module**: state transitions (Initial→Handshake→Established→Draining), failure paths (auth failure, timeout, rekey).
- **Packet framing**: header encode/decode, alignment guarantees, checksum correctness.
- **AEAD wrapper**: encrypt/decrypt round-trips, nonce reuse detection, error propagation.
- **ACK tracker**: range merging, loss detection thresholds, timer calculations.
- **Scheduler**: priority enforcement, flow control window updates, backpressure triggers.
- **Metrics hooks**: ensure counters increment/decrement correctly.

Each module has 100% branch coverage requirement; measurement aided by `cargo llvm-cov` (or equivalent internal tooling) without external crates.

## 3. Property-Based Tests

- **Handshake transcripts**: random sequences of valid/invalid messages; ensure protocol never violates invariants (no key reuse, no accepting out-of-order flights without checks).
- **Frame decoder**: feed randomized frame sequences with corrupted lengths, verify robust rejection.
- **Scheduler fairness**: generate random traffic mixes; ensure priority weights produce bounded latency per class.
- **Loss recovery**: simulate random packet losses; verify retransmit scheduling remains within target window.

Custom generators written manually (no external `proptest` crate); leverage internal utilities to craft random inputs.

## 4. Fuzz Testing

- **Packet parser fuzz**: mutate raw packets, ensure decoder never panics, memory-safe.
- **Handshake message fuzz**: mutated cryptographic handshake payloads, detect logic errors.
- **Stream control fuzz**: random open/close/data/fin sequences to stress stream manager.

Implement lightweight fuzz harness using in-house mutation engine (no external libFuzzer dependency). Run nightly with crash triage pipeline.

## 5. Integration Tests

### Simulated Network Harness
- Use deterministic mock sockets (in-memory queues) to inject latency, loss, duplication, reordering.
- Scenarios:
  - Clean path handshake + stream send/recv.
  - 1% random packet loss during handshake and data.
  - Out-of-order delivery (up to 5 packet reordering depth).
  - Connection migration (IP/port change) mid-session.
  - Key rotation during high throughput.
  - Flow control exhaustion induced by slow consumer.
  - Datagram burst with rate limiter enforcement.

### Platform Integration
- Swap custom transport under MXP message layer; run existing protocol integration tests to ensure compatibility.
- MXP Nexus platform integration: ensure control plane operations (agent register/discover) behave identically.

## 6. Soak Tests

- 24-hour continuous traffic with mixed workloads; monitor for leaks (memory, file descriptors), counter drift, timer skew.
- Key rotation every minute; ensure no handshake deadlocks.
- Simulated brownouts (pause receiver for short periods) to test backpressure + recovery.

Metrics collected via internal logging to CSV; analyzed offline.

## 7. Performance Benchmarks

- **Microbench**: encode/decode loops with payloads 64 B, 1 KB, 16 KB; measure ns latency.
- **Throughput**: multi-threaded harness generating load; verify per-core and aggregate targets.
- **Latency under loss**: 0.1%, 1%, 5% loss scenarios with RTT 1 ms / 50 ms.
- **Burst handling**: 10× surge for 100 ms; ensure scheduler recovers.

Benchmarks run on dedicated bare-metal instances; results logged in `docs/transport/benchmarks/<date>.md`.

## 8. Security Tests

- **Replay attempts**: resend Initial/Handshake packets; ensure ignored after validation.
- **Amplification probe**: send minimal handshake data; ensure response bounded by anti-amplification limit.
- **Key compromise simulation**: rotate keys; verify old keys rejected.
- **DoS scenarios**: flood connection attempts; validate rate limiting and optional proof-of-work.
- **Cipher suite fallback**: ensure unsupported suites rejected gracefully.

Manual test scripts and automated harnesses documented in `docs/transport/security-tests.md` (to be created).

## 9. Compliance & Documentation Tests

- Verify doc references (architecture, plan, targets) remain consistent; automated doc linter ensures required sections present.
- API documentation coverage: Rustdoc examples compile (`cargo test --doc`).

## 10. Test Matrix Tracking

Maintain `docs/transport/test-matrix.csv` (to be added) listing each test case, status (Planned, Implemented, Automated, Passing), and linked evidence (CI run or manual report).

## 11. Tooling & Infrastructure

- Custom harness crate (`mxpnexus-transport-tests`) provides shared utilities.
- CI matrix executes unit + integration tests on Linux/macOS nightly; fuzz/perf run separately.
- Manual approval required before merging code that skips tests.

## 12. Exit Criteria for Test Readiness

1. All categories above have implemented tests with passing results recorded.
2. Test matrix shows 100% coverage of critical and high-priority scenarios.
3. Soak tests completed thrice without critical incidents.
4. Performance benchmarks meet targets on reference hardware.
5. Security tests signed off by security team.

This plan evolves alongside implementation; updates require review from transport leads and QA.

